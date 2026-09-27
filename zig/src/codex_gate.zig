// Untimed Codex app-server compatibility gate (benchmark protocol §16).
// Launches a pinned `codex app-server` over stdio through the same
// explicit-environment process machinery the provider path uses, completes
// JSON-RPC initialization, performs one read-only streamed interaction, and
// tears the process down cleanly. Emits gate evidence as JSON lines on
// stdout; exit code 0 = PASS, 1 = FAIL.

const std = @import("std");
const queue = @import("queue.zig");
const Allocator = std.mem.Allocator;

fn gateLine(step: []const u8, detail: []const u8) void {
    var buf: [1024]u8 = undefined;
    var len: usize = 0;
    for (detail) |c| {
        if (len >= buf.len) break;
        buf[len] = if (c == '"' or c == '\\' or c < ' ') ' ' else c;
        len += 1;
    }
    const stdout = std.fs.File.stdout();
    stdout.writeAll("{\"event\":\"codex_gate\",\"step\":\"") catch {};
    stdout.writeAll(step) catch {};
    stdout.writeAll("\",\"detail\":\"") catch {};
    stdout.writeAll(buf[0..len]) catch {};
    stdout.writeAll("\"}\n") catch {};
}

const LineMsg = struct { text: ?[]u8 }; // null = eof/error

fn stdoutPump(gpa: Allocator, file: std.fs.File, out: *queue.BoundedQueue(LineMsg)) void {
    defer out.close();
    var pending: std.ArrayList(u8) = .empty;
    defer pending.deinit(gpa);
    var chunk: [4096]u8 = undefined;
    while (true) {
        const n = file.read(&chunk) catch {
            _ = out.push(.{ .text = null }) catch {};
            return;
        };
        if (n == 0) {
            _ = out.push(.{ .text = null }) catch {};
            return;
        }
        for (chunk[0..n]) |c| {
            if (c == '\n') {
                const line = gpa.dupe(u8, std.mem.trimRight(u8, pending.items, "\r")) catch
                    return;
                pending.clearRetainingCapacity();
                out.push(.{ .text = line }) catch return;
            } else {
                pending.append(gpa, c) catch return;
            }
        }
    }
}

fn stderrDrain(file: std.fs.File, done: *std.atomic.Value(u64)) void {
    var chunk: [4096]u8 = undefined;
    var total: u64 = 0;
    while (file.read(&chunk)) |n| {
        if (n == 0) break;
        total += n;
    } else |_| {}
    done.store(total, .release);
}

pub fn run(gpa: Allocator, codex_exe: []const u8) !u8 {
    gate(codex_exe, gpa) catch |e| {
        gateLine("result", "FAIL");
        std.debug.print("codex-gate: {s}\n", .{@errorName(e)});
        return 1;
    };
    gateLine("result", "PASS");
    return 0;
}

