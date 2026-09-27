const std = @import("std");

const Allocator = std.mem.Allocator;

pub const ConfigError = error{
    ManifestRead,
    ManifestParse,
    ManifestInvalid,
    AssetHashMismatch,
    HistoryRead,
    HistoryInvalid,
    OutOfMemory,
    Unexpected,
};

pub const Config = struct {
    arena: std.heap.ArenaAllocator,
    manifest: std.json.Value, // object; memory owned by arena
    benchmark_root: []const u8, // absolute
    asset_path: []const u8, // absolute
    asset_pixel_w: u64,
    asset_pixel_h: u64,
    history_prefix: []u8, // logical LF text: joined messages + separator

    pub fn deinit(self: *Config) void {
        self.arena.deinit();
    }
};

fn sha256Hex(gpa: Allocator, data: []const u8) ![]u8 {
    var digest: [32]u8 = undefined;
    std.crypto.hash.sha2.Sha256.hash(data, &digest, .{});
    const hex_chars = "0123456789abcdef";
    const out = try gpa.alloc(u8, 64);
    for (digest, 0..) |b, i| {
        out[i * 2] = hex_chars[b >> 4];
        out[i * 2 + 1] = hex_chars[b & 0xf];
    }
    return out;
}

fn getField(v: std.json.Value, name: []const u8) ?std.json.Value {
    if (v != .object) return null;
    return v.object.get(name);
}

fn getStr(v: std.json.Value, name: []const u8) ?[]const u8 {
    const f = getField(v, name) orelse return null;
    if (f != .string) return null;
    return f.string;
}

fn getInt(v: std.json.Value, name: []const u8) ?i64 {
    const f = getField(v, name) orelse return null;
    if (f != .integer) return null;
    return f.integer;
}

fn getBool(v: std.json.Value, name: []const u8) ?bool {
    const f = getField(v, name) orelse return null;
    if (f != .bool) return null;
    return f.bool;
}

fn getObj(v: std.json.Value, name: []const u8) ?std.json.Value {
    const f = getField(v, name) orelse return null;
    if (f != .object) return null;
    return f;
}

/// Expected chunk count for a frozen scenario name; callers validate the
/// name against manifest.scenarios before use.
pub fn expectedChunks(manifest: std.json.Value, scenario: []const u8) u64 {
    const normal: u64 = blk: {
        const proto = getObj(manifest, "protocol") orelse break :blk 100;
        const n = getInt(proto, "normal_chunks") orelse break :blk 100;
        break :blk if (n < 0) 100 else @intCast(n);
    };
    if (std.mem.eql(u8, scenario, "backpressure")) return 256;
    if (std.mem.eql(u8, scenario, "maximum") or std.mem.eql(u8, scenario, "oversized")) return 1;
    return normal;
}

pub fn scenarioKnown(manifest: std.json.Value, name: []const u8) bool {
    const scenarios = getObj(manifest, "scenarios") orelse return false;
    return scenarios.object.get(name) != null;
}

