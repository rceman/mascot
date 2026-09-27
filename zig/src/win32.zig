const std = @import("std");

pub const c = @cImport({
    @cDefine("WIN32_LEAN_AND_MEAN", "1");
    @cDefine("_WIN32_WINNT", "0x0A00");
    @cDefine("UNICODE", "1");
    @cDefine("_UNICODE", "1");
    @cInclude("windows.h");
    @cInclude("richedit.h");
    @cInclude("commctrl.h");
    @cInclude("imm.h");
    @cInclude("ole2.h");
    @cInclude("wincodec.h");
    @cInclude("wchar.h");
});

/// IDC_ARROW cannot be translated from the MAKEINTRESOURCE macro.
pub const IDC_ARROW_RES: [*:0]const u16 = @ptrFromInt(32512);

pub fn qpc() i64 {
    var value: c.LARGE_INTEGER = std.mem.zeroes(c.LARGE_INTEGER);
    if (c.QueryPerformanceCounter(&value) == 0) {
        @panic("QueryPerformanceCounter failed");
    }
    return value.QuadPart;
}

pub fn qpcFrequency() i64 {
    var value: c.LARGE_INTEGER = std.mem.zeroes(c.LARGE_INTEGER);
    if (c.QueryPerformanceFrequency(&value) == 0 or value.QuadPart <= 0) {
        @panic("QueryPerformanceFrequency failed");
    }
    return value.QuadPart;
}

/// Monotonic milliseconds derived from QPC; used for shared deadlines.
pub fn nowMs() i64 {
    return @intCast(@divTrunc(qpc() * 1000, qpcFrequency()));
}

pub fn dip(value: i32, dpi: u32) i32 {
    return @intCast(@divTrunc(value * @as(i32, @intCast(dpi)) + 48, 96));
}

pub fn wideAlloc(gpa: std.mem.Allocator, text: []const u8) ![:0]u16 {
    return std.unicode.utf8ToUtf16LeAllocZ(gpa, text);
}

pub fn wideLit(comptime text: []const u8) [:0]const u16 {
    return std.unicode.utf8ToUtf16LeStringLiteral(text);
}

/// Bounded wait on a kernel handle up to an absolute millisecond deadline.
pub fn waitHandle(handle: c.HANDLE, deadline_ms: i64) bool {
    const remaining = deadline_ms - nowMs();
    const ms: c.DWORD = if (remaining <= 0) 0 else @intCast(@min(remaining, @as(i64, std.math.maxInt(c.DWORD))));
    return c.WaitForSingleObject(handle, ms) == c.WAIT_OBJECT_0;
}

/// Correct blocking read for handles that may have been opened with
/// FILE_FLAG_OVERLAPPED (std.process.Child stdout/stderr pipes are async
/// named-pipe handles). A dedicated event serializes completion per reader.
pub fn readPipe(handle: c.HANDLE, buffer: []u8, event: c.HANDLE) !usize {
    var overlapped: c.OVERLAPPED = std.mem.zeroes(c.OVERLAPPED);
    overlapped.hEvent = event;
    var amount: c.DWORD = 0;
    const want: c.DWORD = @intCast(@min(buffer.len, std.math.maxInt(c.DWORD)));
    if (c.ReadFile(handle, buffer.ptr, want, &amount, &overlapped) != 0) {
        return amount;
    }
    const err = c.GetLastError();
    if (err == c.ERROR_BROKEN_PIPE or err == c.ERROR_HANDLE_EOF or
        err == c.ERROR_OPERATION_ABORTED)
        return 0;
    if (err == c.ERROR_IO_PENDING) {
        if (c.WaitForSingleObject(event, c.INFINITE) != c.WAIT_OBJECT_0) {
            return error.ReadFailed;
        }
        var got: c.DWORD = 0;
        if (c.GetOverlappedResult(handle, &overlapped, &got, c.TRUE) == 0) {
            const e2 = c.GetLastError();
            if (e2 == c.ERROR_BROKEN_PIPE or e2 == c.ERROR_HANDLE_EOF or
                e2 == c.ERROR_OPERATION_ABORTED)
                return 0;
            return error.ReadFailed;
        }
        return got;
    }
    return error.ReadFailed;
}

/// Create the manual-reset event used by readPipe loops. Caller closes it.
pub fn createPipeEvent() !c.HANDLE {
    const ev = c.CreateEventW(null, c.TRUE, c.FALSE, null);
    if (ev == null) return error.EventFailed;
    return ev;
}

/// Bounded join: returns false if the thread is still running at the deadline.
/// `cancel_io` optionally cancels pending synchronous I/O on that thread.
pub fn joinBounded(thread: std.Thread, deadline_ms: i64, cancel_io: bool) bool {
    while (true) {
        if (c.WaitForSingleObject(thread.getHandle(), 0) == c.WAIT_OBJECT_0) {
            thread.join();
            return true;
        }
        const remaining = deadline_ms - nowMs();
        if (remaining <= 0) return false;
        const slice: c.DWORD = @intCast(@min(remaining, 20));
        if (c.WaitForSingleObject(thread.getHandle(), slice) == c.WAIT_OBJECT_0) {
            thread.join();
            return true;
        }
        if (cancel_io) {
            _ = c.CancelSynchronousIo(thread.getHandle());
        }
    }
}

/// Clear HANDLE_FLAG_INHERIT on this process's own std handles so that only
/// the child pipe ends created by std.process.Child can propagate.
pub fn clearStdInheritFlags() void {
    const ids = [_]c.DWORD{ c.STD_INPUT_HANDLE, c.STD_OUTPUT_HANDLE, c.STD_ERROR_HANDLE };
    for (ids) |id| {
        const h = c.GetStdHandle(id);
        if (h != null and h != c.INVALID_HANDLE_VALUE) {
            _ = c.SetHandleInformation(h, c.HANDLE_FLAG_INHERIT, 0);
        }
    }
}
