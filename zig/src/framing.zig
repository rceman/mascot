const std = @import("std");

pub const FRAME_LIMIT: usize = 65_536;

/// Incremental NDJSON framer. Never retains more than FRAME_LIMIT bytes
/// including the terminating LF; the byte that would exceed the limit is
/// rejected before storage grows. UTF-8/JSON validation happens only once a
/// complete frame (ending in LF) has been received. The receipt clock is
/// sampled at the completing LF before any decoding.
pub const Error = error{
    SessionFailed, // feed called after a previous failure
    FrameTooLarge, // logical frame exceeds the byte limit
    InvalidUtf8, // complete frame is not valid UTF-8
    FrameRejected, // the frame callback rejected the frame
};

pub const Decoder = struct {
    gpa: std.mem.Allocator,
    bytes: std.ArrayList(u8) = .empty,
    peak: usize = 0,
    failed: bool = false,

    pub fn init(gpa: std.mem.Allocator) !Decoder {
        var d = Decoder{ .gpa = gpa };
        try d.bytes.ensureTotalCapacityPrecise(gpa, FRAME_LIMIT);
        return d;
    }

    pub fn deinit(self: *Decoder) void {
        self.bytes.deinit(self.gpa);
    }

    pub fn peakBufferBytes(self: *const Decoder) usize {
        return self.peak;
    }

    /// `clock` is a `fn() i64`; `on_frame` is a
    /// `fn(@TypeOf(ctx), frame: []const u8, receipt_qpc: i64) !void`.
    pub fn feed(
        self: *Decoder,
        input: []const u8,
        clock: anytype,
        ctx: anytype,
        on_frame: anytype,
    ) Error!void {
        if (self.failed) return error.SessionFailed;
        for (input) |byte| {
            if (self.bytes.items.len == FRAME_LIMIT) {
                self.failed = true;
                return error.FrameTooLarge;
            }
            self.bytes.appendAssumeCapacity(byte);
            if (self.bytes.items.len > self.peak) self.peak = self.bytes.items.len;
            if (byte == '\n') {
                const receipt_qpc = clock();
                if (!std.unicode.utf8ValidateSlice(self.bytes.items)) {
                    self.failed = true;
                    return error.InvalidUtf8;
                }
                on_frame(ctx, self.bytes.items, receipt_qpc) catch {
                    self.failed = true;
                    return error.FrameRejected;
                };
                self.bytes.clearRetainingCapacity();
            }
        }
    }

    /// Stream end validation: fails if a frame was previously rejected or a
    /// partial frame remains buffered.
    pub fn finish(self: *const Decoder) Error!void {
        if (self.failed or self.bytes.items.len != 0) return error.SessionFailed;
    }
};

const CountingClock = struct {
    var calls: i64 = 0;
    fn clock() i64 {
        calls += 1;
        return calls;
    }
};

test "split multibyte produces exact frames" {
    const gpa = std.testing.allocator;
    var d = try Decoder.init(gpa);
    defer d.deinit();
    CountingClock.calls = 0;
    var frames: std.ArrayList([]const u8) = .empty;
    defer frames.deinit(gpa);
    const Ctx = struct {
        frames: *std.ArrayList([]const u8),
        gpa: std.mem.Allocator,
    };
    var c = Ctx{ .frames = &frames, .gpa = gpa };
    defer for (frames.items) |f| gpa.free(@constCast(f));
    const collect = struct {
        fn f(cx: *Ctx, frame: []const u8, _: i64) !void {
            try cx.frames.append(cx.gpa, try cx.gpa.dupe(u8, frame));
        }
    }.f;
    try d.feed("{\"text\":\"\xc4", CountingClock.clock, &c, collect);
    try d.feed("\x80\"}\n{\"type\":\"complete\"}\n", CountingClock.clock, &c, collect);
    try d.finish();
    try std.testing.expectEqual(@as(i64, 2), CountingClock.calls);
    try std.testing.expectEqual(@as(usize, 2), frames.items.len);
    try std.testing.expectEqualStrings("{\"text\":\"\xc4\x80\"}\n", frames.items[0]);
    try std.testing.expectEqualStrings("{\"type\":\"complete\"}\n", frames.items[1]);
}

fn countingFrame(count: *usize, _: []const u8, _: i64) !void {
    count.* += 1;
}

fn zeroClock() i64 {
    return 0;
}

test "maximum frame accepted at limit" {
    const gpa = std.testing.allocator;
    var d = try Decoder.init(gpa);
    defer d.deinit();
    var frames: usize = 0;
    const payload = try gpa.alloc(u8, FRAME_LIMIT - 1);
    defer gpa.free(payload);
    @memset(payload, 'x');
    try d.feed(payload, zeroClock, &frames, countingFrame);
    try d.feed("\n", zeroClock, &frames, countingFrame);
    try std.testing.expectEqual(FRAME_LIMIT, d.peakBufferBytes());
    try std.testing.expectEqual(@as(usize, 1), frames);
}

test "oversized rejected before growth" {
    const gpa = std.testing.allocator;
    var d = try Decoder.init(gpa);
    defer d.deinit();
    var frames: usize = 0;
    const payload = try gpa.alloc(u8, FRAME_LIMIT + 1);
    defer gpa.free(payload);
    @memset(payload, 'x');
    try std.testing.expectError(error.FrameTooLarge, d.feed(payload, zeroClock, &frames, countingFrame));
    try std.testing.expectEqual(FRAME_LIMIT, d.peakBufferBytes());
    try std.testing.expectEqual(@as(usize, 0), frames);
    try std.testing.expectError(error.SessionFailed, d.feed("\n", zeroClock, &frames, countingFrame));
    try std.testing.expectError(error.SessionFailed, d.finish());
}

test "partial frame fails finish" {
    const gpa = std.testing.allocator;
    var d = try Decoder.init(gpa);
    defer d.deinit();
    var frames: usize = 0;
    try d.feed("{\"partial\":", zeroClock, &frames, countingFrame);
    try std.testing.expectError(error.SessionFailed, d.finish());
}

test "invalid utf8 rejected only at frame end" {
    const gpa = std.testing.allocator;
    var d = try Decoder.init(gpa);
    defer d.deinit();
    var frames: usize = 0;
    try d.feed("{\"bad\":\"\xff", zeroClock, &frames, countingFrame);
    try std.testing.expectError(error.InvalidUtf8, d.feed("\"}\n", zeroClock, &frames, countingFrame));
    try std.testing.expectEqual(@as(usize, 0), frames);
}

test "frame callback error fails the session" {
    const gpa = std.testing.allocator;
    var d = try Decoder.init(gpa);
    defer d.deinit();
    const reject = struct {
        fn f(_: *usize, _: []const u8, _: i64) !void {
            return error.Deny;
        }
    }.f;
    var frames: usize = 0;
    try std.testing.expectError(error.FrameRejected, d.feed("{\"a\":1}\n", zeroClock, &frames, reject));
    try std.testing.expectError(error.SessionFailed, d.feed("x\n", zeroClock, &frames, reject));
}
