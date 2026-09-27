const std = @import("std");
const win32 = @import("win32.zig");
const framing = @import("framing.zig");
const queue_mod = @import("queue.zig");
const jsonw = @import("json.zig");

const c = win32.c;
const Allocator = std.mem.Allocator;

/// A parsed control command. The arena backs the JSON value tree and is
/// owned by the queued message until the UI thread disposes it.
pub const ControlCmd = struct {
    arena: ?*std.heap.ArenaAllocator, // null for synthesized shutdown
    value: std.json.Value,

    pub fn deinit(self: ControlCmd, gpa: Allocator) void {
        if (self.arena) |arena| {
            arena.deinit();
            std.heap.smp_allocator.destroy(arena);
        } else {
            // Synthesized command: free the hand-built object map.
            var v = self.value;
            switch (v) {
                .object => |*obj| {
                    obj.deinit();
                },
                else => {},
            }
            _ = gpa;
        }
    }
};

pub const WM_APP_CONTROL_MSG: u32 = c.WM_APP + 2;
const WM_APP_CONTROL: c.UINT = c.WM_APP + 2;

pub const Control = struct {
    gpa: Allocator,
    commands: *queue_mod.BoundedQueue(ControlCmd),
    records: *queue_mod.BoundedQueue([]u8),
    reader: ?std.Thread = null,
    writer: ?std.Thread = null,
    wake: c.HWND,
    stop_flag: std.atomic.Value(bool) = std.atomic.Value(bool).init(false),

    pub fn arm(
        gpa: Allocator,
        commands: *queue_mod.BoundedQueue(ControlCmd),
        records: *queue_mod.BoundedQueue([]u8),
        wake: c.HWND,
    ) !*Control {
        const ctl = try gpa.create(Control);
        ctl.* = .{ .gpa = gpa, .commands = commands, .records = records, .wake = wake };
        ctl.writer = std.Thread.spawn(.{}, writerRun, .{ctl}) catch null;
        ctl.reader = std.Thread.spawn(.{}, readerRun, .{ctl}) catch null;
        return ctl;
    }

    /// Bounded teardown: close queues, cancel the blocked stdin read, join.
    pub fn stop(self: *Control) void {
        self.stop_flag.store(true, .seq_cst);
        self.commands.close();
        const deadline = win32.nowMs() + 2000;
        if (self.writer) |t| {
            _ = win32.joinBounded(t, deadline, false);
        }
        if (self.reader) |t| {
            _ = win32.joinBounded(t, deadline, true);
        }
    }

    pub fn deinit(self: *Control) void {
        self.gpa.destroy(self);
    }
};

fn emitErrorRecord(ctl: *Control, msg: []const u8) void {
    var w = jsonw.Writer.init(ctl.gpa);
    w.beginObject(null) catch {
        w.deinit();
        return;
    };
    w.rawField("token", "null") catch {};
    w.boolean("ok", false) catch {};
    w.string("error", msg) catch {};
    w.endObject() catch {};
    const bytes = w.owned() catch {
        w.deinit();
        return;
    };
    ctl.records.tryPush(bytes) catch ctl.gpa.free(bytes);
}

fn writerRun(ctl: *Control) void {
    const stdout = std.fs.File.stdout();
    while (ctl.records.waitPop()) |record| {
        stdout.writeAll(record) catch {};
        stdout.writeAll("\n") catch {};
        ctl.gpa.free(record);
    }
}

fn requestShutdown(ctl: *Control) void {
    // Synthesize a shutdown command (token null, like the Rust control layer).
    var map = std.json.ObjectMap.init(ctl.gpa);
    map.put("command", .{ .string = "shutdown" }) catch {};
    map.put("token", .null) catch {};
    const cmd = ControlCmd{ .arena = null, .value = .{ .object = map } };
    ctl.commands.push(cmd) catch {
        cmd.deinit(ctl.gpa);
        return;
    };
    _ = c.PostMessageW(ctl.wake, WM_APP_CONTROL, 0, 0);
}

fn readerRun(ctl: *Control) void {
    const gpa = ctl.gpa;
    const stdin_handle = c.GetStdHandle(c.STD_INPUT_HANDLE);
    var decoder = framing.Decoder.init(gpa) catch return;
    defer decoder.deinit();
    var buf: [8192]u8 = undefined;
    while (true) {
        if (ctl.stop_flag.load(.seq_cst)) break;
        var amount: c.DWORD = 0;
        const ok = c.ReadFile(stdin_handle, &buf, buf.len, &amount, null);
        if (ok == 0) {
            // Read failure (including a teardown-time CancelSynchronousIo abort).
            if (ctl.stop_flag.load(.seq_cst)) break;
            emitErrorRecord(ctl, "control input read failed");
            requestShutdown(ctl);
            break;
        }
        const n: usize = @intCast(amount);
        if (n == 0) {
            decoder.finish() catch {
                emitErrorRecord(ctl, "incomplete control frame");
            };
            requestShutdown(ctl);
            break;
        }
        decoder.feed(buf[0..n], zeroClock, ctl, controlFrame) catch |err| {
            const detail = switch (err) {
                error.FrameTooLarge => "logical frame exceeds 65536-byte limit",
                error.InvalidUtf8 => "invalid frame UTF-8",
                error.FrameRejected => "invalid control JSON",
                else => "decoder session already failed",
            };
            var msgbuf: [160]u8 = undefined;
            const msg = std.fmt.bufPrint(&msgbuf, "invalid control frame: {s}", .{detail}) catch
                "invalid control frame";
            emitErrorRecord(ctl, msg);
            requestShutdown(ctl);
            break;
        };
    }
}

fn zeroClock() i64 {
    return 0;
}

fn controlFrame(ctl: *Control, frame: []const u8, _: i64) !void {
    const arena = std.heap.smp_allocator.create(std.heap.ArenaAllocator) catch
        return error.OutOfMemory;
    arena.* = std.heap.ArenaAllocator.init(std.heap.smp_allocator);
    const value = std.json.parseFromSliceLeaky(std.json.Value, arena.allocator(), frame, .{}) catch {
        arena.deinit();
        std.heap.smp_allocator.destroy(arena);
        return error.InvalidJson;
    };
    ctl.commands.push(.{ .arena = arena, .value = value }) catch {
        const cmd = ControlCmd{ .arena = arena, .value = value };
        cmd.deinit(ctl.gpa);
        return error.Closed;
    };
    _ = c.PostMessageW(ctl.wake, WM_APP_CONTROL, 0, 0);
}
