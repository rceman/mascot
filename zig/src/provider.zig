const std = @import("std");
const win32 = @import("win32.zig");
const framing = @import("framing.zig");
const queue_mod = @import("queue.zig");
const config = @import("config.zig");
const jsonw = @import("json.zig");

const c = win32.c;
const Allocator = std.mem.Allocator;

pub fn qpc() i64 {
    return win32.qpc();
}

pub fn qpcFrequency() i64 {
    return win32.qpcFrequency();
}

fn nowMs() i64 {
    return win32.nowMs();
}

pub const EventKind = enum { complete, cancelled, failed };

pub const Event = union(enum) {
    started: struct { generation: u64, pid: u32 },
    chunk: struct { generation: u64, id: u64, seq: u32, text: []u8, receipt_qpc: i64 },
    terminal: struct { generation: u64, id: u64, kind: EventKind, last_seq: i64 },
    session_closed: struct { generation: u64, exit_code: ?u32, err: ?[]u8 },
    stopped: struct { err: ?[]u8 },

    pub fn deinit(self: Event, gpa: Allocator) void {
        switch (self) {
            .chunk => |e| gpa.free(e.text),
            .session_closed => |e| {
                if (e.err) |s| gpa.free(s);
            },
            .stopped => |e| {
                if (e.err) |s| gpa.free(s);
            },
            else => {},
        }
    }
};

pub const Command = union(enum) {
    request: struct { id: u64, prompt: []u8, scenario: []u8 }, // owned
    cancel: u64,
    shutdown,

    fn deinit(self: Command, gpa: Allocator) void {
        switch (self) {
            .request => |r| {
                gpa.free(r.prompt);
                gpa.free(r.scenario);
            },
            else => {},
        }
    }
};

const Work = union(enum) {
    run: Command,
    client_response: struct { generation: u64, id: u64, request_id: u64 },
    session_failed: struct { generation: u64, err: []u8 }, // err owned
    session_eof: struct { generation: u64 },

    fn deinit(self: Work, gpa: Allocator) void {
        switch (self) {
            .run => |cmd| cmd.deinit(gpa),
            .session_failed => |sf| gpa.free(sf.err),
            else => {},
        }
    }
};

const Phase = enum { await_start, streaming, terminal };

const Protocol = struct {
    id: u64,
    expected_chunks: u64,
    next_seq: u64 = 0,
    phase: Phase = .await_start,
    cancel_requested: bool = false,
    client_request_seen: bool = false,
    requires_client_response: bool = false,
    client_response_sent: bool = false,
    terminal_emitted: bool = false,

    fn claimTerminal(self: *Protocol) ?TerminalClaim {
        if (self.terminal_emitted) return null;
        self.terminal_emitted = true;
        self.phase = .terminal;
        return .{ .id = self.id, .last_seq = @as(i64, @intCast(self.next_seq)) - 1 };
    }
};

pub const Shared = struct {
    mutex: std.Thread.Mutex = .{},
    cond: std.Thread.Condition = .{},
    generation: u64 = 0,
    protocol: ?Protocol = null,
    eof: bool = false,
    shutdown_sent: bool = false,
    shutdown_ack: bool = false,
    shutdown_requested: bool = false,
    shutdown_deadline_ms: i64 = 0,
    failed: ?[]u8 = null, // owned

    fn setFailed(self: *Shared, gpa: Allocator, msg: []const u8) void {
        // caller must hold mutex
        if (self.failed == null) {
            self.failed = gpa.dupe(u8, msg) catch null;
        }
    }

    fn clearFailed(self: *Shared, gpa: Allocator) void {
        if (self.failed) |m| gpa.free(m);
        self.failed = null;
    }

    fn notify(self: *Shared) void {
        self.cond.broadcast();
    }
};

const TerminalClaim = struct { id: u64, last_seq: i64 };

pub const StderrTail = struct {
    mutex: std.Thread.Mutex = .{},
    buf: [4096]u8 = undefined,
    len: usize = 0,
    total: u64 = 0,

    fn append(self: *StderrTail, data: []const u8) void {
        self.mutex.lock();
        defer self.mutex.unlock();
        self.total += data.len;
        for (data) |b| {
            if (self.len == self.buf.len) {
                std.mem.copyForwards(u8, self.buf[0 .. self.buf.len - 1], self.buf[1..]);
                self.buf[self.buf.len - 1] = b;
            } else {
                self.buf[self.len] = b;
                self.len += 1;
            }
        }
    }

    pub fn totalBytes(self: *StderrTail) u64 {
        self.mutex.lock();
        defer self.mutex.unlock();
        return self.total;
    }
};

pub const ProviderConfig = struct {
    path: []const u8,
    arguments: []const []const u8,
    cwd: []const u8,
    environment: []const EnvPair,
    manifest: std.json.Value, // object; for scenario chunk counts
    cancel_timeout_ms: u64,
    shutdown_timeout_ms: u64,

    pub const EnvPair = struct { key: []const u8, value: []const u8 };
};

const Diag = struct {
    buf: [192]u8 = undefined,
    msg: ?[]const u8 = null,

    fn set(self: *Diag, s: []const u8) void {
        self.msg = s;
    }

    fn setf(self: *Diag, comptime fmt: []const u8, args: anytype) void {
        self.msg = std.fmt.bufPrint(&self.buf, fmt, args) catch "protocol violation";
    }
};

const ReaderCtx = struct {
    shared: *Shared,
    events: *queue_mod.BoundedQueue(Event),
    records: ?*queue_mod.BoundedQueue([]u8),
    work: *queue_mod.BoundedQueue(Work),
    wake: c.HWND,
    generation: u64,
    gpa: Allocator,
    diag: Diag = .{},
};

const Session = struct {
    generation: u64,
    child: std.process.Child,
    argv: [][]const u8, // owned buffer; element slices borrow config
    stdout_reader: ?std.Thread = null,
    stderr_reader: ?std.Thread = null,
};

fn pushEvent(
    events: *queue_mod.BoundedQueue(Event),
    wake: c.HWND,
    event: Event,
    gpa: Allocator,
) error{Closed}!void {
    events.push(event) catch {
        event.deinit(gpa);
        return error.Closed;
    };
    _ = c.PostMessageW(wake, WM_APP_PROVIDER, 0, 0);
}

pub const WM_APP_PROVIDER_MSG: u32 = c.WM_APP + 1;

const WM_APP_PROVIDER: c.UINT = c.WM_APP + 1;

