const std = @import("std");
const win32 = @import("win32.zig");

const c = win32.c;
const Allocator = std.mem.Allocator;

pub const ST_DEFAULT: c.DWORD = 0;
pub const CP_UNICODE: c.DWORD = 1200;
pub const GT_DEFAULT: c.DWORD = 0;
pub const GTL_DEFAULT: c.DWORD = 0;

pub const RICHEDIT_CLASS = win32.wideLit("RICHEDIT50W");

pub const EditError = error{ CreateFailed, ReadFailed, OutOfMemory, Unexpected };

/// Load msftedit.dll from System32 only. Caller must FreeLibrary it after all
/// RichEdit controls are destroyed.
pub fn loadRichEdit() !c.HMODULE {
    const mod = c.LoadLibraryExW(
        win32.wideLit("msftedit.dll").ptr,
        null,
        c.LOAD_LIBRARY_SEARCH_SYSTEM32,
    );
    if (mod == null) return error.LoadFailed;
    return mod;
}

pub fn createEdit(
    parent: c.HWND,
    style: c.DWORD,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    undo_limit: u32,
) !c.HWND {
    const hwnd = c.CreateWindowExW(
        0,
        RICHEDIT_CLASS.ptr,
        win32.wideLit("").ptr,
        style,
        x,
        y,
        width,
        height,
        parent,
        null,
        null,
        null,
    );
    if (hwnd == null) return error.CreateFailed;
    _ = c.SendMessageW(hwnd, c.EM_SETTEXTMODE, c.TM_PLAINTEXT | c.TM_MULTILEVELUNDO | c.TM_MULTICODEPAGE, 0);
    _ = c.SendMessageW(hwnd, c.EM_SETUNDOLIMIT, undo_limit, 0);
    _ = c.SendMessageW(hwnd, c.EM_SETTYPOGRAPHYOPTIONS, c.TO_ADVANCEDTYPOGRAPHY, c.TO_ADVANCEDTYPOGRAPHY);
    return hwnd;
}

/// Replace text; accepts logical-LF UTF-8, converts to RichEdit CRLF.
pub fn setText(hwnd: c.HWND, text: []const u8) !void {
    const gpa = std.heap.smp_allocator;
    const normalized = try std.mem.replaceOwned(u8, gpa, text, "\n", "\r\n");
    defer gpa.free(normalized);
    const wide = try win32.wideAlloc(gpa, normalized);
    defer gpa.free(wide);
    var st = c.SETTEXTEX{
        .flags = ST_DEFAULT,
        .codepage = CP_UNICODE,
    };
    _ = c.SendMessageW(hwnd, c.EM_SETTEXTEX, @bitCast(@intFromPtr(&st)), @bitCast(@intFromPtr(wide.ptr)));
}

/// Read all text as logical-LF UTF-8.
pub fn getText(hwnd: c.HWND) ![]u8 {
    const gpa = std.heap.smp_allocator;
    var len_params = c.GETTEXTLENGTHEX{
        .flags = GTL_DEFAULT,
        .codepage = CP_UNICODE,
    };
    const units = c.SendMessageW(hwnd, c.EM_GETTEXTLENGTHEX, @bitCast(@intFromPtr(&len_params)), 0);
    if (units < 0) return error.ReadFailed;
    const buf = try gpa.alloc(u16, @as(usize, @intCast(units)) + 1);
    defer gpa.free(buf);
    var params = c.GETTEXTEX{
        .cb = @intCast((buf.len) * @sizeOf(u16)),
        .flags = GT_DEFAULT,
        .codepage = CP_UNICODE,
        .lpDefaultChar = null,
        .lpUsedDefChar = null,
    };
    const copied = c.SendMessageW(hwnd, c.EM_GETTEXTEX, @bitCast(@intFromPtr(&params)), @bitCast(@intFromPtr(buf.ptr)));
    if (copied < 0) return error.ReadFailed;
    const text_u16 = buf[0..@intCast(copied)];
    const utf8 = try std.unicode.utf16LeToUtf8Alloc(gpa, text_u16);
    defer gpa.free(utf8);
    // normalize CRLF and lone CR to LF
    var out: std.ArrayList(u8) = .empty;
    errdefer out.deinit(gpa);
    var i: usize = 0;
    while (i < utf8.len) {
        if (utf8[i] == '\r') {
            try out.append(gpa, '\n');
            i += 1;
            if (i < utf8.len and utf8[i] == '\n') i += 1;
        } else {
            try out.append(gpa, utf8[i]);
            i += 1;
        }
    }
    return out.toOwnedSlice(gpa);
}

