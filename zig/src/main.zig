const std = @import("std");
const win32 = @import("win32.zig");
const framing = @import("framing.zig");
const queue_mod = @import("queue.zig");
const config = @import("config.zig");
const provider = @import("provider.zig");
const control = @import("control.zig");
const platform = @import("platform.zig");
const jsonw = @import("json.zig");

const c = win32.c;
const Allocator = std.mem.Allocator;

comptime {
    _ = framing;
    _ = queue_mod;
    _ = config;
    _ = provider;
    _ = jsonw;
}

const DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2: c.DPI_AWARENESS_CONTEXT = @ptrFromInt(@as(usize, @bitCast(@as(isize, -4))));

fn eprintln(comptime fmt: []const u8, args: anytype) void {
    std.debug.print(fmt ++ "\n", args);
}

pub fn main() u8 {
    const gpa = std.heap.smp_allocator;
    const args = std.process.argsAlloc(gpa) catch return 64;
    defer std.process.argsFree(gpa, args);
    var fixture: ?[]const u8 = null;
    var vectors: ?[]const u8 = null;
    var control_mode = false;
    var i: usize = 1;
    while (i < args.len) : (i += 1) {
        if (std.mem.eql(u8, args[i], "--fixture") and i + 1 < args.len) {
            i += 1;
            fixture = args[i];
        } else if (std.mem.eql(u8, args[i], "--decode-vectors") and i + 1 < args.len) {
            i += 1;
            vectors = args[i];
        } else if (std.mem.eql(u8, args[i], "--control")) {
            control_mode = true;
        }
    }
    if (vectors) |path| {
        const code = runDecodeVectors(gpa, path) catch |e| {
            eprintln("decode-vectors failed: {s}", .{@errorName(e)});
            return 64;
        };
        return code;
    }
    const fixture_path = fixture orelse {
        eprintln("missing --fixture", .{});
        return 64;
    };
    return runApp(gpa, fixture_path, control_mode) catch |e| {
        eprintln("fatal: {s}", .{@errorName(e)});
        return 64;
    };
}

// ------------------------------------------------------ decode vectors

fn runDecodeVectors(gpa: Allocator, path: []const u8) !u8 {
    const data = std.fs.cwd().readFileAlloc(gpa, path, 64 << 20) catch return error.VectorFile;
    defer gpa.free(data);
    var arena = std.heap.ArenaAllocator.init(gpa);
    defer arena.deinit();
    const parsed = std.json.parseFromSliceLeaky(std.json.Value, arena.allocator(), data, .{}) catch
        return error.VectorFile;
    if (parsed != .array) return error.VectorFile;
    const stdout = std.fs.File.stdout();
    for (parsed.array.items) |row| {
        const name = blk: {
            if (row != .object) break :blk "";
            const f = row.object.get("name") orelse break :blk "";
            if (f != .string) break :blk "";
            break :blk f.string;
        };
        const fragments = blk: {
            if (row != .object) break :blk @as(std.json.Value, .{ .array = std.json.Array.init(arena.allocator()) });
            const f = row.object.get("fragments_base64") orelse break :blk @as(std.json.Value, .{ .array = std.json.Array.init(arena.allocator()) });
            break :blk f;
        };

        var decoder = framing.Decoder.init(gpa) catch return error.OutOfMemory;
        var frames: std.ArrayList([]const u8) = .empty;
        defer frames.deinit(gpa);
        defer for (frames.items) |f| gpa.free(@constCast(f));
        var rejected = false;
        if (fragments == .array) {
            for (fragments.array.items) |frag| {
                if (rejected) break;
                if (frag != .string) continue;
                const enc = std.base64.standard.Decoder;
                const n = enc.calcSizeForSlice(frag.string) catch continue;
                const raw = gpa.alloc(u8, n) catch return error.OutOfMemory;
                defer gpa.free(raw);
                enc.decode(raw, frag.string) catch continue;
                const Ctx = struct {
                    frames: *std.ArrayList([]const u8),
                    gpa: Allocator,
                };
                var ctx = Ctx{ .frames = &frames, .gpa = gpa };
                decoder.feed(raw, vectorClock, &ctx, collectFrame) catch {
                    rejected = true;
                    break;
                };
            }
        }
        if (!rejected) {
            decoder.finish() catch {
                rejected = true;
            };
        }
        const peak = decoder.peakBufferBytes();
        decoder.deinit();

        var w = jsonw.Writer.init(gpa);
        defer w.deinit();
        try w.beginObject(null);
        try w.string("name", name);
        if (frames.items.len == 0) {
            try w.nullField("frames_base64");
        } else {
            try w.beginArray("frames_base64");
            const enc = std.base64.standard.Encoder;
            for (frames.items) |f| {
                const b64 = gpa.alloc(u8, enc.calcSize(f.len)) catch return error.OutOfMemory;
                defer gpa.free(b64);
                _ = enc.encode(b64, f);
                try w.string(null, b64);
            }
            try w.endArray();
        }
        try w.boolean("rejected", rejected);
        try w.uint("peak_buffer_bytes", peak);
        try w.endObject();
        const line = try w.owned();
        defer gpa.free(line);
        try stdout.writeAll(line);
        try stdout.writeAll("\n");
    }
    return 0;
}