pub const Provider = struct {
    gpa: Allocator,
    work: *queue_mod.BoundedQueue(Work),
    shared: *Shared,
    stderr_tail: *StderrTail,
    coordinator: ?std.Thread = null,
    shutdown_timeout_ms: u64,

    pub fn spawn(
        gpa: Allocator,
        cfg: ProviderConfig,
        events: *queue_mod.BoundedQueue(Event),
        records: ?*queue_mod.BoundedQueue([]u8),
        wake: c.HWND,
    ) !*Provider {
        const shared = try gpa.create(Shared);
        shared.* = .{};
        const work = try queue_mod.BoundedQueue(Work).init(gpa, 16);
        const tail = try gpa.create(StderrTail);
        tail.* = .{};
        const p = try gpa.create(Provider);
        p.* = .{
            .gpa = gpa,
            .work = work,
            .shared = shared,
            .stderr_tail = tail,
            .shutdown_timeout_ms = cfg.shutdown_timeout_ms,
        };
        const ctx = try gpa.create(CoordCtx);
        ctx.* = .{
            .cfg = cfg,
            .work = work,
            .shared = shared,
            .events = events,
            .records = records,
            .tail = tail,
            .wake = wake,
            .gpa = gpa,
        };
        p.coordinator = std.Thread.spawn(.{}, coordinatorRun, .{ctx}) catch |e| {
            gpa.destroy(ctx);
            gpa.destroy(p);
            work.deinit(gpa);
            gpa.destroy(tail);
            shared.clearFailed(gpa);
            gpa.destroy(shared);
            return e;
        };
        return p;
    }

    /// UI-facing send. Request/Cancel are queued for the coordinator;
    /// Shutdown arms the deadline flag and closes the work queue.
    pub fn send(self: *Provider, cmd: Command) !void {
        switch (cmd) {
            .shutdown => {
                {
                    self.shared.mutex.lock();
                    defer self.shared.mutex.unlock();
                    if (!self.shared.shutdown_requested) {
                        self.shared.shutdown_requested = true;
                        self.shared.shutdown_deadline_ms = nowMs() + @as(i64, @intCast(self.shutdown_timeout_ms));
                    }
                }
                self.shared.notify();
                self.work.close();
            },
            else => {
                self.work.tryPush(.{ .run = cmd }) catch {
                    cmd.deinit(self.gpa);
                    return error.ProviderQueueUnavailable;
                };
            },
        }
    }

    pub fn stderrTotal(self: *Provider) u64 {
        return self.stderr_tail.totalBytes();
    }

    /// Bounded join of the coordinator thread; false on timeout.
    pub fn join(self: *Provider, timeout_ms: u64) bool {
        const t = self.coordinator orelse return true;
        const deadline = nowMs() + @as(i64, @intCast(timeout_ms));
        return win32.joinBounded(t, deadline, false);
    }

    /// Free remaining structures after join succeeds (or on teardown bail).
    pub fn deinit(self: *Provider) void {
        const gpa = self.gpa;
        // drain any undelivered work items
        while (self.work.pop()) |item| item.deinit(gpa);
        self.work.deinit(gpa);
        self.shared.clearFailed(gpa);
        gpa.destroy(self.shared);
        gpa.destroy(self.stderr_tail);
        gpa.destroy(self);
    }
};

const CoordCtx = struct {
    cfg: ProviderConfig,
    work: *queue_mod.BoundedQueue(Work),
    shared: *Shared,
    events: *queue_mod.BoundedQueue(Event),
    records: ?*queue_mod.BoundedQueue([]u8),
    tail: *StderrTail,
    wake: c.HWND,
    gpa: Allocator,
};

fn spawnChild(ctx: *CoordCtx) !*Session {
    const gpa = ctx.gpa;
    const session = try gpa.create(Session);
    errdefer gpa.destroy(session);

    var argv = try gpa.alloc([]const u8, 1 + ctx.cfg.arguments.len);
    errdefer gpa.free(argv);
    argv[0] = ctx.cfg.path;
    for (ctx.cfg.arguments, 0..) |a, i| argv[1 + i] = a;

    var env = std.process.EnvMap.init(gpa);
    defer env.deinit();
    for (ctx.cfg.environment) |pair| {
        try env.put(pair.key, pair.value);
    }

    var child = std.process.Child.init(argv, gpa);
    child.cwd = ctx.cfg.cwd;
    child.env_map = &env;
    child.stdin_behavior = .Pipe;
    child.stdout_behavior = .Pipe;
    child.stderr_behavior = .Pipe;
    child.create_no_window = true;
    child.spawn() catch |e| {
        var buf: [128]u8 = undefined;
        const detail = std.fmt.bufPrint(&buf, "provider spawn: {s}", .{@errorName(e)}) catch "provider spawn failed";
        // stash detail into session-less error path via setFailed below
        ctx.shared.mutex.lock();
        ctx.shared.setFailed(gpa, detail);
        ctx.shared.mutex.unlock();
        return error.SpawnFailed;
    };
    // The Zig Child API cleared the inherit flag on our parent-side ends.
    // Only the child-side pipe ends were inheritable.
    const pid = c.GetProcessId(child.id);

    const generation = gen: {
        ctx.shared.mutex.lock();
        defer ctx.shared.mutex.unlock();
        ctx.shared.generation += 1;
        ctx.shared.eof = false;
        ctx.shared.clearFailed(gpa);
        ctx.shared.protocol = null;
        ctx.shared.shutdown_sent = false;
        ctx.shared.shutdown_ack = false;
        break :gen ctx.shared.generation;
    };

    session.* = .{
        .generation = generation,
        .child = child,
        .argv = argv,
    };

    const reader_ctx = gpa.create(ReaderCtx) catch {
        // Child was spawned; reap it rather than leaking. errdefer frees
        // session and argv.
        _ = c.TerminateProcess(child.id, 1);
        _ = win32.waitHandle(child.id, nowMs() + 1000);
        _ = c.CloseHandle(child.id);
        _ = c.CloseHandle(child.thread_handle);
        if (child.stdin) |f| {
            var ff = f;
            ff.close();
        }
        if (child.stdout) |f| {
            var ff = f;
            ff.close();
        }
        if (child.stderr) |f| {
            var ff = f;
            ff.close();
        }
        return error.OutOfMemory;
    };
    reader_ctx.* = .{
        .shared = ctx.shared,
        .events = ctx.events,
        .records = ctx.records,
        .work = ctx.work,
        .wake = ctx.wake,
        .generation = generation,
        .gpa = gpa,
    };

    const stdout_handle = session.child.stdout.?.handle;
    const stderr_handle = session.child.stderr.?.handle;
    session.stdout_reader = std.Thread.spawn(.{}, stdoutRun, .{ reader_ctx, stdout_handle }) catch |e| blk: {
        std.debug.print("stdout reader spawn failed: {s}\n", .{@errorName(e)});
        gpa.destroy(reader_ctx);
        break :blk null;
    };
    session.stderr_reader = std.Thread.spawn(.{}, stderrRun, .{ ctx.tail, stderr_handle }) catch |e| blk: {
        std.debug.print("stderr reader spawn failed: {s}\n", .{@errorName(e)});
        break :blk null;
    };
    _ = pushEvent(ctx.events, ctx.wake, .{ .started = .{ .generation = generation, .pid = pid } }, gpa) catch {};
    return session;
}

fn writeLine(file: std.fs.File, bytes: []const u8) !void {
    try file.writeAll(bytes);
    try file.writeAll("\n");
}