/// Append logical-LF text at the document end (paragraph-mark CR style).
pub fn appendText(hwnd: c.HWND, text: []const u8) !void {
    const gpa = std.heap.smp_allocator;
    const normalized = blk: {
        const lf = try normalizeLf(gpa, text);
        defer gpa.free(lf);
        break :blk try std.mem.replaceOwned(u8, gpa, lf, "\n", "\r");
    };
    defer gpa.free(normalized);
    const wide = try win32.wideAlloc(gpa, normalized);
    defer gpa.free(wide);
    _ = c.SendMessageW(hwnd, c.EM_SETSEL, @bitCast(@as(isize, -1)), -1);
    _ = c.SendMessageW(hwnd, c.EM_REPLACESEL, 0, @bitCast(@intFromPtr(wide.ptr)));
    _ = c.SendMessageW(hwnd, c.EM_SCROLLCARET, 0, 0);
}

fn normalizeLf(gpa: Allocator, text: []const u8) ![]u8 {
    var out: std.ArrayList(u8) = .empty;
    errdefer out.deinit(gpa);
    var i: usize = 0;
    while (i < text.len) {
        if (text[i] == '\r') {
            try out.append(gpa, '\n');
            i += 1;
            if (i < text.len and text[i] == '\n') i += 1;
        } else {
            try out.append(gpa, text[i]);
            i += 1;
        }
    }
    return out.toOwnedSlice(gpa);
}

pub fn utf16Units(text: []const u8) usize {
    var units: usize = 0;
    var it = std.unicode.Wtf8View.initUnchecked(text).iterator();
    while (it.nextCodepoint()) |cp| {
        units += if (cp > 0xFFFF) 2 else 1;
    }
    return units;
}

pub fn utf8ForUnits(text: []const u8, units: usize) usize {
    var used: usize = 0;
    var it = std.unicode.Wtf8View.initUnchecked(text).iterator();
    var bytes: usize = 0;
    while (it.nextCodepoint()) |cp| {
        const cost: usize = if (cp > 0xFFFF) 2 else 1;
        if (used + cost > units) break;
        used += cost;
        bytes = it.i;
    }
    return bytes;
}

pub fn selection(hwnd: c.HWND) !struct { start: u32, end: u32 } {
    var range = c.CHARRANGE{ .cpMin = 0, .cpMax = 0 };
    _ = c.SendMessageW(hwnd, c.EM_EXGETSEL, 0, @bitCast(@intFromPtr(&range)));
    return .{
        .start = @intCast(@max(range.cpMin, 0)),
        .end = @intCast(@max(range.cpMax, 0)),
    };
}

pub fn setSelection(hwnd: c.HWND, start: u32, end: u32) void {
    var range = c.CHARRANGE{
        .cpMin = @intCast(@min(start, std.math.maxInt(c.LONG))),
        .cpMax = @intCast(@min(end, std.math.maxInt(c.LONG))),
    };
    _ = c.SendMessageW(hwnd, c.EM_EXSETSEL, 0, @bitCast(@intFromPtr(&range)));
}

/// Cancel an in-progress IME composition on the control.
pub fn cancelImeComposition(hwnd: c.HWND) void {
    const himc = c.ImmGetContext(hwnd);
    if (himc == null) return;
    _ = c.ImmNotifyIME(himc, c.NI_COMPOSITIONSTR, c.CPS_CANCEL, 0);
    _ = c.ImmReleaseContext(hwnd, himc);
}