fn vectorClock() i64 {
    return 0;
}

fn collectFrame(ctx: anytype, frame: []const u8, _: i64) !void {
    try ctx.frames.append(ctx.gpa, try ctx.gpa.dupe(u8, frame));
}

// ------------------------------------------------------------ run app

fn runApp(gpa: Allocator, fixture_path: []const u8, control_mode: bool) !u8 {
    _ = c.SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    win32.clearStdInheritFlags();
    _ = c.CoInitializeEx(null, c.COINIT_APARTMENTTHREADED);
    defer _ = c.CoUninitialize();

    const cfg = config.load(gpa, fixture_path) catch |e| {
        eprintln("config load failed: {s}", .{@errorName(e)});
        return error.ConfigFailed;
    };
    defer cfg.deinit();

    const decoded = platform.decodePng(gpa, cfg.asset_path) catch |e| {
        eprintln("asset decode failed: {s}", .{@errorName(e)});
        return error.AssetFailed;
    };
    defer gpa.free(decoded.pixels);
    const mascot_source = decoded.pixels;
    if (decoded.w != cfg.asset_pixel_w or decoded.h != cfg.asset_pixel_h) {
        eprintln("decoded asset dimensions differ from the manifest", .{});
        return error.AssetFailed;
    }

    const events = try queue_mod.BoundedQueue(provider.Event).init(gpa, 64);
    defer events.deinit(gpa);
    const commands = try queue_mod.BoundedQueue(control.ControlCmd).init(gpa, 16);
    defer commands.deinit(gpa);
    const records: ?*queue_mod.BoundedQueue([]u8) = if (control_mode)
        try queue_mod.BoundedQueue([]u8).init(gpa, 256)
    else
        null;
    defer if (records) |q| q.deinit(gpa);

    const ui = try gpa.create(platform.Ui);
    defer gpa.destroy(ui);
    ui.* = .{
        .gpa = gpa,
        .config = cfg,
        .mascot_source = mascot_source,
        .mascot_src_w = decoded.w,
        .mascot_src_h = decoded.h,
        .model = .{ .scenario = try gpa.dupe(u8, "normal") },
        .ui_events = events,
        .commands = commands,
        .records = records,
        .control_enabled = control_mode,
    };

    try platform.registerClasses();
    try platform.createMascot(ui);

    if (c.RegisterHotKey(ui.mascot, platform.HOTKEY_ID_SHOW, c.MOD_CONTROL | c.MOD_ALT | c.MOD_NOREPEAT, c.VK_SPACE) == 0)
        return error.HotkeyFailed;
    if (c.RegisterHotKey(ui.mascot, platform.HOTKEY_ID_CANCEL, c.MOD_CONTROL | c.MOD_ALT | c.MOD_NOREPEAT, c.VK_ESCAPE) == 0)
        return error.HotkeyFailed;
    defer {
        _ = c.UnregisterHotKey(ui.mascot, platform.HOTKEY_ID_SHOW);
        _ = c.UnregisterHotKey(ui.mascot, platform.HOTKEY_ID_CANCEL);
    }

    // Build provider config from the manifest.
    const prov = cfg.manifest.object.get("provider").?.object;
    const proto = cfg.manifest.object.get("protocol").?.object;
    const cancel_timeout: u64 = @intCast(proto.get("cancel_timeout_ms").?.integer);
    const shutdown_timeout: u64 = @intCast(proto.get("shutdown_timeout_ms").?.integer);
    const prov_path = prov.get("path").?.string;
    const prov_cwd = prov.get("cwd").?.string;
    const args_arr = prov.get("arguments").?.array;
    var argv_list: std.ArrayList([]const u8) = .empty;
    defer argv_list.deinit(gpa);
    for (args_arr.items) |a| {
        if (a != .string) return error.ConfigFailed;
        try argv_list.append(gpa, a.string);
    }
    const env_obj = prov.get("environment").?.object;
    var env_list: std.ArrayList(provider.ProviderConfig.EnvPair) = .empty;
    defer env_list.deinit(gpa);
    var env_it = env_obj.iterator();
    while (env_it.next()) |entry| {
        if (entry.value_ptr.* != .string) return error.ConfigFailed;
        try env_list.append(gpa, .{ .key = entry.key_ptr.*, .value = entry.value_ptr.string });
    }
    const pcfg = provider.ProviderConfig{
        .path = prov_path,
        .arguments = argv_list.items,
        .cwd = prov_cwd,
        .environment = env_list.items,
        .manifest = cfg.manifest,
        .cancel_timeout_ms = cancel_timeout,
        .shutdown_timeout_ms = shutdown_timeout,
    };
    const prov_inst = try provider.Provider.spawn(gpa, pcfg, events, records, ui.mascot);
    ui.provider = prov_inst;
    defer prov_inst.deinit();

    var ctl: ?*control.Control = null;
    if (control_mode) {
        ctl = try control.Control.arm(gpa, commands, records.?, ui.mascot);
    }
    defer if (ctl) |ctl_| ctl_.deinit();

    // Message loop.
    var msg: c.MSG = undefined;
    var exit_code: u8 = 0;
    while (true) {
        const r = c.GetMessageW(&msg, null, 0, 0);
        if (r == 0) {
            exit_code = @intCast(@as(u32, @truncate(@as(u64, @bitCast(msg.wParam)))));
            break;
        }
        if (r < 0) {
            eprintln("GetMessageW failed", .{});
            platform.requestShutdown(ui, null);
            exit_code = 64;
            break;
        }
        if (consumeSubmitKey(ui, &msg)) continue;
        _ = c.TranslateMessage(&msg);
        _ = c.DispatchMessageW(&msg);
    }

    if (ctl) |ctl_| ctl_.stop();
    platform.teardown(ui);
    return exit_code;
}

fn consumeSubmitKey(ui: *platform.Ui, msg: *const c.MSG) bool {
    if (msg.message != c.WM_KEYDOWN or msg.wParam != c.VK_RETURN) return false;
    if (ui.composer == null) return false;
    const comp = ui.composer.?;
    if (comp.input == null or msg.hwnd != comp.input) return false;
    if ((@as(u16, @bitCast(c.GetKeyState(c.VK_CONTROL))) & 0x8000) == 0) return false;
    if (ui.composing) return false;
    _ = c.PostMessageW(ui.mascot, platform.WM_APP_SUBMIT, 0, 0);
    return true;
}