/// Wait for a provider-initiated client_request to have been seen, then
/// answer it once with the frozen accepted response.
fn serviceClientResponse(
    active: *Session,
    shared: *Shared,
    deadline_ms: i64,
) !void {
    {
        shared.mutex.lock();
        defer shared.mutex.unlock();
        while (true) {
            const protocol = shared.protocol orelse return;
            if (!protocol.requires_client_response or protocol.client_response_sent) return;
            if (protocol.client_request_seen) break;
            if (shared.eof or shared.failed != null) return;
            const remaining = deadline_ms - nowMs();
            if (remaining <= 0) return error.ClientResponseDeadline;
            shared.cond.timedWait(&shared.mutex, @intCast(remaining * std.time.ns_per_ms)) catch {};
        }
    }
    try writeLine(active.child.stdin.?, "{\"type\":\"client_response\",\"request_id\":1,\"result\":{\"accepted\":true}}");
    shared.mutex.lock();
    defer shared.mutex.unlock();
    if (shared.protocol) |*p| p.client_response_sent = true;
}

fn teardown(
    session: *?*Session,
    ctx: *CoordCtx,
    error_msg: ?[]const u8,
    deadline_ms: i64,
) ?[]const u8 {
    const active = session.* orelse return error_msg;
    session.* = null;
    const gpa = ctx.gpa;
    var err = error_msg;

    const handle = active.child.id;
    const grace = if (err != null) nowMs() + 100 else deadline_ms;
    var reaped = win32.waitHandle(handle, @min(grace, deadline_ms));
    if (!reaped) {
        if (c.TerminateProcess(handle, 1) == 0) {
            err = joinErr(gpa, err, "provider kill failed");
        }
        reaped = win32.waitHandle(handle, deadline_ms);
        if (!reaped) {
            err = joinErr(gpa, err, "provider did not exit before teardown deadline");
        }
    }
    var exit_code: ?u32 = null;
    if (reaped) {
        var code: c.DWORD = 0;
        if (c.GetExitCodeProcess(handle, &code) != 0) exit_code = code;
    }
    // Close process handles: the child is reaped or irrecoverable.
    _ = c.CloseHandle(handle);
    _ = c.CloseHandle(active.child.thread_handle);
    if (active.child.stdin) |f| {
        var ff = f;
        ff.close();
        active.child.stdin = null;
    }

    const reader_deadline = @max(deadline_ms, nowMs() + 500);
    if (active.stdout_reader) |t| {
        if (!win32.joinBounded(t, reader_deadline, false)) {
            err = joinErr(gpa, err, "provider reader did not finish within teardown bound");
        }
        active.stdout_reader = null;
    }
    if (active.stderr_reader) |t| {
        if (!win32.joinBounded(t, reader_deadline, false)) {
            err = joinErr(gpa, err, "provider reader did not finish within teardown bound");
        }
        active.stderr_reader = null;
    }
    if (active.child.stdout) |f| {
        var ff = f;
        ff.close();
        active.child.stdout = null;
    }
    if (active.child.stderr) |f| {
        var ff = f;
        ff.close();
        active.child.stderr = null;
    }

    const generation = active.generation;
    const owned_err: ?[]u8 = if (err) |e| gpa.dupe(u8, e) catch null else null;
    gpa.free(active.argv);
    gpa.destroy(active);
    _ = pushEvent(ctx.events, ctx.wake, .{ .session_closed = .{
        .generation = generation,
        .exit_code = exit_code,
        .err = owned_err,
    } }, gpa) catch {
        if (owned_err) |e| gpa.free(e);
    };
    return err;
}

fn joinErr(gpa: Allocator, err: ?[]const u8, msg: []const u8) ?[]const u8 {
    if (err) |e| {
        return std.fmt.allocPrint(gpa, "{s}; {s}", .{ e, msg }) catch err;
    }
    return msg;
}

fn gracefulShutdown(
    session: *?*Session,
    ctx: *CoordCtx,
    timeout_ms: u64,
) ?[]const u8 {
    const deadline_ms = blk: {
        ctx.shared.mutex.lock();
        defer ctx.shared.mutex.unlock();
        if (ctx.shared.shutdown_deadline_ms != 0) break :blk ctx.shared.shutdown_deadline_ms;
        const d = nowMs() + @as(i64, @intCast(timeout_ms));
        ctx.shared.shutdown_deadline_ms = d;
        break :blk d;
    };
    var err: ?[]const u8 = null;
    if (session.*) |active| {
        const ack_deadline = @max(deadline_ms - 100, nowMs());
        _ = serviceClientResponse(active, ctx.shared, ack_deadline) catch {};
        {
            ctx.shared.mutex.lock();
            ctx.shared.shutdown_sent = true;
            ctx.shared.mutex.unlock();
        }
        writeLine(active.child.stdin.?, "{\"type\":\"shutdown\"}") catch {
            err = "provider shutdown write failed";
        };
        ctx.shared.mutex.lock();
        while (true) {
            if (ctx.shared.eof or ctx.shared.shutdown_ack) break;
            const remaining = ack_deadline - nowMs();
            if (remaining <= 0) break;
            ctx.shared.cond.timedWait(&ctx.shared.mutex, @intCast(remaining * std.time.ns_per_ms)) catch break;
        }
        const acknowledged = ctx.shared.shutdown_ack;
        ctx.shared.mutex.unlock();
        if (!acknowledged and err == null) {
            err = "provider shutdown acknowledgement timeout";
        }
    }
    return teardown(session, ctx, err, deadline_ms);
}

