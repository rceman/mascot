const std = @import("std");

/// Bounded blocking FIFO with close semantics. push blocks on a full queue
/// (backpressure); try_push fails on full; both fail once closed.
pub fn BoundedQueue(comptime T: type) type {
    return struct {
        const Self = @This();

        mutex: std.Thread.Mutex = .{},
        not_full: std.Thread.Condition = .{},
        not_empty: std.Thread.Condition = .{},
        buf: []T,
        head: usize = 0,
        len: usize = 0,
        closed: bool = false,

        pub fn init(gpa: std.mem.Allocator, cap: usize) !*Self {
            const self = try gpa.create(Self);
            errdefer gpa.destroy(self);
            self.* = .{ .buf = try gpa.alloc(T, cap) };
            return self;
        }

        pub fn deinit(self: *Self, gpa: std.mem.Allocator) void {
            gpa.free(self.buf);
            gpa.destroy(self);
        }

        pub fn capacity(self: *const Self) usize {
            return self.buf.len;
        }

        /// Blocking push; error{Closed} leaves the value with the caller.
        pub fn push(self: *Self, value: T) error{Closed}!void {
            self.mutex.lock();
            defer self.mutex.unlock();
            while (true) {
                if (self.closed) return error.Closed;
                if (self.len < self.buf.len) {
                    self.buf[(self.head + self.len) % self.buf.len] = value;
                    self.len += 1;
                    self.not_empty.signal();
                    return;
                }
                self.not_full.wait(&self.mutex);
            }
        }

        pub fn tryPush(self: *Self, value: T) error{ Closed, Full }!void {
            self.mutex.lock();
            defer self.mutex.unlock();
            if (self.closed) return error.Closed;
            if (self.len >= self.buf.len) return error.Full;
            self.buf[(self.head + self.len) % self.buf.len] = value;
            self.len += 1;
            self.not_empty.signal();
        }

        /// Non-blocking pop; returns null when empty or when empty after close.
        pub fn pop(self: *Self) ?T {
            self.mutex.lock();
            defer self.mutex.unlock();
            if (self.len == 0) return null;
            const item = self.buf[self.head];
            self.head = (self.head + 1) % self.buf.len;
            self.len -= 1;
            self.not_full.signal();
            return item;
        }

        /// Blocking pop; null once closed and drained.
        pub fn waitPop(self: *Self) ?T {
            self.mutex.lock();
            defer self.mutex.unlock();
            while (true) {
                if (self.len > 0) {
                    const item = self.buf[self.head];
                    self.head = (self.head + 1) % self.buf.len;
                    self.len -= 1;
                    self.not_full.signal();
                    return item;
                }
                if (self.closed) return null;
                self.not_empty.wait(&self.mutex);
            }
        }

        pub fn lenNow(self: *Self) usize {
            self.mutex.lock();
            defer self.mutex.unlock();
            return self.len;
        }

        pub fn close(self: *Self) void {
            self.mutex.lock();
            self.closed = true;
            self.mutex.unlock();
            self.not_full.broadcast();
            self.not_empty.broadcast();
        }
    };
}

test "push pop order and close" {
    const gpa = std.testing.allocator;
    const Q = BoundedQueue(i32);
    const q = try Q.init(gpa, 2);
    defer q.deinit(gpa);
    try q.push(1);
    try q.push(2);
    try std.testing.expectError(error.Full, q.tryPush(3));
    try std.testing.expectEqual(@as(usize, 2), q.lenNow());
    try std.testing.expectEqual(@as(?i32, 1), q.pop());
    q.close();
    try std.testing.expectError(error.Closed, q.push(4));
    try std.testing.expectEqual(@as(?i32, 2), q.pop());
    try std.testing.expectEqual(@as(?i32, null), q.pop());
}

test "wait_pop wakes on close" {
    const gpa = std.testing.allocator;
    const Q = BoundedQueue(u8);
    const q = try Q.init(gpa, 1);
    defer q.deinit(gpa);
    var result: ?u8 = 99;
    const t = try std.Thread.spawn(.{}, struct {
        fn run(qq: *Q, out: *?u8) void {
            out.* = qq.waitPop();
        }
    }.run, .{ q, &result });
    std.Thread.sleep(20 * std.time.ns_per_ms);
    q.close();
    t.join();
    try std.testing.expectEqual(@as(?u8, null), result);
}

test "blocked producer wakes on drain" {
    const gpa = std.testing.allocator;
    const Q = BoundedQueue(i32);
    const q = try Q.init(gpa, 1);
    defer q.deinit(gpa);
    try q.push(7);
    var ok = false;
    const t = try std.Thread.spawn(.{}, struct {
        fn run(qq: *Q, out: *bool) void {
            qq.push(9) catch return;
            out.* = true;
        }
    }.run, .{ q, &ok });
    std.Thread.sleep(20 * std.time.ns_per_ms);
    try std.testing.expectEqual(@as(?i32, 7), q.pop());
    t.join();
    try std.testing.expectEqual(@as(?i32, 9), q.pop());
}

test "blocked producer released with error on close" {
    const gpa = std.testing.allocator;
    const Q = BoundedQueue(i32);
    const q = try Q.init(gpa, 1);
    defer q.deinit(gpa);
    try q.push(7);
    var got_closed = false;
    const t = try std.Thread.spawn(.{}, struct {
        fn run(qq: *Q, out: *bool) void {
            qq.push(9) catch {
                out.* = true;
                return;
            };
        }
    }.run, .{ q, &got_closed });
    std.Thread.sleep(20 * std.time.ns_per_ms);
    q.close();
    t.join();
    try std.testing.expect(got_closed);
    try std.testing.expectEqual(@as(?i32, 7), q.pop());
}
