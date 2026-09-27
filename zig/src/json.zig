const std = @import("std");
const Allocator = std.mem.Allocator;

/// Deterministic JSON record writer. Objects/arrays are tracked with a
/// "first element" stack so callers never manage separators.
pub const Writer = struct {
    out: std.ArrayList(u8) = .empty,
    gpa: std.mem.Allocator,
    stack: [32]bool = undefined,
    depth: usize = 0,

    pub fn init(gpa: std.mem.Allocator) Writer {
        return .{ .gpa = gpa };
    }

    pub fn deinit(self: *Writer) void {
        self.out.deinit(self.gpa);
    }

    pub fn owned(self: *Writer) ![]u8 {
        return self.out.toOwnedSlice(self.gpa);
    }

    fn elem(self: *Writer) Allocator.Error!void {
        if (self.depth > 0 and !self.stack[self.depth - 1]) {
            try self.out.append(self.gpa, ',');
        }
        if (self.depth > 0) self.stack[self.depth - 1] = false;
    }

    pub fn raw(self: *Writer, bytes: []const u8) Allocator.Error!void {
        try self.elem();
        try self.out.appendSlice(self.gpa, bytes);
    }

    fn key(self: *Writer, name: []const u8) Allocator.Error!void {
        try self.elem();
        try self.out.append(self.gpa, '"');
        try self.out.appendSlice(self.gpa, name);
        try self.out.appendSlice(self.gpa, "\":");
    }

    pub fn beginObject(self: *Writer, name: ?[]const u8) Allocator.Error!void {
        if (name) |n| try self.key(n) else try self.elem();
        try self.out.append(self.gpa, '{');
        self.stack[self.depth] = true;
        self.depth += 1;
    }

    pub fn endObject(self: *Writer) Allocator.Error!void {
        self.depth -= 1;
        try self.out.append(self.gpa, '}');
    }

    pub fn beginArray(self: *Writer, name: ?[]const u8) Allocator.Error!void {
        if (name) |n| try self.key(n) else try self.elem();
        try self.out.append(self.gpa, '[');
        self.stack[self.depth] = true;
        self.depth += 1;
    }

    pub fn endArray(self: *Writer) Allocator.Error!void {
        self.depth -= 1;
        try self.out.append(self.gpa, ']');
    }

    pub fn int(self: *Writer, name: ?[]const u8, value: i64) Allocator.Error!void {
        if (name) |n| try self.key(n) else try self.elem();
        try self.out.print(self.gpa, "{d}", .{value});
    }

    pub fn uint(self: *Writer, name: ?[]const u8, value: u64) Allocator.Error!void {
        if (name) |n| try self.key(n) else try self.elem();
        try self.out.print(self.gpa, "{d}", .{value});
    }

    pub fn boolean(self: *Writer, name: ?[]const u8, value: bool) Allocator.Error!void {
        if (name) |n| try self.key(n) else try self.elem();
        try self.out.appendSlice(self.gpa, if (value) "true" else "false");
    }

    pub fn nullField(self: *Writer, name: ?[]const u8) Allocator.Error!void {
        if (name) |n| try self.key(n) else try self.elem();
        try self.out.appendSlice(self.gpa, "null");
    }

    /// Write an already-serialized JSON value (used for token echo).
    pub fn rawField(self: *Writer, name: []const u8, raw_json: []const u8) Allocator.Error!void {
        try self.key(name);
        try self.out.appendSlice(self.gpa, raw_json);
    }

    pub fn string(self: *Writer, name: ?[]const u8, value: []const u8) Allocator.Error!void {
        if (name) |n| try self.key(n) else try self.elem();
        try self.writeString(value);
    }

    fn writeString(self: *Writer, value: []const u8) Allocator.Error!void {
        try self.out.append(self.gpa, '"');
        var start: usize = 0;
        var i: usize = 0;
        while (i < value.len) : (i += 1) {
            const b = value[i];
            const esc: ?[]const u8 = switch (b) {
                '"' => "\\\"",
                '\\' => "\\\\",
                0x08 => "\\u0008",
                0x09 => "\\t",
                0x0a => "\\n",
                0x0c => "\\u000c",
                0x0d => "\\r",
                else => if (b < 0x20) "" else null,
            };
            if (esc) |e| {
                if (i > start) try self.out.appendSlice(self.gpa, value[start..i]);
                if (e.len == 0) {
                    try self.out.print(self.gpa, "\\u{x:0>4}", .{b});
                } else {
                    try self.out.appendSlice(self.gpa, e);
                }
                start = i + 1;
            }
        }
        if (value.len > start) try self.out.appendSlice(self.gpa, value[start..]);
        try self.out.append(self.gpa, '"');
    }

    /// Serialize a parsed JSON value (used for control token echo).
    pub fn writeValue(self: *Writer, v: std.json.Value) Allocator.Error!void {
        switch (v) {
            .null => try self.raw("null"),
            .bool => |b| try self.raw(if (b) "true" else "false"),
            .integer => |i| try self.rawInt(i),
            .float => |f| {
                try self.elem();
                try self.out.print(self.gpa, "{d}", .{f});
            },
            .number_string => |s| try self.raw(s),
            .string => |s| {
                try self.elem();
                try self.writeString(s);
            },
            .array => |arr| {
                try self.beginArray(null);
                for (arr.items) |item| try self.writeValue(item);
                try self.endArray();
            },
            .object => |obj| {
                try self.beginObject(null);
                var it = obj.iterator();
                while (it.next()) |entry| {
                    try self.key(entry.key_ptr.*);
                    try self.valueEntry(entry.value_ptr.*);
                }
                try self.endObject();
            },
        }
    }

    fn rawInt(self: *Writer, i: i64) Allocator.Error!void {
        try self.elem();
        try self.out.print(self.gpa, "{d}", .{i});
    }

    /// Write a value that follows an already-emitted "key:" — no separator.
    fn valueEntry(self: *Writer, v: std.json.Value) Allocator.Error!void {
        switch (v) {
            .null => try self.out.appendSlice(self.gpa, "null"),
            .bool => |b| try self.out.appendSlice(self.gpa, if (b) "true" else "false"),
            .integer => |i| try self.out.print(self.gpa, "{d}", .{i}),
            .float => |f| try self.out.print(self.gpa, "{d}", .{f}),
            .number_string => |s| try self.out.appendSlice(self.gpa, s),
            .string => |s| try self.writeString(s),
            .array => |arr| {
                try self.out.append(self.gpa, '[');
                self.stack[self.depth] = true;
                self.depth += 1;
                for (arr.items) |item| try self.writeValue(item);
                self.depth -= 1;
                try self.out.append(self.gpa, ']');
            },
            .object => |obj| {
                try self.out.append(self.gpa, '{');
                self.stack[self.depth] = true;
                self.depth += 1;
                var it = obj.iterator();
                while (it.next()) |entry| {
                    try self.key(entry.key_ptr.*);
                    try self.valueEntry(entry.value_ptr.*);
                }
                self.depth -= 1;
                try self.out.append(self.gpa, '}');
            },
        }
    }
};