fn coordinatorRun(ctx: *CoordCtx) void {
    const gpa = ctx.gpa;
    var session: ?*Session = null;
    defer gpa.destroy(ctx);

    while (true) {
        {
            ctx.shared.mutex.lock();
            const sd = ctx.shared.shutdown_requested;
            ctx.shared.mutex.unlock();
            if (sd) break;
        }
        const item = ctx.work.waitPop() orelse break;
        switch (item) {
            .run => |cmd| switch (cmd) {
                .request => |req| {
                    defer cmd.deinit(gpa);
                    const expected = config.expectedChunks(ctx.cfg.manifest, req.scenario);
                    const needs_child = blk: {
                        if (session == null) break :blk true;
                        ctx.shared.mutex.lock();
                        const f = ctx.shared.failed != null;
                        ctx.shared.mutex.unlock();
                        break :blk f;
                    };
                    if (needs_child) {
                        _ = teardown(&session, ctx, null, nowMs() + @as(i64, @intCast(ctx.cfg.shutdown_timeout_ms)));
                        const new_session = spawnChild(ctx) catch {
                            const gen = genblk: {
                                ctx.shared.mutex.lock();
                                const g = ctx.shared.generation;
                                ctx.shared.mutex.unlock();
                                break :genblk g;
                            };
                            const spawn_err: ?[]u8 = blk: {
                                ctx.shared.mutex.lock();
                                const m = ctx.shared.failed;
                                break :blk if (m) |s| gpa.dupe(u8, s) catch null else null;
                            };
                            // take failed out of shared so spawn-reset semantics stay clean
                            ctx.shared.mutex.lock();
                            ctx.shared.clearFailed(gpa);
                            ctx.shared.mutex.unlock();
                            _ = pushEvent(ctx.events, ctx.wake, .{ .terminal = .{
                                .generation = gen,
                                .id = req.id,
                                .kind = .failed,
                                .last_seq = -1,
                            } }, gpa) catch {};
                            _ = pushEvent(ctx.events, ctx.wake, .{ .session_closed = .{
                                .generation = gen,
                                .exit_code = null,
                                .err = spawn_err,
                            } }, gpa) catch {
                                if (spawn_err) |e| gpa.free(e);
                            };
                            continue;
                        };
                        session = new_session;
                    }
                    {
                        ctx.shared.mutex.lock();
                        ctx.shared.protocol = .{
                            .id = req.id,
                            .expected_chunks = expected,
                            .requires_client_response = std.mem.eql(u8, req.scenario, "client_request"),
                        };
                        ctx.shared.mutex.unlock();
                    }
                    var frame = jsonw.Writer.init(gpa);
                    defer frame.deinit();
                    frame.beginObject(null) catch {};
                    frame.uint("id", req.id) catch {};
                    frame.string("prompt", req.prompt) catch {};
                    frame.string("scenario", req.scenario) catch {};
                    frame.string("type", "request") catch {};
                    frame.endObject() catch {};
                    const frame_bytes = frame.owned() catch null;
                    defer if (frame_bytes) |b| gpa.free(b);
                    if (session) |active| {
                        const write_ok = blk: {
                            const b = frame_bytes orelse break :blk false;
                            writeLine(active.child.stdin.?, b) catch break :blk false;
                            break :blk true;
                        };
                        if (!write_ok) {
                            var claim: ?TerminalClaim = null;
                            {
                                ctx.shared.mutex.lock();
                                ctx.shared.setFailed(gpa, "provider stdin write failed");
                                if (ctx.shared.protocol) |*p| claim = p.claimTerminal();
                                ctx.shared.mutex.unlock();
                            }
                            ctx.shared.notify();
                            if (claim) |t| {
                                _ = pushEvent(ctx.events, ctx.wake, .{ .terminal = .{
                                    .generation = if (session) |s| s.generation else 0,
                                    .id = t.id,
                                    .kind = .failed,
                                    .last_seq = t.last_seq,
                                } }, gpa) catch {};
                            }
                            _ = teardown(&session, ctx, "stdin write failed", nowMs() + @as(i64, @intCast(ctx.cfg.shutdown_timeout_ms)));
                        }
                    }
                },
                .cancel => |id| {
                    if (session) |active| {
                        {
                            ctx.shared.mutex.lock();
                            if (ctx.shared.protocol) |*p| {
                                if (p.id == id) p.cancel_requested = true;
                            }
                            ctx.shared.mutex.unlock();
                        }
                        const deadline = nowMs() + @as(i64, @intCast(ctx.cfg.cancel_timeout_ms));
                        serviceClientResponse(active, ctx.shared, deadline) catch {
                            {
                                ctx.shared.mutex.lock();
                                ctx.shared.setFailed(gpa, "client_request response deadline exceeded");
                                ctx.shared.mutex.unlock();
                            }
                            ctx.shared.notify();
                            _ = teardown(&session, ctx, "client_request response deadline exceeded", nowMs() + @as(i64, @intCast(ctx.cfg.shutdown_timeout_ms)));
                            continue;
                        };
                        var cframe = jsonw.Writer.init(gpa);
                        defer cframe.deinit();
                        cframe.beginObject(null) catch {};
                        cframe.uint("id", id) catch {};
                        cframe.string("type", "cancel") catch {};
                        cframe.endObject() catch {};
                        const cbytes = cframe.owned() catch null;
                        defer if (cbytes) |b| gpa.free(b);
                        if (cbytes) |b| {
                            writeLine(active.child.stdin.?, b) catch {};
                        }
                        var claim: ?TerminalClaim = null;
                        {
                            ctx.shared.mutex.lock();
                            while (true) {
                                const terminal = if (ctx.shared.protocol) |p| p.phase == .terminal else false;
                                if (terminal or ctx.shared.failed != null or ctx.shared.eof or ctx.shared.shutdown_requested)
                                    break;
                                const remaining = deadline - nowMs();
                                if (remaining <= 0) break;
                                ctx.shared.cond.timedWait(&ctx.shared.mutex, @intCast(remaining * std.time.ns_per_ms)) catch break;
                            }
                            const terminal_seen = if (ctx.shared.protocol) |p| p.phase == .terminal else false;
                            const aborted = ctx.shared.failed != null or ctx.shared.eof or ctx.shared.shutdown_requested;
                            if (!(terminal_seen or aborted)) {
                                ctx.shared.setFailed(gpa, "cancel deadline exceeded");
                                if (ctx.shared.protocol) |*p| claim = p.claimTerminal();
                            }
                            ctx.shared.mutex.unlock();
                        }
                        if (claim) |t| {
                            ctx.shared.notify();
                            _ = pushEvent(ctx.events, ctx.wake, .{ .terminal = .{
                                .generation = if (session) |s| s.generation else 0,
                                .id = t.id,
                                .kind = .failed,
                                .last_seq = t.last_seq,
                            } }, gpa) catch {};
                            _ = teardown(&session, ctx, "cancel deadline exceeded", nowMs() + @as(i64, @intCast(ctx.cfg.shutdown_timeout_ms)));
                        }
                    }
                },
                .shutdown => {},
            },
            .client_response => |cr| {
                if (session) |active| {
                    const current = blk: {
                        ctx.shared.mutex.lock();
                        const ok = ctx.shared.generation == cr.generation and
                            (if (ctx.shared.protocol) |p| p.id == cr.id else false) and
                            cr.request_id == 1;
                        ctx.shared.mutex.unlock();
                        break :blk ok;
                    };
                    if (current) {
                        const deadline = nowMs() + @as(i64, @intCast(ctx.cfg.cancel_timeout_ms));
                        serviceClientResponse(active, ctx.shared, deadline) catch {
                            ctx.shared.mutex.lock();
                            ctx.shared.setFailed(gpa, "client_request response deadline exceeded");
                            ctx.shared.mutex.unlock();
                            ctx.shared.notify();
                        };
                    }
                }
            },
            .session_failed => |sf| {
                defer gpa.free(sf.err);
                if (session) |s| {
                    if (s.generation == sf.generation) {
                        _ = teardown(&session, ctx, sf.err, nowMs() + @as(i64, @intCast(ctx.cfg.shutdown_timeout_ms)));
                    }
                }
            },
            .session_eof => |se| {
                if (session) |s| {
                    if (s.generation == se.generation) {
                        _ = teardown(&session, ctx, null, nowMs() + @as(i64, @intCast(ctx.cfg.shutdown_timeout_ms)));
                    }
                }
            },
        }
        {
            ctx.shared.mutex.lock();
            const sd = ctx.shared.shutdown_requested;
            ctx.shared.mutex.unlock();
            if (sd) break;
        }
    }

    const shutdown_err = gracefulShutdown(&session, ctx, ctx.cfg.shutdown_timeout_ms);
    const owned_err: ?[]u8 = if (shutdown_err) |e| gpa.dupe(u8, e) catch null else null;
    _ = pushEvent(ctx.events, ctx.wake, .{ .stopped = .{ .err = owned_err } }, gpa) catch {
        if (owned_err) |e| gpa.free(e);
    };
}