pub fn load(gpa: Allocator, manifest_arg: []const u8) !*Config {
    var arena = std.heap.ArenaAllocator.init(gpa);
    errdefer arena.deinit();
    const aa = arena.allocator();

    const manifest_path = std.fs.cwd().realpathAlloc(aa, manifest_arg) catch
        return error.ManifestRead;
    const data = std.fs.cwd().readFileAlloc(aa, manifest_path, 4 << 20) catch
        return error.ManifestRead;
    const parsed = std.json.parseFromSliceLeaky(std.json.Value, aa, data, .{}) catch
        return error.ManifestParse;
    if (parsed != .object) return error.ManifestParse;

    const manifest_dir = std.fs.path.dirname(manifest_path) orelse return error.ManifestInvalid;
    const root = std.fs.path.dirname(manifest_dir) orelse return error.ManifestInvalid;

    const asset = getObj(parsed, "asset") orelse return error.ManifestInvalid;
    const asset_pixel_w = getInt(asset, "pixel_width") orelse 0;
    const asset_pixel_h = getInt(asset, "pixel_height") orelse 0;
    if (asset_pixel_w <= 0 or asset_pixel_h <= 0 or
        (getInt(asset, "logical_width_dip") orelse 0) != 64 or
        (getInt(asset, "logical_height_dip") orelse 0) != 64)
        return error.ManifestInvalid;
    const asset_rel = getStr(asset, "path") orelse return error.ManifestInvalid;
    const asset_sha = getStr(asset, "sha256") orelse return error.ManifestInvalid;

    const proto = getObj(parsed, "protocol") orelse return error.ManifestInvalid;
    if ((getInt(proto, "max_stdout_frame_bytes_including_lf") orelse 0) != 65536 or
        (getInt(proto, "max_stdin_frame_bytes_including_lf") orelse 0) != 65536)
        return error.ManifestInvalid;
    const cancel_timeout_ms = getInt(proto, "cancel_timeout_ms") orelse return error.ManifestInvalid;
    const shutdown_timeout_ms = getInt(proto, "shutdown_timeout_ms") orelse return error.ManifestInvalid;
    _ = cancel_timeout_ms;
    _ = shutdown_timeout_ms;

    const provider = getObj(parsed, "provider") orelse return error.ManifestInvalid;
    if (getBool(provider, "inherit_environment") orelse true)
        return error.ManifestInvalid;
    _ = getStr(provider, "path") orelse return error.ManifestInvalid;
    _ = getObj(provider, "environment") orelse return error.ManifestInvalid;
    const prov_args = getField(provider, "arguments") orelse return error.ManifestInvalid;
    if (prov_args != .array) return error.ManifestInvalid;

    const version = getStr(parsed, "version") orelse return error.ManifestInvalid;
    if (!std.mem.startsWith(u8, version, "windows-v")) return error.ManifestInvalid;

    const ui = getObj(parsed, "ui") orelse return error.ManifestInvalid;
    if ((getInt(ui, "composer_client_width_dip") orelse 0) != 640 or
        (getInt(ui, "composer_client_height_dip") orelse 0) != 480 or
        (getInt(ui, "input_height_dip") orelse 0) != 128 or
        (getInt(ui, "response_height_dip") orelse 0) != 272 or
        (getInt(ui, "margin_dip") orelse 0) != 12)
        return error.ManifestInvalid;
    const hotkey = getStr(ui, "hotkey") orelse "";
    const submit = getStr(ui, "submit") orelse "";
    const cancel = getStr(ui, "cancel") orelse "";
    if (!std.mem.eql(u8, hotkey, "CTRL+ALT+SPACE") or
        !std.mem.eql(u8, submit, "CTRL+ENTER") or
        !std.mem.eql(u8, cancel, "CTRL+ALT+ESCAPE"))
        return error.ManifestInvalid;
    if (getBool(ui, "hide_cancels_request") orelse true)
        return error.ManifestInvalid;
    const history_messages = getInt(ui, "history_messages") orelse return error.ManifestInvalid;

    const scenarios = getObj(parsed, "scenarios") orelse return error.ManifestInvalid;
    for ([_][]const u8{
        "normal",    "cancel",          "client_request", "backpressure", "maximum",
        "oversized", "unexpected_exit", "stderr",         "fragmented",
    }) |required| {
        if (scenarios.object.get(required) == null) return error.ManifestInvalid;
    }

    const asset_path = std.fs.path.resolve(aa, &.{ root, asset_rel }) catch
        return error.ManifestInvalid;
    const asset_data = std.fs.cwd().readFileAlloc(aa, asset_path, 8 << 20) catch
        return error.ManifestInvalid;
    const actual = sha256Hex(aa, asset_data) catch return error.ManifestInvalid;
    if (!std.ascii.eqlIgnoreCase(actual, asset_sha))
        return error.AssetHashMismatch;

    const history_path = std.fs.path.resolve(aa, &.{ root, "fixtures/history.json" }) catch
        return error.ManifestInvalid;
    const history_data = std.fs.cwd().readFileAlloc(aa, history_path, 1 << 20) catch
        return error.HistoryRead;
    const history = std.json.parseFromSliceLeaky(std.json.Value, aa, history_data, .{}) catch
        return error.HistoryInvalid;
    if (history != .array) return error.HistoryInvalid;
    if (history.array.items.len != @as(usize, @intCast(history_messages)))
        return error.HistoryInvalid;
    var prefix: std.ArrayList(u8) = .empty;
    for (history.array.items, 0..) |item, i| {
        if (item != .string) return error.HistoryInvalid;
        if (i != 0) try prefix.append(aa, '\n');
        try prefix.appendSlice(aa, item.string);
    }
    try prefix.appendSlice(aa, "\n\n");

    const cfg = try aa.create(Config);
    cfg.* = .{
        .arena = arena,
        .manifest = parsed,
        .benchmark_root = root,
        .asset_path = asset_path,
        .asset_pixel_w = @intCast(asset_pixel_w),
        .asset_pixel_h = @intCast(asset_pixel_h),
        .history_prefix = try prefix.toOwnedSlice(aa),
    };
    return cfg;
}

test "sha256 known vectors" {
    const gpa = std.testing.allocator;
    const empty = try sha256Hex(gpa, "");
    defer gpa.free(empty);
    try std.testing.expectEqualStrings(
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        empty,
    );
    const abc = try sha256Hex(gpa, "abc");
    defer gpa.free(abc);
    try std.testing.expectEqualStrings(
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        abc,
    );
}