/// Convenience: serialize a parsed JSON value to owned bytes.
pub fn serializeValue(gpa: std.mem.Allocator, v: std.json.Value) ![]u8 {
    var w = Writer.init(gpa);
    errdefer w.deinit();
    try w.writeValue(v);
    return w.owned();
}

test "json writer escapes and separates" {
    const gpa = std.testing.allocator;
    var w = Writer.init(gpa);
    defer w.deinit();
    try w.beginObject(null);
    try w.string("a", "x\"y\nz");
    try w.int("b", -4);
    try w.boolean("c", true);
    try w.nullField("d");
    try w.beginArray("e");
    try w.uint(null, 7);
    try w.string(null, "p");
    try w.endArray();
    try w.endObject();
    const s = try w.owned();
    defer gpa.free(s);
    try std.testing.expectEqualStrings("{\"a\":\"x\\\"y\\nz\",\"b\":-4,\"c\":true,\"d\":null,\"e\":[7,\"p\"]}", s);
    const parsed = try std.json.parseFromSlice(std.json.Value, gpa, s, .{});
    defer parsed.deinit();
    try std.testing.expectEqualStrings("x\"y\nz", parsed.value.object.get("a").?.string);
}

test "serializeValue echoes token shapes" {
    const gpa = std.testing.allocator;
    const parsed = try std.json.parseFromSlice(std.json.Value, gpa, "{\"token\":12,\"command\":\"state\"}", .{});
    defer parsed.deinit();
    const tok = parsed.value.object.get("token").?;
    const raw = try serializeValue(gpa, tok);
    defer gpa.free(raw);
    try std.testing.expectEqualStrings("12", raw);
}