fn stdoutRun(ctx: *ReaderCtx, pipe_handle: c.HANDLE) void {
    const gpa = ctx.gpa;
    defer gpa.destroy(ctx);
    var decoder = framing.Decoder.init(gpa) catch return;
    defer decoder.deinit();
    const event = win32.createPipeEvent() catch return;
    defer _ = c.CloseHandle(event);
    var buf: [8192]u8 = undefined;
    var fatal: ?[]const u8 = null;
    while (true) {
        const n = win32.readPipe(pipe_handle, &buf, event) catch {
            fatal = "provider stdout read failed";
            break;
        };
        if (n == 0) break;
        decoder.feed(buf[0..n], qpcClock, ctx, handleFrame) catch |err| {
            fatal = switch (err) {
                error.FrameRejected => ctx.diag.msg orelse "provider stream error",
                error.FrameTooLarge => "logical frame exceeds 65536-byte limit",
                error.InvalidUtf8 => "invalid frame UTF-8",
                else => "decoder session already failed",
            };
            break;
        };
        {
            ctx.shared.mutex.lock();
            const f = ctx.shared.failed;
            ctx.shared.mutex.unlock();
            if (f) |msg| {
                fatal = msg;
                break;
            }
        }
    }

    var claim: ?TerminalClaim = null;
    {
        ctx.shared.mutex.lock();
        ctx.shared.eof = true;
        if (fatal == null) {
            decoder.finish() catch {
                fatal = "failed or incomplete NDJSON stream";
            };
        }
        if (fatal == null) {
            if (ctx.shared.protocol) |p| {
                if (p.phase != .terminal and !ctx.shared.shutdown_requested and !ctx.shared.shutdown_sent) {
                    fatal = "provider stdout ended mid-request";
                }
            }
        }
        if (fatal) |msg| {
            ctx.shared.setFailed(gpa, msg);
        }
        if (fatal != null) {
            if (ctx.shared.protocol) |*p| claim = p.claimTerminal();
        }
        ctx.shared.mutex.unlock();
    }
    ctx.shared.notify();

    if (fatal) |msg| {
        if (claim) |t| {
            _ = pushEvent(ctx.events, ctx.wake, .{ .terminal = .{
                .generation = ctx.generation,
                .id = t.id,
                .kind = .failed,
                .last_seq = t.last_seq,
            } }, gpa) catch {};
        }
        const owned = gpa.dupe(u8, msg) catch {
            return;
        };
        ctx.work.tryPush(.{ .session_failed = .{ .generation = ctx.generation, .err = owned } }) catch {
            gpa.free(owned);
            ctx.shared.mutex.lock();
            const sd = ctx.shared.shutdown_requested;
            ctx.shared.mutex.unlock();
            if (!sd) {
                const rec = buildErrorRecord(gpa, "provider work queue unavailable") catch null;
                if (rec) |r| {
                    ctx.records.?.tryPush(r) catch gpa.free(r);
                }
            }
        };
    } else {
        ctx.work.tryPush(.{ .session_eof = .{ .generation = ctx.generation } }) catch {};
    }
}

fn qpcClock() i64 {
    return qpc();
}

fn stderrRun(tail: *StderrTail, pipe_handle: c.HANDLE) void {
    const event = win32.createPipeEvent() catch return;
    defer _ = c.CloseHandle(event);
    var buf: [8192]u8 = undefined;
    while (true) {
        const n = win32.readPipe(pipe_handle, &buf, event) catch return;
        if (n == 0) return;
        tail.append(buf[0..n]);
    }
}

fn buildErrorRecord(gpa: Allocator, msg: []const u8) ![]u8 {
    var w = jsonw.Writer.init(gpa);
    errdefer w.deinit();
    try w.beginObject(null);
    try w.rawField("token", "null");
    try w.boolean("ok", false);
    try w.string("error", msg);
    try w.endObject();
    return w.owned();
}

fn buildFrameRecord(gpa: Allocator, id: u64, seq: u64, receipt_qpc: i64, emit_qpc: []const u8) ?[]u8 {
    var w = jsonw.Writer.init(gpa);
    w.beginObject(null) catch {
        w.deinit();
        return null;
    };
    w.string("event", "frame_received") catch {};
    w.uint("request_id", id) catch {};
    w.uint("seq", seq) catch {};
    var receipt_buf: [24]u8 = undefined;
    const receipt_str = std.fmt.bufPrint(&receipt_buf, "{d}", .{receipt_qpc}) catch "0";
    w.string("receipt_qpc", receipt_str) catch {};
    w.string("emit_qpc", emit_qpc) catch {};
    w.int("qpc_frequency", qpcFrequency()) catch {};
    w.endObject() catch {};
    return w.owned() catch null;
}

const FrameFail = error{Rejected};

fn fail(ctx: *ReaderCtx, msg: []const u8) FrameFail {
    ctx.diag.set(msg);
    return error.Rejected;
}

const Deliver = union(enum) {
    none,
    chunk: struct { id: u64, seq: u32, text: []u8 },
    terminal: struct { id: u64, kind: EventKind, last_seq: i64 },
};

fn frameString(value: std.json.Value, key: []const u8) ?[]const u8 {
    if (value != .object) return null;
    const f = value.object.get(key) orelse return null;
    if (f != .string) return null;
    return f.string;
}

fn frameInt(value: std.json.Value, key: []const u8) ?i64 {
    if (value != .object) return null;
    const f = value.object.get(key) orelse return null;
    if (f != .integer) return null;
    return f.integer;
}