fn gate(codex_exe: []const u8, gpa: Allocator) !void {
    const profile = std.process.getEnvVarOwned(gpa, "USERPROFILE") catch "";
    const homepath = if (std.mem.startsWith(u8, profile, "C:")) profile[2..] else profile;
    const temp = std.process.getEnvVarOwned(gpa, "TEMP") catch "C:\\Windows\\Temp";

    var env = std.process.EnvMap.init(gpa);
    defer env.deinit();
    try env.put("PATH", "C:\\Windows\\System32;C:\\Windows");
    try env.put("SystemRoot", "C:\\Windows");
    try env.put("WINDIR", "C:\\Windows");
    try env.put("TEMP", temp);
    try env.put("TMP", temp);
    try env.put("USERPROFILE", profile);
    try env.put("HOMEDRIVE", "C:");
    try env.put("HOMEPATH", homepath);

    const cwd = std.fs.path.dirname(codex_exe);
    var argv = [_][]const u8{ codex_exe, "app-server" };
    var child = std.process.Child.init(&argv, gpa);
    child.cwd = cwd;
    child.env_map = &env;
    child.stdin_behavior = .Pipe;
    child.stdout_behavior = .Pipe;
    child.stderr_behavior = .Pipe;
    child.create_no_window = true;
    try child.spawn();
    gateLine("spawn", "codex app-server started");

    const stdin = child.stdin orelse return error.StdinMissing;
    const stdout = child.stdout orelse return error.StdoutMissing;
    const stderr = child.stderr orelse return error.StderrMissing;

    const lines = try queue.BoundedQueue(LineMsg).init(gpa, 64);
    defer lines.deinit(gpa);
    const pump_thread = try std.Thread.spawn(.{}, stdoutPump, .{ gpa, stdout, lines });
    var stderr_total = std.atomic.Value(u64).init(0);
    const drain_thread = try std.Thread.spawn(.{}, stderrDrain, .{ stderr, &stderr_total });

    const frames = [_][]const u8{
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"clientInfo\":{\"name\":\"mascot-gate\",\"version\":\"0.1\"}}}",
        "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}",
        "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"config/read\",\"params\":{}}",
        "{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"thread/list\",\"params\":{}}",
    };
    for (frames) |frame| {
        try stdin.writeAll(frame);
        try stdin.writeAll("\n");
    }

    var answered = [_]bool{ false, false, false, false };
    var received: u64 = 0;
    var notifications: u64 = 0;
    const deadline = std.time.milliTimestamp() + 30_000;
    while (!(answered[1] and answered[2] and answered[3])) {
        if (std.time.milliTimestamp() >= deadline) return error.ReplyTimeout;
        const msg = lines.pop() orelse {
            std.Thread.sleep(5 * std.time.ns_per_ms);
            continue;
        };
        const line = msg.text orelse return error.StdoutEof;
        defer gpa.free(line);
        received += 1;
        for (1..4) |id| {
            var marker_buf: [16]u8 = undefined;
            const marker = std.fmt.bufPrint(&marker_buf, "\"id\":{d}", .{id}) catch continue;
            if (std.mem.indexOf(u8, line, marker) != null) {
                if (std.mem.indexOf(u8, line, "\"error\"") != null)
                    return error.ServerError;
                answered[id] = true;
                var detail_buf: [48]u8 = undefined;
                const detail = std.fmt.bufPrint(&detail_buf, "id {d} answered", .{id}) catch "";
                gateLine("reply", detail);
            }
        }
        if (std.mem.indexOf(u8, line, "\"id\"") == null) notifications += 1;
    }
    {
        var detail_buf: [128]u8 = undefined;
        const detail = std.fmt.bufPrint(&detail_buf,
            "config/read + thread/list answered; {d} frames, {d} unsolicited notifications",
            .{ received, notifications }) catch "";
        gateLine("interaction", detail);
    }

    // Clean teardown: close stdin, allow graceful exit, terminate on timeout.
    stdin.close();
    child.stdin = null;
    var wait_done = std.atomic.Value(bool).init(false);
    const waiter = try std.Thread.spawn(.{}, struct {
        fn wait(c: *std.process.Child, done: *std.atomic.Value(bool)) void {
            _ = c.wait() catch {};
            done.store(true, .release);
        }
    }.wait, .{ &child, &wait_done });
    const wait_deadline = std.time.milliTimestamp() + 5_000;
    while (!wait_done.load(.acquire)) {
        if (std.time.milliTimestamp() >= wait_deadline) {
            _ = child.kill() catch {};
            break;
        }
        std.Thread.sleep(20 * std.time.ns_per_ms);
    }
    waiter.join();
    pump_thread.join();
    drain_thread.join();
    gateLine("teardown", if (wait_done.load(.acquire)) "exited after stdin close"
        else "terminated after stdin close timeout");
    var detail_buf: [64]u8 = undefined;
    const detail = std.fmt.bufPrint(&detail_buf, "stderr drained {d} bytes",
        .{stderr_total.load(.acquire)}) catch "";
    gateLine("teardown", detail);
}