fn handleFrame(ctx_any: anytype, frame: []const u8, receipt_qpc: i64) FrameFail!void {
    const ctx: *ReaderCtx = ctx_any;
    const gpa = ctx.gpa;
    var arena = std.heap.ArenaAllocator.init(gpa);
    defer arena.deinit();
    const value = std.json.parseFromSliceLeaky(std.json.Value, arena.allocator(), frame, .{}) catch
        return fail(ctx, "invalid frame JSON");
    const kind = frameString(value, "type") orelse return fail(ctx, "frame missing type");
    const frame_id_i = frameInt(value, "id") orelse 0;
    const frame_id: u64 = if (frame_id_i < 0) 0 else @intCast(frame_id_i);

    var deliver: Deliver = .none;
    var frame_record: ?[]u8 = null;
    var early_ok = false;
    var client_resp_id: u64 = 0;

    {
        ctx.shared.mutex.lock();
        defer ctx.shared.mutex.unlock();
        if (ctx.shared.generation != ctx.generation)
            return fail(ctx, "stale session frame");

        if (std.mem.eql(u8, kind, "start")) {
            const p = &(ctx.shared.protocol orelse return fail(ctx, "start without active request"));
            if (p.id != frame_id) return fail(ctx, "start id mismatch");
            if (p.phase != .await_start) return fail(ctx, "duplicate start");
            const chunks = frameInt(value, "chunks") orelse return fail(ctx, "start missing chunks");
            if (chunks < 0 or @as(u64, @intCast(chunks)) != p.expected_chunks)
                return fail(ctx, "start chunk count mismatch");
            const frequency = frameInt(value, "qpc_frequency") orelse return fail(ctx, "start missing qpc_frequency");
            if (frequency != qpcFrequency()) return fail(ctx, "qpc frequency mismatch");
            p.phase = .streaming;
        } else if (std.mem.eql(u8, kind, "chunk")) {
            const p = &(ctx.shared.protocol orelse return fail(ctx, "chunk without request"));
            if (p.id != frame_id) return fail(ctx, "chunk id mismatch");
            if (p.phase != .streaming) return fail(ctx, "chunk outside streaming");
            const seq_i = frameInt(value, "seq") orelse return fail(ctx, "chunk missing seq");
            if (seq_i < 0) return fail(ctx, "chunk seq out of order");
            const seq: u64 = @intCast(seq_i);
            if (seq != p.next_seq or seq >= p.expected_chunks)
                return fail(ctx, "chunk seq out of order");
            const text = frameString(value, "text") orelse return fail(ctx, "chunk missing text");
            const emit_qpc = frameString(value, "emit_qpc") orelse return fail(ctx, "chunk missing emit_qpc");
            if (emit_qpc.len != 20) return fail(ctx, "invalid emit_qpc");
            for (emit_qpc) |b| {
                if (!std.ascii.isDigit(b)) return fail(ctx, "invalid emit_qpc");
            }
            const emit_val = std.fmt.parseInt(i64, emit_qpc, 10) catch return fail(ctx, "invalid emit_qpc");
            if (emit_val <= 0) return fail(ctx, "invalid emit_qpc");
            p.next_seq += 1;

            const owned_text = gpa.dupe(u8, text) catch return fail(ctx, "chunk allocation failed");
            frame_record = buildFrameRecord(gpa, frame_id, seq, receipt_qpc, emit_qpc);
            if (frame_record == null) {
                gpa.free(owned_text);
                return fail(ctx, "record allocation failed");
            }
            deliver = .{ .chunk = .{ .id = frame_id, .seq = @intCast(seq), .text = owned_text } };
            if (frame_record == null) {
                gpa.free(owned_text);
                return fail(ctx, "record allocation failed");
            }
            deliver = .{ .chunk = .{ .id = frame_id, .seq = @intCast(seq), .text = owned_text } };
        } else if (std.mem.eql(u8, kind, "client_request")) {
            const p = &(ctx.shared.protocol orelse return fail(ctx, "client_request without request"));
            if (p.id != frame_id) return fail(ctx, "client_request id mismatch");
            if (p.phase != .streaming) return fail(ctx, "client_request outside streaming");
            const request_id = frameInt(value, "request_id") orelse return fail(ctx, "client_request missing request_id");
            const method = frameString(value, "method") orelse return fail(ctx, "client_request missing method");
            const params_ok = blk: {
                const params = value.object.get("params") orelse break :blk false;
                const v = frameString(params, "value") orelse break :blk false;
                break :blk std.mem.eql(u8, v, "ok");
            };
            if (request_id != 1 or !std.mem.eql(u8, method, "benchmark.confirm") or !params_ok)
                return fail(ctx, "unexpected client_request");
            if (p.client_request_seen) return fail(ctx, "duplicate client_request");
            p.client_request_seen = true;
            client_resp_id = p.id;
            early_ok = true; // special flow: unlock, notify, push work
        } else if (std.mem.eql(u8, kind, "complete")) {
            const p = &(ctx.shared.protocol orelse return fail(ctx, "complete without request"));
            if (p.id != frame_id) return fail(ctx, "complete id mismatch");
            if (p.phase != .streaming) return fail(ctx, "complete outside streaming");
            if (p.next_seq != p.expected_chunks) return fail(ctx, "premature complete");
            const t = p.claimTerminal() orelse return fail(ctx, "duplicate terminal");
            deliver = .{ .terminal = .{ .id = t.id, .kind = .complete, .last_seq = t.last_seq } };
        } else if (std.mem.eql(u8, kind, "cancelled")) {
            const shutdown_sent = ctx.shared.shutdown_sent or ctx.shared.shutdown_requested;
            const p = &(ctx.shared.protocol orelse return fail(ctx, "cancelled without request"));
            if (p.id != frame_id) return fail(ctx, "cancelled id mismatch");
            if (p.phase == .terminal or p.terminal_emitted) return fail(ctx, "duplicate terminal");
            if (!p.cancel_requested and !shutdown_sent) return fail(ctx, "cancelled without request");
            const last_seq = frameInt(value, "last_seq") orelse return fail(ctx, "cancelled missing last_seq");
            if (last_seq != @as(i64, @intCast(p.next_seq)) - 1) return fail(ctx, "cancelled last_seq mismatch");
            const t = p.claimTerminal() orelse return fail(ctx, "duplicate terminal");
            deliver = .{ .terminal = .{ .id = t.id, .kind = .cancelled, .last_seq = t.last_seq } };
        } else if (std.mem.eql(u8, kind, "shutdown_ack")) {
            if (!ctx.shared.shutdown_sent) return fail(ctx, "unsolicited shutdown_ack");
            if (ctx.shared.shutdown_ack) return fail(ctx, "duplicate shutdown_ack");
            ctx.shared.shutdown_ack = true;
        } else {
            ctx.diag.setf("unexpected frame type '{s}'", .{kind});
            return error.Rejected;
        }
    }
    ctx.shared.notify();

    if (early_ok) {
        // client_request path: schedule the client_response work item.
        ctx.work.tryPush(.{ .client_response = .{
            .generation = ctx.generation,
            .id = client_resp_id,
            .request_id = 1,
        } }) catch {
            ctx.shared.mutex.lock();
            const sd = ctx.shared.shutdown_requested;
            ctx.shared.mutex.unlock();
            if (!sd) {
                const rec = buildErrorRecord(ctx.gpa, "provider work queue overflow") catch null;
                if (rec) |r| {
                    if (ctx.records) |q| {
                        q.tryPush(r) catch ctx.gpa.free(r);
                    } else ctx.gpa.free(r);
                }
            }
            return fail(ctx, "provider work queue overflow");
        };
        return;
    }

    if (frame_record) |rec| {
        if (ctx.records) |q| {
            q.tryPush(rec) catch {
                gpa.free(rec);
                freeDeliver(gpa, deliver);
                return fail(ctx, "control output queue overflow");
            };
        } else {
            gpa.free(rec);
        }
    }

    switch (deliver) {
        .none => {},
        .chunk => |ch| {
            pushEvent(ctx.events, ctx.wake, .{ .chunk = .{
                .generation = ctx.generation,
                .id = ch.id,
                .seq = ch.seq,
                .text = ch.text,
                .receipt_qpc = receipt_qpc,
            } }, gpa) catch return fail(ctx, "provider-to-UI queue closed");
        },
        .terminal => |t| {
            pushEvent(ctx.events, ctx.wake, .{ .terminal = .{
                .generation = ctx.generation,
                .id = t.id,
                .kind = t.kind,
                .last_seq = t.last_seq,
            } }, gpa) catch return fail(ctx, "provider-to-UI queue closed");
        },
    }
}

fn freeDeliver(gpa: Allocator, d: Deliver) void {
    switch (d) {
        .chunk => |ch| gpa.free(ch.text),
        else => {},
    }
}

// ---------------------------------------------------------------- tests

const TestRig = struct {
    shared: Shared,
    events: *queue_mod.BoundedQueue(Event),
    records: *queue_mod.BoundedQueue([]u8),
    work: *queue_mod.BoundedQueue(Work),
    ctx: ReaderCtx,

    fn init(gpa: Allocator) !*TestRig {
        const rig = try gpa.create(TestRig);
        rig.events = try queue_mod.BoundedQueue(Event).init(gpa, 64);
        rig.records = try queue_mod.BoundedQueue([]u8).init(gpa, 256);
        rig.work = try queue_mod.BoundedQueue(Work).init(gpa, 16);
        rig.shared = .{};
        rig.ctx = .{
            .shared = &rig.shared,
            .events = rig.events,
            .records = rig.records,
            .work = rig.work,
            .wake = null,
            .generation = 1,
            .gpa = gpa,
        };
        return rig;
    }

    fn activate(self: *TestRig, id: u64, expected: u64) void {
        self.shared.mutex.lock();
        defer self.shared.mutex.unlock();
        self.shared.generation = 1;
        self.shared.protocol = .{ .id = id, .expected_chunks = expected };
    }

    fn feed(self: *TestRig, frame: []const u8) !void {
        return handleFrame(&self.ctx, frame, 7);
    }

    fn deinit(self: *TestRig, gpa: Allocator) void {
        while (self.events.pop()) |e| e.deinit(gpa);
        while (self.records.pop()) |r| gpa.free(r);
        while (self.work.pop()) |w| w.deinit(gpa);
        self.events.deinit(gpa);
        self.records.deinit(gpa);
        self.work.deinit(gpa);
        self.shared.clearFailed(gpa);
        gpa.destroy(self);
    }
};

fn startFrame(gpa: Allocator, id: u64, chunks: u64) ![]u8 {
    var w = jsonw.Writer.init(gpa);
    defer w.deinit();
    try w.beginObject(null);
    try w.uint("id", id);
    try w.uint("chunks", chunks);
    try w.int("qpc_frequency", qpcFrequency());
    try w.string("type", "start");
    try w.endObject();
    return w.owned();
}

fn chunkFrame(gpa: Allocator, id: u64, seq: u64, txt: []const u8) ![]u8 {
    var w = jsonw.Writer.init(gpa);
    defer w.deinit();
    try w.beginObject(null);
    try w.uint("id", id);
    try w.uint("seq", seq);
    try w.string("text", txt);
    try w.string("emit_qpc", "00000000000000000100");
    try w.string("type", "chunk");
    try w.endObject();
    return w.owned();
}

test "protocol: normal sequence accepted" {
    const gpa = std.testing.allocator;
    const rig = try TestRig.init(gpa);
    defer rig.deinit(gpa);
    rig.activate(7, 2);
    const sf = try startFrame(gpa, 7, 2);
    defer gpa.free(sf);
    try rig.feed(sf);
    const c1 = try chunkFrame(gpa, 7, 0, "a");
    defer gpa.free(c1);
    try rig.feed(c1);
    const c2 = try chunkFrame(gpa, 7, 1, "b");
    defer gpa.free(c2);
    try rig.feed(c2);
    try rig.feed("{\"type\":\"complete\",\"id\":7}");
    {
        rig.shared.mutex.lock();
        defer rig.shared.mutex.unlock();
        try std.testing.expectEqual(Phase.terminal, rig.shared.protocol.?.phase);
        try std.testing.expectEqual(@as(u64, 2), rig.shared.protocol.?.next_seq);
    }
    var terminal: usize = 0;
    var chunks: usize = 0;
    while (rig.events.pop()) |event| {
        switch (event) {
            .chunk => |e| {
                chunks += 1;
                try std.testing.expect(e.seq < 2);
                gpa.free(e.text);
            },
            .terminal => |e| {
                terminal += 1;
                try std.testing.expectEqual(EventKind.complete, e.kind);
                try std.testing.expectEqual(@as(i64, 1), e.last_seq);
            },
            else => {},
        }
    }
    try std.testing.expectEqual(@as(usize, 2), chunks);
    try std.testing.expectEqual(@as(usize, 1), terminal);
    if (rig.records.pop()) |r| gpa.free(r) else return error.TestUnexpected;
}

test "protocol: wrong id, dup seq, skipped seq rejected" {
    const gpa = std.testing.allocator;
    const rig = try TestRig.init(gpa);
    defer rig.deinit(gpa);
    rig.activate(7, 2);
    const sf = try startFrame(gpa, 7, 2);
    defer gpa.free(sf);
    try rig.feed(sf);
    const wrong = try chunkFrame(gpa, 8, 0, "a");
    defer gpa.free(wrong);
    try std.testing.expectError(error.Rejected, rig.feed(wrong));
}

test "protocol: duplicate and skipped seq rejected" {
    const gpa = std.testing.allocator;
    {
        const rig = try TestRig.init(gpa);
        defer rig.deinit(gpa);
        rig.activate(7, 2);
        const sf = try startFrame(gpa, 7, 2);
        defer gpa.free(sf);
        try rig.feed(sf);
        const c1 = try chunkFrame(gpa, 7, 0, "a");
        defer gpa.free(c1);
        try rig.feed(c1);
        try std.testing.expectError(error.Rejected, rig.feed(c1));
    }
    {
        const rig = try TestRig.init(gpa);
        defer rig.deinit(gpa);
        rig.activate(7, 2);
        const sf = try startFrame(gpa, 7, 2);
        defer gpa.free(sf);
        try rig.feed(sf);
        const c2 = try chunkFrame(gpa, 7, 1, "b");
        defer gpa.free(c2);
        try std.testing.expectError(error.Rejected, rig.feed(c2));
    }
}

test "protocol: premature complete and duplicate terminal rejected" {
    const gpa = std.testing.allocator;
    const rig = try TestRig.init(gpa);
    defer rig.deinit(gpa);
    rig.activate(7, 1);
    const sf = try startFrame(gpa, 7, 1);
    defer gpa.free(sf);
    try rig.feed(sf);
    try std.testing.expectError(error.Rejected, rig.feed("{\"type\":\"complete\",\"id\":7,\"dummy\":0}"));
    // after chunk it should succeed, then a second complete is a duplicate terminal
    const c1 = try chunkFrame(gpa, 7, 0, "a");
    defer gpa.free(c1);
    try rig.feed(c1);
    try rig.feed("{\"type\":\"complete\",\"id\":7}");
    try std.testing.expectError(error.Rejected, rig.feed("{\"type\":\"complete\",\"id\":7}"));
}

test "protocol: terminal claim once" {
    const gpa = std.testing.allocator;
    const rig = try TestRig.init(gpa);
    defer rig.deinit(gpa);
    rig.activate(7, 3);
    const sf = try startFrame(gpa, 7, 3);
    defer gpa.free(sf);
    try rig.feed(sf);
    const c1 = try chunkFrame(gpa, 7, 0, "a");
    defer gpa.free(c1);
    try rig.feed(c1);
    const c2 = try chunkFrame(gpa, 7, 1, "b");
    defer gpa.free(c2);
    try rig.feed(c2);
    {
        rig.shared.mutex.lock();
        defer rig.shared.mutex.unlock();
        const first = rig.shared.protocol.?.claimTerminal();
        try std.testing.expect(first != null);
        try std.testing.expectEqual(@as(u64, 7), first.?.id);
        try std.testing.expectEqual(@as(i64, 1), first.?.last_seq);
        try std.testing.expect(rig.shared.protocol.?.claimTerminal() == null);
    }
}

test "protocol: frequency mismatch rejected" {
    const gpa = std.testing.allocator;
    const rig = try TestRig.init(gpa);
    defer rig.deinit(gpa);
    rig.activate(7, 2);
    var w = jsonw.Writer.init(gpa);
    defer w.deinit();
    try w.beginObject(null);
    try w.uint("id", 7);
    try w.uint("chunks", 2);
    try w.int("qpc_frequency", 12345);
    try w.string("type", "start");
    try w.endObject();
    const frame = try w.owned();
    defer gpa.free(frame);
    try std.testing.expectError(error.Rejected, rig.feed(frame));
}

test "protocol: cancel permits terminal then fresh request" {
    const gpa = std.testing.allocator;
    const rig = try TestRig.init(gpa);
    defer rig.deinit(gpa);
    rig.activate(7, 2);
    const sf = try startFrame(gpa, 7, 2);
    defer gpa.free(sf);
    try rig.feed(sf);
    const c1 = try chunkFrame(gpa, 7, 0, "a");
    defer gpa.free(c1);
    try rig.feed(c1);
    {
        rig.shared.mutex.lock();
        rig.shared.protocol.?.cancel_requested = true;
        rig.shared.mutex.unlock();
    }
    try rig.feed("{\"type\":\"cancelled\",\"id\":7,\"last_seq\":0}");
    rig.activate(8, 1);
    const sf2 = try startFrame(gpa, 8, 1);
    defer gpa.free(sf2);
    try rig.feed(sf2);
    const c2 = try chunkFrame(gpa, 8, 0, "z");
    defer gpa.free(c2);
    try rig.feed(c2);
    try rig.feed("{\"type\":\"complete\",\"id\":8}");
    {
        rig.shared.mutex.lock();
        defer rig.shared.mutex.unlock();
        try std.testing.expectEqual(Phase.terminal, rig.shared.protocol.?.phase);
    }
}

test "protocol: cancelled without request rejected" {
    const gpa = std.testing.allocator;
    const rig = try TestRig.init(gpa);
    defer rig.deinit(gpa);
    rig.activate(7, 2);
    const sf = try startFrame(gpa, 7, 2);
    defer gpa.free(sf);
    try rig.feed(sf);
    const c1 = try chunkFrame(gpa, 7, 0, "a");
    defer gpa.free(c1);
    try rig.feed(c1);
    try std.testing.expectError(error.Rejected, rig.feed("{\"type\":\"cancelled\",\"id\":7,\"last_seq\":0}"));
}

test "protocol: client_request answered once" {
    const gpa = std.testing.allocator;
    const rig = try TestRig.init(gpa);
    defer rig.deinit(gpa);
    rig.activate(7, 1);
    const sf = try startFrame(gpa, 7, 1);
    defer gpa.free(sf);
    try rig.feed(sf);
    const request = "{\"type\":\"client_request\",\"id\":7,\"request_id\":1,\"method\":\"benchmark.confirm\",\"params\":{\"value\":\"ok\"}}";
    try rig.feed(request);
    const work_item = rig.work.pop();
    try std.testing.expect(work_item != null);
    switch (work_item.?) {
        .client_response => |cr| {
            try std.testing.expectEqual(@as(u64, 1), cr.generation);
            try std.testing.expectEqual(@as(u64, 7), cr.id);
            try std.testing.expectEqual(@as(u64, 1), cr.request_id);
        },
        else => return error.TestUnexpected,
    }
    try std.testing.expectError(error.Rejected, rig.feed(request));
}

test "protocol: client_request before start and after terminal rejected" {
    const gpa = std.testing.allocator;
    const rig = try TestRig.init(gpa);
    defer rig.deinit(gpa);
    rig.activate(7, 1);
    const request = "{\"type\":\"client_request\",\"id\":7,\"request_id\":1,\"method\":\"benchmark.confirm\",\"params\":{\"value\":\"ok\"}}";
    try std.testing.expectError(error.Rejected, rig.feed(request));
    const sf = try startFrame(gpa, 7, 1);
    defer gpa.free(sf);
    try rig.feed(sf);
    const c1 = try chunkFrame(gpa, 7, 0, "a");
    defer gpa.free(c1);
    try rig.feed(c1);
    try rig.feed("{\"type\":\"complete\",\"id\":7}");
    try std.testing.expectError(error.Rejected, rig.feed(request));
}

test "protocol: shutdown_ack requires sent and is once" {
    const gpa = std.testing.allocator;
    const rig = try TestRig.init(gpa);
    defer rig.deinit(gpa);
    {
        rig.shared.mutex.lock();
        rig.shared.generation = 1;
        rig.shared.mutex.unlock();
    }
    const ack = "{\"type\":\"shutdown_ack\"}";
    try std.testing.expectError(error.Rejected, rig.feed(ack));
    {
        rig.shared.mutex.lock();
        rig.shared.shutdown_sent = true;
        rig.shared.mutex.unlock();
    }
    try rig.feed(ack);
    try std.testing.expectError(error.Rejected, rig.feed(ack));
}

test "protocol: shutdown_ack accepted mid-stream when requested" {
    const gpa = std.testing.allocator;
    const rig = try TestRig.init(gpa);
    defer rig.deinit(gpa);
    rig.activate(7, 2);
    const sf = try startFrame(gpa, 7, 2);
    defer gpa.free(sf);
    try rig.feed(sf);
    const c1 = try chunkFrame(gpa, 7, 0, "a");
    defer gpa.free(c1);
    try rig.feed(c1);
    {
        rig.shared.mutex.lock();
        rig.shared.shutdown_sent = true;
        rig.shared.mutex.unlock();
    }
    try rig.feed("{\"type\":\"shutdown_ack\"}");
    rig.shared.mutex.lock();
    defer rig.shared.mutex.unlock();
    try std.testing.expect(rig.shared.shutdown_ack);
}
