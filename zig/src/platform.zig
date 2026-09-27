const std = @import("std");
const win32 = @import("win32.zig");
const provider = @import("provider.zig");
const config = @import("config.zig");
const queue_mod = @import("queue.zig");
const text = @import("text.zig");
const control = @import("control.zig");
const jsonw = @import("json.zig");

const c = win32.c;
const Allocator = std.mem.Allocator;

pub const WM_APP_PROVIDER: c.UINT = c.WM_APP + 1;
pub const WM_APP_CONTROL: c.UINT = c.WM_APP + 2;
pub const WM_APP_SUBMIT: c.UINT = c.WM_APP + 3;

pub const HOTKEY_ID_SHOW: i32 = 1;
pub const HOTKEY_ID_CANCEL: i32 = 2;

const MASCOT_CLASS = win32.wideLit("MascotZigMascot");
const COMPOSER_CLASS = win32.wideLit("MascotZigComposer");

pub const UiError = error{
    PresentFailed,
    ComposerFailed,
    ProviderFailed,
    NoComposer,
    OutOfMemory,
    Unexpected,
};

const Snapshot = struct {
    text: []u8, // owned
    start: u32,
    end: u32,

    fn deinit(self: *Snapshot, gpa: Allocator) void {
        gpa.free(self.text);
    }
};

const Surface = struct {
    dc: c.HDC,
    bitmap: c.HBITMAP,
    previous: c.HGDIOBJ,
    pixels: usize,

    fn create(source: []const u8, src_w: usize, src_h: usize, pixels: usize) !*Surface {
        const gpa = std.heap.smp_allocator;
        if (source.len != src_w * src_h * 4 or pixels == 0 or src_w == 0 or src_h == 0)
            return error.InvalidSurface;
        const screen = c.GetDC(null);
        if (screen == null) return error.PresentFailed;
        defer _ = c.ReleaseDC(null, screen);
        const dc = c.CreateCompatibleDC(screen);
        if (dc == null) return error.PresentFailed;
        var err_dc = true;
        defer {
            if (err_dc) _ = c.DeleteDC(dc);
        }
        var bmi: c.BITMAPINFO = std.mem.zeroes(c.BITMAPINFO);
        bmi.bmiHeader.biSize = @sizeOf(c.BITMAPINFOHEADER);
        bmi.bmiHeader.biWidth = @intCast(pixels);
        bmi.bmiHeader.biHeight = @intCast(pixels);
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = c.BI_RGB;
        var bits: ?*anyopaque = null;
        const bitmap = c.CreateDIBSection(dc, &bmi, c.DIB_RGB_COLORS, &bits, null, 0);
        if (bitmap == null or bits == null) {
            if (bitmap != null) _ = c.DeleteObject(bitmap);
            return error.PresentFailed;
        }
        var err_bmp = true;
        defer {
            if (err_bmp) _ = c.DeleteObject(bitmap);
        }
        const previous = c.SelectObject(dc, bitmap);
        if (previous == null) return error.PresentFailed;
        // nearest-neighbor scale from the premultiplied source
        const dst: [*]u8 = @ptrCast(bits.?);
        for (0..pixels) |y| {
            for (0..pixels) |x| {
                const sx = x * src_w / pixels;
                const sy = y * src_h / pixels;
                @memcpy(dst[(y * pixels + x) * 4 ..][0..4], source[(sy * src_w + sx) * 4 ..][0..4]);
            }
        }
        const surface = try gpa.create(Surface);
        surface.* = .{ .dc = dc, .bitmap = bitmap, .previous = previous, .pixels = pixels };
        err_dc = false;
        err_bmp = false;
        return surface;
    }

    fn deinit(self: *Surface) void {
        _ = c.SelectObject(self.dc, self.previous);
        _ = c.DeleteObject(self.bitmap);
        _ = c.DeleteDC(self.dc);
        std.heap.smp_allocator.destroy(self);
    }
};

const Composer = struct {
    hwnd: c.HWND,
    input: c.HWND,
    response: c.HWND,
    status: c.HWND,
    send: c.HWND,
    cancel: c.HWND,
    font: c.HFONT,
    richedit: c.HMODULE,
    dpi: u32,
};

const Model = struct {
    provider_state: []const u8 = "idle",
    scenario: []u8, // owned
    request_id: u64 = 0,
    request_count: u64 = 0,
    provider_pid: u32 = 0,
    last_seq: i64 = -1,
    response: std.ArrayList(u8) = .empty, // owned; logical-LF UTF-8
    run_invalid: ?[]u8 = null, // owned
    provider_error: ?[]u8 = null, // owned
    generation: u64 = 0,

    fn deinit(self: *Model, gpa: Allocator) void {
        gpa.free(self.scenario);
        self.response.deinit(gpa);
        if (self.run_invalid) |s| gpa.free(s);
        if (self.provider_error) |s| gpa.free(s);
    }
};

pub const Ui = struct {
    gpa: Allocator,
    config: *config.Config,
    mascot: c.HWND = null,
    mascot_source: []const u8, // premultiplied BGRA src_w x src_h, owned by caller
    mascot_src_w: usize,
    mascot_src_h: usize,
    surface: ?*Surface = null,
    composer: ?*Composer = null,
    model: Model,
    composing: bool = false,
    snapshot: ?Snapshot = null,
    ui_events: *queue_mod.BoundedQueue(provider.Event),
    commands: *queue_mod.BoundedQueue(control.ControlCmd),
    records: ?*queue_mod.BoundedQueue([]u8),
    provider: ?*provider.Provider = null,
    control_enabled: bool = false,
    shutdown_started: bool = false,
    cancel_pending: bool = false,
    pending_shutdown_token: ?[]u8 = null, // owned raw JSON
    presents: u64 = 0,
    paints: u64 = 0,
};

fn eventMatches(generation: u64, id: u64, model: *const Model) bool {
    return generation == model.generation and id == model.request_id;
}

fn appendBounded(list: *std.ArrayList(u8), gpa: Allocator, limit: usize, chunk: []const u8) !void {
    if (list.items.len + chunk.len > limit) return error.Overflow;
    try list.appendSlice(gpa, chunk);
}

fn fontHeight(dpi: u32) i32 {
    return -win32.dip(16, dpi);
}

fn currentDpi(hwnd: c.HWND) u32 {
    const dpi = c.GetDpiForWindow(hwnd);
    return if (dpi == 0) c.GetDpiForSystem() else dpi;
}

fn uiFromHwnd(hwnd: c.HWND) ?*Ui {
    const raw = c.GetWindowLongPtrW(hwnd, c.GWLP_USERDATA);
    if (raw == 0) return null;
    return @ptrFromInt(@as(usize, @intCast(raw)));
}

fn storeUi(hwnd: c.HWND, ui: *Ui) void {
    _ = c.SetWindowLongPtrW(hwnd, c.GWLP_USERDATA, @bitCast(@intFromPtr(ui)));
}

// ---------------------------------------------------------------- mascot

pub fn registerClasses() !void {
    var mascot_class = std.mem.zeroes(c.WNDCLASSW);
    mascot_class.lpfnWndProc = mascotProc;
    mascot_class.hInstance = c.GetModuleHandleW(null);
    mascot_class.hCursor = c.LoadCursorW(null, IDC_ARROW_W);
    mascot_class.lpszClassName = MASCOT_CLASS.ptr;
    if (c.RegisterClassW(&mascot_class) == 0) return error.RegisterFailed;

    var composer_class = std.mem.zeroes(c.WNDCLASSW);
    composer_class.lpfnWndProc = composerProc;
    composer_class.hInstance = c.GetModuleHandleW(null);
    composer_class.hCursor = c.LoadCursorW(null, IDC_ARROW_W);
    composer_class.hbrBackground = c.GetSysColorBrush(c.COLOR_WINDOW);
    composer_class.lpszClassName = COMPOSER_CLASS.ptr;
    if (c.RegisterClassW(&composer_class) == 0) return error.RegisterFailed;
}

const IDC_ARROW_W: [*:0]const u16 = @ptrFromInt(32512);

pub fn createMascot(ui: *Ui) !void {
    const dpi = c.GetDpiForSystem();
    const size_px = win32.dip(64, dpi);
    var cursor = c.POINT{ .x = 0, .y = 0 };
    var work = c.RECT{ .left = 0, .top = 0, .right = 2560, .bottom = 1440 };
    if (c.GetCursorPos(&cursor) != 0) {
        var info = std.mem.zeroes(c.MONITORINFO);
        info.cbSize = @sizeOf(c.MONITORINFO);
        const monitor = c.MonitorFromPoint(cursor, c.MONITOR_DEFAULTTONEAREST);
        if (c.GetMonitorInfoW(monitor, &info) != 0) {
            work = info.rcWork;
        } else {
            const primary = c.MonitorFromWindow(null, c.MONITOR_DEFAULTTOPRIMARY);
            if (primary != null and c.GetMonitorInfoW(primary, &info) != 0)
                work = info.rcWork;
        }
        const left = cursor.x - @divTrunc(size_px, 2);
        const top = cursor.y - @divTrunc(size_px, 2);
        const clamped_x = @max(left, @min(work.left, work.right - size_px));
        const clamped_y = @max(top, @min(work.top, work.bottom - size_px));
        cursor.x = clamped_x;
        cursor.y = clamped_y;
    } else {
        cursor.x = work.left + @max(0, @divTrunc(work.right - work.left - size_px, 2));
        cursor.y = work.top + @max(0, @divTrunc(work.bottom - work.top - size_px, 2));
    }
    const mascot = c.CreateWindowExW(
        c.WS_EX_LAYERED | c.WS_EX_TOPMOST | c.WS_EX_TOOLWINDOW | c.WS_EX_NOACTIVATE,
        MASCOT_CLASS.ptr,
        win32.wideLit("Mascot Zig").ptr,
        c.WS_POPUP,
        cursor.x,
        cursor.y,
        size_px,
        size_px,
        null,
        null,
        c.GetModuleHandleW(null),
        ui,
    );
    if (mascot == null) return error.PresentFailed;
    ui.mascot = mascot;
    const eff_dpi = currentDpi(mascot);
    try presentMascot(ui, eff_dpi);
    _ = c.ShowWindow(mascot, c.SW_SHOWNOACTIVATE);
}

fn presentMascot(ui: *Ui, dpi: u32) !void {
    const pixels_i = win32.dip(64, dpi);
    if (pixels_i <= 0) return error.PresentFailed;
    const pixels: usize = @intCast(pixels_i);
    const surface = try Surface.create(ui.mascot_source, ui.mascot_src_w, ui.mascot_src_h, pixels);
    if (ui.surface) |old| old.deinit();
    ui.surface = surface;

    var bounds: c.RECT = undefined;
    if (c.GetWindowRect(ui.mascot, &bounds) == 0) return error.PresentFailed;
    var top_left = c.POINT{ .x = bounds.left, .y = bounds.top };
    var size = c.SIZE{ .cx = @intCast(pixels), .cy = @intCast(pixels) };
    var zero = c.POINT{ .x = 0, .y = 0 };
    var blend = c.BLENDFUNCTION{
        .BlendOp = c.AC_SRC_OVER,
        .BlendFlags = 0,
        .SourceConstantAlpha = 255,
        .AlphaFormat = c.AC_SRC_ALPHA,
    };
    const screen = c.GetDC(null);
    if (screen == null) return error.PresentFailed;
    const presented = c.UpdateLayeredWindow(
        ui.mascot,
        screen,
        &top_left,
        &size,
        surface.dc,
        &zero,
        0,
        &blend,
        c.ULW_ALPHA,
    );
    _ = c.ReleaseDC(null, screen);
    if (presented == 0) return error.PresentFailed;
    ui.presents += 1;
}

fn hitTest(ui: *Ui, lparam: c.LPARAM) i32 {
    const x: i32 = @as(i16, @truncate(@as(isize, lparam)));
    const y: i32 = @as(i16, @truncate(lparam >> 16));
    var bounds: c.RECT = undefined;
    if (c.GetWindowRect(ui.mascot, &bounds) == 0) return c.HTTRANSPARENT;
    const width = bounds.right - bounds.left;
    const height = bounds.bottom - bounds.top;
    if (width <= 0 or height <= 0) return c.HTTRANSPARENT;
    const px: i64 = @as(i64, x) - bounds.left;
    const py: i64 = @as(i64, y) - bounds.top;
    if (px < 0 or py < 0 or px >= width or py >= height) return c.HTTRANSPARENT;
    const src_w: i64 = @intCast(ui.mascot_src_w);
    const src_h: i64 = @intCast(ui.mascot_src_h);
    const sx = @min(@divTrunc(px * src_w, width), src_w - 1);
    const sy = @min(@divTrunc(py * src_h, height), src_h - 1);
    const alpha = ui.mascot_source[@intCast((@as(usize, @intCast(sy)) * ui.mascot_src_w + @as(usize, @intCast(sx))) * 4 + 3)];
    return if (alpha > 0) c.HTCAPTION else c.HTTRANSPARENT;
}

fn showExitMenu(ui: *Ui) void {
    const menu = c.CreatePopupMenu();
    if (menu == null) return;
    defer _ = c.DestroyMenu(menu);
    const label = win32.wideLit("Exit");
    _ = c.AppendMenuW(menu, c.MF_STRING, 1, @ptrCast(label.ptr));
    var pt = c.POINT{ .x = 0, .y = 0 };
    _ = c.GetCursorPos(&pt);
    _ = c.SetForegroundWindow(ui.mascot);
    const cmd = c.TrackPopupMenu(menu, c.TPM_RETURNCMD | c.TPM_RIGHTBUTTON | c.TPM_NONOTIFY, pt.x, pt.y, 0, ui.mascot, null);
    if (cmd == 1) requestShutdown(ui, null);
}

fn onDpiChanged(ui: *Ui, lparam: c.LPARAM) void {
    const suggested: *const c.RECT = @ptrFromInt(@as(usize, @bitCast(lparam)));
    _ = c.SetWindowPos(ui.mascot, null, suggested.left, suggested.top, suggested.right - suggested.left, suggested.bottom - suggested.top, c.SWP_NOZORDER | c.SWP_NOACTIVATE);
    presentMascot(ui, currentDpi(ui.mascot)) catch |e| {
        std.debug.print("mascot DPI presentation failed: {s}\n", .{@errorName(e)});
    };
    if (ui.composer) |comp| {
        const new_font = createFont(currentDpi(comp.hwnd));
        if (new_font != null) {
            const old = comp.font;
            comp.font = new_font;
            for ([_]c.HWND{ comp.input, comp.response, comp.status, comp.send, comp.cancel }) |child| {
                _ = c.SendMessageW(child, c.WM_SETFONT, @intFromPtr(new_font), 1);
            }
            if (old != null) _ = c.DeleteObject(old);
        }
        comp.dpi = currentDpi(comp.hwnd);
        relayout(ui, comp);
    }
}

fn createFont(dpi: u32) c.HFONT {
    return c.CreateFontW(
        fontHeight(dpi),
        0,
        0,
        0,
        c.FW_NORMAL,
        0,
        0,
        0,
        c.DEFAULT_CHARSET,
        c.OUT_DEFAULT_PRECIS,
        c.CLIP_DEFAULT_PRECIS,
        c.CLEARTYPE_QUALITY,
        c.DEFAULT_PITCH | c.FF_DONTCARE,
        win32.wideLit("Segoe UI").ptr,
    );
}

// ------------------------------------------------------------- composer

fn ensureComposer(ui: *Ui) !*Composer {
    if (ui.composer) |comp| return comp;
    const dpi = currentDpi(ui.mascot);
    const richedit = text.loadRichEdit() catch return error.ComposerFailed;
    const init = c.OleInitialize(null);
    _ = init;
    var rect = c.RECT{ .left = 0, .top = 0, .right = win32.dip(640, dpi), .bottom = win32.dip(480, dpi) };
    if (c.AdjustWindowRectExForDpi(&rect, c.WS_CAPTION | c.WS_SYSMENU | c.WS_MINIMIZEBOX | c.WS_CLIPCHILDREN, 0, 0, dpi) == 0) {
        _ = c.FreeLibrary(richedit);
        return error.ComposerFailed;
    }
    var x: i32 = 120;
    var y: i32 = 120;
    var bounds: c.RECT = undefined;
    if (ui.mascot != null and c.GetWindowRect(ui.mascot, &bounds) != 0) {
        x = bounds.right + win32.dip(8, dpi);
        y = bounds.top;
    }
    var work = c.RECT{ .left = 0, .top = 0, .right = 2560, .bottom = 1440 };
    const pt = c.POINT{ .x = x, .y = y };
    var info = std.mem.zeroes(c.MONITORINFO);
    info.cbSize = @sizeOf(c.MONITORINFO);
    const monitor = c.MonitorFromPoint(pt, c.MONITOR_DEFAULTTONEAREST);
    if (monitor != null and c.GetMonitorInfoW(monitor, &info) != 0) {
        work = info.rcWork;
    }
    const width = rect.right - rect.left;
    const height = rect.bottom - rect.top;
    x = @max(x, @min(work.left, work.right - width));
    y = @max(y, @min(work.top, work.bottom - height));
    const hwnd = c.CreateWindowExW(
        0,
        COMPOSER_CLASS.ptr,
        win32.wideLit("Mascot Composer").ptr,
        c.WS_CAPTION | c.WS_SYSMENU | c.WS_MINIMIZEBOX | c.WS_CLIPCHILDREN,
        x,
        y,
        width,
        height,
        null,
        null,
        c.GetModuleHandleW(null),
        ui,
    );
    if (hwnd == null) {
        _ = c.FreeLibrary(richedit);
        return error.ComposerFailed;
    }
    enforceComposerClient(hwnd, dpi);
    return ui.composer orelse error.ComposerFailed;
}

fn enforceComposerClient(hwnd: c.HWND, dpi: u32) void {
    var client = std.mem.zeroes(c.RECT);
    if (c.GetClientRect(hwnd, &client) == 0) return;
    const want_w = win32.dip(640, dpi);
    const want_h = win32.dip(480, dpi);
    if (client.right - client.left != want_w or client.bottom - client.top != want_h) {
        var bounds: c.RECT = undefined;
        if (c.GetWindowRect(hwnd, &bounds) == 0) return;
        var rect = c.RECT{ .left = 0, .top = 0, .right = want_w, .bottom = want_h };
        _ = c.AdjustWindowRectExForDpi(&rect, c.WS_CAPTION | c.WS_SYSMENU | c.WS_MINIMIZEBOX | c.WS_CLIPCHILDREN, 0, 0, dpi);
        _ = c.SetWindowPos(hwnd, null, bounds.left, bounds.top, rect.right - rect.left, rect.bottom - rect.top, c.SWP_NOZORDER | c.SWP_NOACTIVATE);
    }
}

fn composerDpi(ui: *Ui) u32 {
    if (ui.composer) |comp| return comp.dpi;
    return currentDpi(ui.mascot);
}

fn relayout(ui: *Ui, comp: *Composer) void {
    _ = ui;
    const dpi = comp.dpi;
    const margin = win32.dip(12, dpi);
    const input_h = win32.dip(128, dpi);
    const response_h = win32.dip(272, dpi);
    var client: c.RECT = undefined;
    if (c.GetClientRect(comp.hwnd, &client) == 0) return;
    const width = client.right - client.left;
    const inner = width - 2 * margin;
    const status_y = client.bottom - margin - win32.dip(28, dpi);
    _ = c.SetWindowPos(comp.input, null, margin, margin, inner, input_h, c.SWP_NOZORDER);
    _ = c.SetWindowPos(comp.response, null, margin, margin * 2 + input_h, inner, response_h, c.SWP_NOZORDER);
    _ = c.SetWindowPos(comp.status, null, margin, status_y, inner - win32.dip(200, dpi), win32.dip(28, dpi), c.SWP_NOZORDER);
    _ = c.SetWindowPos(comp.send, null, width - margin - win32.dip(200, dpi), status_y, win32.dip(80, dpi), win32.dip(28, dpi), c.SWP_NOZORDER);
    _ = c.SetWindowPos(comp.cancel, null, width - margin - win32.dip(112, dpi), status_y, win32.dip(100, dpi), win32.dip(28, dpi), c.SWP_NOZORDER);
}

fn createChildren(ui: *Ui, hwnd: c.HWND) !void {
    const dpi = currentDpi(hwnd);
    const margin = win32.dip(12, dpi);
    const input_h = win32.dip(128, dpi);
    const response_h = win32.dip(272, dpi);
    const client_w = win32.dip(640, dpi);
    const inner = client_w - 2 * margin;
    const font = createFont(dpi);
    const richedit = text.loadRichEdit() catch {
        if (font != null) _ = c.DeleteObject(font);
        return error.ComposerFailed;
    };

    const input = text.createEdit(
        hwnd,
        c.WS_CHILD | c.WS_VISIBLE | c.WS_VSCROLL | c.ES_MULTILINE | c.ES_WANTRETURN | c.ES_NOHIDESEL,
        margin,
        margin,
        inner,
        input_h,
        32,
    ) catch {
        _ = c.FreeLibrary(richedit);
        if (font != null) _ = c.DeleteObject(font);
        return error.ComposerFailed;
    };
    _ = c.SendMessageW(input, c.EM_SETLIMITTEXT, 4096, 0);

    const response = text.createEdit(
        hwnd,
        c.WS_CHILD | c.WS_VISIBLE | c.WS_VSCROLL | c.ES_MULTILINE | c.ES_READONLY | c.ES_NOHIDESEL,
        margin,
        margin * 2 + input_h,
        inner,
        response_h,
        0,
    ) catch {
        _ = c.DestroyWindow(input);
        _ = c.FreeLibrary(richedit);
        if (font != null) _ = c.DeleteObject(font);
        return error.ComposerFailed;
    };
    _ = c.SendMessageW(response, c.EM_SETLIMITTEXT, 262144 + 8192, 0);
    text.setText(response, ui.config.history_prefix) catch {};

    const status = c.CreateWindowExW(0, win32.wideLit("Static").ptr, win32.wideLit("").ptr, c.WS_CHILD | c.WS_VISIBLE | c.SS_LEFT, margin, 0, inner - win32.dip(200, dpi), win32.dip(28, dpi), hwnd, null, c.GetModuleHandleW(null), null);
    const send = c.CreateWindowExW(0, win32.wideLit("Button").ptr, win32.wideLit("Send").ptr, c.WS_CHILD | c.WS_VISIBLE | c.BS_PUSHBUTTON, 0, 0, win32.dip(80, dpi), win32.dip(28, dpi), hwnd, hmenuId(8), c.GetModuleHandleW(null), null);
    const cancel = c.CreateWindowExW(0, win32.wideLit("Button").ptr, win32.wideLit("Cancel").ptr, c.WS_CHILD | c.WS_VISIBLE | c.BS_PUSHBUTTON, 0, 0, win32.dip(100, dpi), win32.dip(28, dpi), hwnd, hmenuId(12), c.GetModuleHandleW(null), null);

    const comp = try ui.gpa.create(Composer);
    comp.* = .{
        .hwnd = hwnd,
        .input = input,
        .response = response,
        .status = status orelse null,
        .send = send orelse null,
        .cancel = cancel orelse null,
        .font = font orelse null,
        .richedit = richedit,
        .dpi = dpi,
    };
    for ([_]struct { h: c.HWND, id: usize }{
        .{ .h = input, .id = 1 },
        .{ .h = response, .id = 2 },
        .{ .h = comp.status, .id = 3 },
        .{ .h = comp.send, .id = 4 },
        .{ .h = comp.cancel, .id = 5 },
    }) |entry| {
        if (entry.h != null) {
            _ = c.SetWindowSubclass(entry.h, controlSubclassProc, entry.id, @intFromPtr(ui));
            _ = c.SendMessageW(entry.h, c.WM_SETFONT, @intFromPtr(comp.font), 1);
        }
    }
    relayout(ui, comp);
    refreshStatus(ui);
    ui.composer = comp;
}

fn showComposer(ui: *Ui) !void {
    const comp = try ensureComposer(ui);
    _ = c.ShowWindow(comp.hwnd, c.SW_SHOWNORMAL);
    // Raise above unrelated non-topmost windows so the composer is actually
    // clickable; SetForegroundWindow alone can be denied by the foreground lock.
    _ = c.SetWindowPos(comp.hwnd, c.HWND_TOP, 0, 0, 0, 0,
        c.SWP_NOMOVE | c.SWP_NOSIZE);
    _ = c.SetForegroundWindow(comp.hwnd);
    _ = c.SetFocus(comp.input);
}

fn hideComposer(ui: *Ui) void {
    const comp = ui.composer orelse return;
    if (ui.composing) {
        if (ui.snapshot) |*snap| {
            const t = snap.text;
            text.cancelImeComposition(comp.input);
            text.setText(comp.input, t) catch {};
            text.setSelection(comp.input, snap.start, snap.end);
        }
        ui.composing = false;
    }
    _ = c.ShowWindow(comp.hwnd, c.SW_HIDE);
}

// ---------------------------------------------------------------- events

pub fn dispatchProviderEvents(ui: *Ui) void {
    for (0..64) |_| {
        const event = ui.ui_events.pop() orelse break;
        defer event.deinit(ui.gpa);
        switch (event) {
            .started => |e| {
                if (e.generation >= ui.model.generation) {
                    ui.model.generation = e.generation;
                    ui.model.provider_pid = @intCast(e.pid);
                }
                refreshStatus(ui);
            },
            .chunk => |e| {
                var accepted: ?i64 = null;
                if (eventMatches(e.generation, e.id, &ui.model)) {
                    if (appendBounded(&ui.model.response, ui.gpa, 262144, e.text)) |_| {
                        ui.model.last_seq = e.seq;
                        accepted = win32.qpc();
                    } else |_| {
                        setRunInvalid(ui, "response text exceeds native buffer contract");
                    }
                }
                if (accepted) |accepted_qpc| {
                    emitChunkAccepted(ui, e.id, e.seq, accepted_qpc);
                    appendResponseView(ui, e.text);
                } else {
                    if (ui.model.run_invalid != null) {
                        cancelRequest(ui) catch {};
                    }
                }
            },
            .terminal => |e| {
                const matched = eventMatches(e.generation, e.id, &ui.model);
                if (matched) {
                    ui.model.provider_state = switch (e.kind) {
                        .complete => "complete",
                        .cancelled => "cancelled",
                        .failed => "failed",
                    };
                    ui.model.last_seq = e.last_seq;
                    ui.cancel_pending = false;
                }
                if (matched) {
                    emitTerminal(ui, e.id, @tagName(e.kind), e.last_seq);
                }
                refreshStatus(ui);
            },
            .session_closed => |e| {
                if (e.generation == ui.model.generation) {
                    ui.model.provider_pid = 0;
                    if (e.err) |err| {
                        ui.model.provider_state = "failed";
                        setProviderError(ui, err);
                    }
                }
                refreshStatus(ui);
            },
            .stopped => |e| {
                if (ui.pending_shutdown_token) |token| {
                    var w = jsonw.Writer.init(ui.gpa);
                    defer w.deinit();
                    w.beginObject(null) catch {};
                    w.rawField("token", token) catch {};
                    if (e.err) |err| {
                        w.boolean("ok", false) catch {};
                        w.string("error", err) catch {};
                    } else {
                        w.boolean("ok", true) catch {};
                    }
                    w.endObject() catch {};
                    if (w.owned()) |rec| {
                        emitRecord(ui, rec);
                    } else |_| {}
                    ui.gpa.free(token);
                    ui.pending_shutdown_token = null;
                }
                ui.commands.close();
                if (ui.records) |q| q.close();
                c.PostQuitMessage(if (e.err != null) 1 else 0);
            },
        }
    }
}

fn setRunInvalid(ui: *Ui, msg: []const u8) void {
    if (ui.model.run_invalid) |old| ui.gpa.free(old);
    ui.model.run_invalid = ui.gpa.dupe(u8, msg) catch null;
}

fn setProviderError(ui: *Ui, msg: []const u8) void {
    if (ui.model.provider_error) |old| ui.gpa.free(old);
    ui.model.provider_error = ui.gpa.dupe(u8, msg) catch null;
}

fn emitRecord(ui: *Ui, record: []u8) void {
    const q = ui.records orelse {
        ui.gpa.free(record);
        return;
    };
    q.tryPush(record) catch {
        ui.gpa.free(record);
        setRunInvalid(ui, "control output queue overflow");
        std.debug.print("control output queue overflow\n", .{});
    };
}

fn emitChunkAccepted(ui: *Ui, id: u64, seq: u32, accepted_qpc: i64) void {
    var w = jsonw.Writer.init(ui.gpa);
    w.beginObject(null) catch {
        w.deinit();
        return;
    };
    w.string("event", "chunk_accepted") catch {};
    w.uint("request_id", id) catch {};
    w.uint("seq", seq) catch {};
    var buf: [24]u8 = undefined;
    const s = std.fmt.bufPrint(&buf, "{d}", .{accepted_qpc}) catch "0";
    w.string("accepted_qpc", s) catch {};
    w.endObject() catch {};
    const rec = w.owned() catch {
        w.deinit();
        return;
    };
    emitRecord(ui, rec);
}

fn emitTerminal(ui: *Ui, id: u64, kind: []const u8, last_seq: i64) void {
    var w = jsonw.Writer.init(ui.gpa);
    w.beginObject(null) catch {
        w.deinit();
        return;
    };
    w.string("event", "terminal") catch {};
    w.uint("request_id", id) catch {};
    w.string("kind", kind) catch {};
    w.int("last_seq", last_seq) catch {};
    var buf: [24]u8 = undefined;
    const s = std.fmt.bufPrint(&buf, "{d}", .{win32.qpc()}) catch "0";
    w.string("qpc", s) catch {};
    w.endObject() catch {};
    const rec = w.owned() catch {
        w.deinit();
        return;
    };
    emitRecord(ui, rec);
}

// --------------------------------------------------------------- submit

pub fn submit(ui: *Ui) !void {
    if (ui.shutdown_started) return error.ShutdownInProgress;
    if (std.mem.eql(u8, ui.model.provider_state, "streaming"))
        return error.RequestActive;
    if (ui.composing) return error.Composing;
    const input = inputHwnd(ui) orelse return error.NoComposer;
    const prompt = try text.getText(input);
    const new_normal = ui.gpa.dupe(u8, "normal") catch {
        ui.gpa.free(prompt);
        return error.OutOfMemory;
    };
    const scenario_owned = ui.model.scenario;
    ui.model.scenario = new_normal;
    const id = ui.model.request_count + 1;
    ui.model.request_count = id;
    ui.model.request_id = id;
    ui.model.last_seq = -1;
    ui.model.response.clearRetainingCapacity();
    if (ui.model.provider_error) |e| {
        ui.gpa.free(e);
        ui.model.provider_error = null;
    }
    ui.model.provider_state = "streaming";
    ui.cancel_pending = false;
    if (ui.provider) |p| {
        p.send(.{ .request = .{
            .id = id,
            .prompt = prompt,
            .scenario = scenario_owned,
        } }) catch {
            // send consumed and freed the command payload on failure.
            ui.model.provider_state = "failed";
            setRunInvalid(ui, "provider command queue unavailable");
            return error.ProviderFailed;
        };
    } else {
        ui.gpa.free(prompt);
        ui.gpa.free(scenario_owned);
    }
    resetResponseView(ui);
    refreshStatus(ui);
}

fn inputHwnd(ui: *Ui) ?c.HWND {
    const comp = ui.composer orelse return null;
    if (comp.input == null) return null;
    return comp.input;
}

fn resetResponseView(ui: *Ui) void {
    const comp = ui.composer orelse return;
    if (comp.response == null) return;
    text.setText(comp.response, ui.config.history_prefix) catch {};
}

fn appendResponseView(ui: *Ui, chunk: []const u8) void {
    const comp = ui.composer orelse return;
    if (comp.response == null) return;
    text.appendText(comp.response, chunk) catch {};
}

pub fn cancelRequest(ui: *Ui) !void {
    if (!std.mem.eql(u8, ui.model.provider_state, "streaming") or
        ui.model.request_id == 0 or ui.cancel_pending)
        return;
    const p = ui.provider orelse return;
    p.send(.{ .cancel = ui.model.request_id }) catch {
        setRunInvalid(ui, "cancel enqueue failed: provider command queue unavailable");
        return error.ProviderQueueUnavailable;
    };
    ui.cancel_pending = true;
}

pub fn requestShutdown(ui: *Ui, token: ?[]const u8) void {
    if (token) |t| {
        if (ui.pending_shutdown_token) |old| ui.gpa.free(old);
        ui.pending_shutdown_token = ui.gpa.dupe(u8, t) catch null;
    }
    if (ui.shutdown_started) return;
    ui.shutdown_started = true;
    hideComposer(ui);
    if (ui.provider) |p| {
        p.send(.shutdown) catch {
            std.debug.print("provider shutdown request failed\n", .{});
            c.PostQuitMessage(64);
        };
    } else {
        c.PostQuitMessage(0);
    }
}

// ------------------------------------------------------------ messages

fn mascotProc(hwnd: c.HWND, msg: c.UINT, w: c.WPARAM, l: c.LPARAM) callconv(.winapi) c.LRESULT {
    const ui = uiFromHwnd(hwnd);
    if (ui == null) {
        if (msg == c.WM_NCCREATE) {
            const cs: *c.CREATESTRUCTW = @ptrFromInt(@as(usize, @bitCast(l)));
            _ = c.SetWindowLongPtrW(hwnd, c.GWLP_USERDATA, @as(c.LONG_PTR, @bitCast(@as(usize, @intFromPtr(cs.lpCreateParams)))));
        }
        return c.DefWindowProcW(hwnd, msg, w, l);
    }
    switch (msg) {
        c.WM_NCDESTROY => {
            _ = c.SetWindowLongPtrW(hwnd, c.GWLP_USERDATA, 0);
            return c.DefWindowProcW(hwnd, msg, w, l);
        },
        WM_APP_PROVIDER => dispatchProviderEvents(ui.?),
        WM_APP_CONTROL => dispatchControl(ui.?),
        WM_APP_SUBMIT => submit(ui.?) catch |e| {
            std.debug.print("submit failed: {s}\n", .{@errorName(e)});
        },
        c.WM_DPICHANGED => onDpiChanged(ui.?, l),
        c.WM_NCHITTEST => return hitTest(ui.?, l),
        c.WM_HOTKEY => {
            if (w == HOTKEY_ID_SHOW) {
                toggleComposer(ui.?);
            } else if (w == HOTKEY_ID_CANCEL) {
                cancelRequest(ui.?) catch |e| {
                    std.debug.print("hotkey cancel failed: {s}\n", .{@errorName(e)});
                };
            }
            return 0;
        },
        c.WM_RBUTTONUP, c.WM_NCRBUTTONUP => {
            showExitMenu(ui.?);
            return 0;
        },
        c.WM_CLOSE => {
            requestShutdown(ui.?, null);
            return 0;
        },
        c.WM_DESTROY => {
            c.PostQuitMessage(0);
            return 0;
        },
        else => return c.DefWindowProcW(hwnd, msg, w, l),
    }
    return 0;
}

fn toggleComposer(ui: *Ui) void {
    if (ui.composer != null and c.IsWindowVisible(ui.composer.?.hwnd) != 0) {
        hideComposer(ui);
    } else {
        showComposer(ui) catch |e| {
            std.debug.print("composer show failed: {s}\n", .{@errorName(e)});
        };
    }
}

fn composerProc(hwnd: c.HWND, msg: c.UINT, w: c.WPARAM, l: c.LPARAM) callconv(.winapi) c.LRESULT {
    const ui = uiFromHwnd(hwnd);
    if (ui == null) {
        if (msg == c.WM_NCCREATE) {
            const cs: *c.CREATESTRUCTW = @ptrFromInt(@as(usize, @bitCast(l)));
            _ = c.SetWindowLongPtrW(hwnd, c.GWLP_USERDATA, @as(c.LONG_PTR, @bitCast(@as(usize, @intFromPtr(cs.lpCreateParams)))));
        }
        return c.DefWindowProcW(hwnd, msg, w, l);
    }
    switch (msg) {
        c.WM_CREATE => {
            createChildren(ui.?, hwnd) catch |e| {
                std.debug.print("composer children failed: {s}\n", .{@errorName(e)});
            };
            return 0;
        },
        c.WM_PAINT => {
            ui.?.paints += 1;
            return c.DefWindowProcW(hwnd, msg, w, l);
        },
        c.WM_SIZE => {
            if (ui.?.composer) |comp| relayout(ui.?, comp);
            return c.DefWindowProcW(hwnd, msg, w, l);
        },
        c.WM_COMMAND => {
            const control_id = w & 0xffff;
            const code = (w >> 16) & 0xffff;
            if (code == c.BN_CLICKED) {
                if (control_id == 8) {
                    submit(ui.?) catch |e| {
                        std.debug.print("submit failed: {s}\n", .{@errorName(e)});
                    };
                } else if (control_id == 12) {
                    cancelRequest(ui.?) catch {};
                }
            }
            return 0;
        },
        c.WM_CLOSE => {
            hideComposer(ui.?);
            return 0;
        },
        c.WM_DESTROY => {
            if (ui.?.composer) |comp| {
                if (comp.hwnd == hwnd) {
                    // Child windows are being destroyed; the system removes
                    // their subclasses automatically.
                    if (comp.font != null) _ = c.DeleteObject(comp.font);
                    _ = c.FreeLibrary(comp.richedit);
                    _ = c.OleUninitialize();
                    ui.?.gpa.destroy(comp);
                    ui.?.composer = null;
                }
            }
            return 0;
        },
        else => return c.DefWindowProcW(hwnd, msg, w, l),
    }
}

fn hmenuId(comptime id: usize) c.HMENU {
    // Child-control IDs travel in the hMenu slot; values must satisfy the
    // pointee alignment asserted by @ptrFromInt.
    return @ptrFromInt(id);
}

fn controlSubclassProc(
    hwnd: c.HWND,
    msg: c.UINT,
    w: c.WPARAM,
    l: c.LPARAM,
    subclass_id: c.UINT_PTR,
    reference: c.DWORD_PTR,
) callconv(.winapi) c.LRESULT {
    const ui: *Ui = @ptrFromInt(@as(usize, @intCast(reference)));
    if (msg == c.WM_PAINT) {
        ui.paints += 1;
        return c.DefSubclassProc(hwnd, msg, w, l);
    }
    if (subclass_id != 1) {
        return c.DefSubclassProc(hwnd, msg, w, l);
    }
    switch (msg) {
        c.WM_IME_STARTCOMPOSITION => onImeStart(ui, hwnd),
        c.WM_IME_COMPOSITION => ui.composing = true,
        c.WM_IME_ENDCOMPOSITION => ui.composing = false,
        c.WM_KEYDOWN => {
            if (w == c.VK_RETURN and (@as(u16, @bitCast(c.GetKeyState(c.VK_CONTROL))) & 0x8000) != 0 and !ui.composing) {
                _ = c.PostMessageW(ui.mascot, WM_APP_SUBMIT, 0, 0);
                return 0;
            }
        },
        else => {},
    }
    return c.DefSubclassProc(hwnd, msg, w, l);
}

fn onImeStart(ui: *Ui, hwnd: c.HWND) void {
    ui.composing = true;
    const t = text.getText(hwnd) catch return;
    const sel = text.selection(hwnd) catch {
        ui.gpa.free(t);
        return;
    };
    if (ui.snapshot) |*old| old.deinit(ui.gpa);
    ui.snapshot = .{ .text = t, .start = sel.start, .end = sel.end };
}

fn refreshStatus(ui: *Ui) void {
    const comp = ui.composer orelse return;
    if (comp.status == null) return;
    var w = jsonw.Writer.init(ui.gpa);
    defer w.deinit();
    // status text is plain provider state for now
    const wide = win32.wideAlloc(ui.gpa, ui.model.provider_state) catch return;
    defer ui.gpa.free(wide);
    _ = c.SetWindowTextW(comp.status, wide.ptr);
}

// ------------------------------------------------------------ control

fn replyOk(ui: *Ui, token_raw: []const u8, include_text: bool) !void {
    var w = jsonw.Writer.init(ui.gpa);
    defer w.deinit();
    const state = try stateJson(ui);
    defer ui.gpa.free(state);
    try w.beginObject(null);
    try w.rawField("token", token_raw);
    try w.boolean("ok", true);
    try w.rawField("state", state);
    if (include_text) {
        const t = try textJson(ui);
        defer ui.gpa.free(t);
        try w.rawField("text", t);
    }
    try w.endObject();
    const rec = try w.owned();
    emitRecord(ui, rec);
}

fn replyErr(ui: *Ui, token_raw: []const u8, msg: []const u8) void {
    var w = jsonw.Writer.init(ui.gpa);
    defer w.deinit();
    w.beginObject(null) catch return;
    w.rawField("token", token_raw) catch {};
    w.boolean("ok", false) catch {};
    w.string("error", msg) catch {};
    w.endObject() catch {};
    const rec = w.owned() catch return;
    emitRecord(ui, rec);
}

fn dispatchControl(ui: *Ui) void {
    for (0..16) |_| {
        const cmd = ui.commands.pop() orelse break;
        defer cmd.deinit(ui.gpa);
        dispatchCommand(ui, cmd.value) catch {};
    }
}

fn commandString(value: std.json.Value, key: []const u8) ?[]const u8 {
    if (value != .object) return null;
    const f = value.object.get(key) orelse return null;
    if (f != .string) return null;
    return f.string;
}

fn dispatchCommand(ui: *Ui, value: std.json.Value) !void {
    var token_owned: ?[]u8 = null;
    defer if (token_owned) |t| ui.gpa.free(t);
    const token: []const u8 = blk: {
        if (value != .object) break :blk "null";
        const t = value.object.get("token") orelse break :blk "null";
        token_owned = jsonw.serializeValue(ui.gpa, t) catch null;
        break :blk token_owned orelse "null";
    };
    const name = commandString(value, "command") orelse "";

    if (std.mem.eql(u8, name, "state")) {
        replyOk(ui, token, false) catch replyErr(ui, token, "state unavailable");
    } else if (std.mem.eql(u8, name, "text")) {
        const t = textJson(ui) catch {
            replyErr(ui, token, "composer not created");
            return;
        };
        defer ui.gpa.free(t);
        replyWithText(ui, token, t) catch {};
    } else if (std.mem.eql(u8, name, "set_text")) {
        const new_text = commandString(value, "text") orelse "";
        setInputText(ui, new_text) catch |e| {
            replyErr(ui, token, switch (e) {
                error.Nul => "text contains NUL",
                error.TooLong => "text exceeds input limit",
                else => "composer not created",
            });
            return;
        };
        replyOk(ui, token, false) catch {};
    } else if (std.mem.eql(u8, name, "scenario")) {
        const scenario = commandString(value, "name") orelse "";
        if (std.mem.eql(u8, ui.model.provider_state, "streaming")) {
            replyErr(ui, token, "request active");
            return;
        }
        if (!config.scenarioKnown(ui.config.manifest, scenario)) {
            var buf: [128]u8 = undefined;
            const msg = std.fmt.bufPrint(&buf, "unknown scenario '{s}'", .{scenario}) catch "unknown scenario";
            replyErr(ui, token, msg);
            return;
        }
        const duped = ui.gpa.dupe(u8, scenario) catch {
            replyErr(ui, token, "out of memory");
            return;
        };
        ui.gpa.free(ui.model.scenario);
        ui.model.scenario = duped;
        replyOk(ui, token, false) catch {};
    } else if (std.mem.eql(u8, name, "show")) {
        showComposer(ui) catch {
            replyErr(ui, token, "composer unavailable");
            return;
        };
        replyOk(ui, token, false) catch {};
    } else if (std.mem.eql(u8, name, "hide")) {
        hideComposer(ui);
        replyOk(ui, token, false) catch {};
    } else if (std.mem.eql(u8, name, "submit")) {
        submit(ui) catch |e| {
            replyErr(ui, token, switch (e) {
                error.ShutdownInProgress => "shutdown in progress",
                error.RequestActive => "a request is already active",
                error.Composing => "submit disabled while composing",
                error.NoComposer => "composer not created",
                else => "submit failed",
            });
            return;
        };
        replyOk(ui, token, false) catch {};
    } else if (std.mem.eql(u8, name, "cancel")) {
        cancelRequest(ui) catch {
            replyErr(ui, token, "cancel enqueue failed");
            return;
        };
        replyOk(ui, token, false) catch {};
    } else if (std.mem.eql(u8, name, "shutdown")) {
        requestShutdown(ui, token);
    } else {
        var buf: [128]u8 = undefined;
        const msg = std.fmt.bufPrint(&buf, "unknown command '{s}'", .{name}) catch "unknown command";
        replyErr(ui, token, msg);
    }
}

fn replyWithText(ui: *Ui, token_raw: []const u8, text_json: []const u8) !void {
    var w = jsonw.Writer.init(ui.gpa);
    defer w.deinit();
    const state = try stateJson(ui);
    defer ui.gpa.free(state);
    try w.beginObject(null);
    try w.rawField("token", token_raw);
    try w.boolean("ok", true);
    try w.rawField("state", state);
    try w.rawField("text", text_json);
    try w.endObject();
    emitRecord(ui, try w.owned());
}

fn setInputText(ui: *Ui, value: []const u8) !void {
    if (std.mem.indexOfScalar(u8, value, 0) != null) return error.Nul;
    if (text.utf16Units(value) > 4096) return error.TooLong;
    const comp = ui.composer orelse return error.NoComposer;
    try text.setText(comp.input, value);
    const sel = try text.selection(comp.input);
    const current = try text.getText(comp.input);
    if (ui.snapshot) |*old| old.deinit(ui.gpa);
    ui.snapshot = .{ .text = current, .start = sel.start, .end = sel.end };
}

fn stateJson(ui: *Ui) ![]u8 {
    var w = jsonw.Writer.init(ui.gpa);
    errdefer w.deinit();
    try w.beginObject(null);
    var buf: [32]u8 = undefined;
    const pid = c.GetCurrentProcessId();
    try w.int("pid", pid);
    try w.string("mascot_hwnd", hwndStr(&buf, ui.mascot));
    var buf2: [32]u8 = undefined;
    const comp_hwnd: c.HWND = if (ui.composer) |comp| comp.hwnd else null;
    try w.string("composer_hwnd", hwndStr(&buf2, comp_hwnd));
    var buf3: [32]u8 = undefined;
    var buf4: [32]u8 = undefined;
    const in_hwnd: c.HWND = if (ui.composer) |comp| comp.input else null;
    const resp_hwnd: c.HWND = if (ui.composer) |comp| comp.response else null;
    try w.string("input_hwnd", hwndStr(&buf3, in_hwnd));
    try w.string("response_hwnd", hwndStr(&buf4, resp_hwnd));
    const visible = if (ui.composer) |comp| c.IsWindowVisible(comp.hwnd) != 0 else false;
    try w.boolean("composer_visible", visible);
    try w.boolean("composing", ui.composing);
    try w.uint("provider_pid", if (ui.model.provider_pid != 0) ui.model.provider_pid else 0);
    try w.string("provider_state", ui.model.provider_state);
    try w.uint("request_id", ui.model.request_id);
    try w.uint("request_count", ui.model.request_count);
    try w.int("last_seq", ui.model.last_seq);
    try w.uint("response_utf8_bytes", ui.model.response.items.len);
    try w.uint("mascot_presents", ui.presents);
    try w.uint("composer_paints", ui.paints);
    try w.uint("queued_provider_frames", ui.ui_events.lenNow());
    try w.uint("queue_capacity_frames", ui.ui_events.capacity());
    try w.beginObject("cache_counts");
    const tail_total = if (ui.provider) |p| p.stderrTotal() else 0;
    try w.uint("stderr_tail_bytes", @min(tail_total, 4096));
    try w.uint("stderr_total_bytes", tail_total);
    try w.uint("retained_response_utf8_bytes", ui.model.response.items.len);
    try w.uint("dib_buffers", if (ui.surface != null) 1 else 0);
    try w.beginObject("richedit_layout_cache_bytes");
    try w.nullField("value");
    try w.string("reason", "opaque system RichEdit layout cache");
    try w.endObject();
    try w.beginObject("caret_draws");
    try w.nullField("value");
    try w.string("reason", "native caret drawing does not surface through WM_PAINT");
    try w.endObject();
    if (ui.model.run_invalid) |e| try w.string("run_invalid", e) else try w.nullField("run_invalid");
    if (ui.model.provider_error) |e| try w.string("provider_error", e) else try w.nullField("provider_error");
    try w.endObject(); // cache_counts
    try w.endObject(); // state
    return w.owned();
}

fn textJson(ui: *Ui) ![]u8 {
    const comp = ui.composer orelse return error.NoComposer;
    var w = jsonw.Writer.init(ui.gpa);
    errdefer w.deinit();
    const input_text = try text.getText(comp.input);
    defer ui.gpa.free(input_text);
    const response_full = try text.getText(comp.response);
    defer ui.gpa.free(response_full);
    const response = if (std.mem.startsWith(u8, response_full, ui.config.history_prefix))
        response_full[ui.config.history_prefix.len..]
    else
        response_full;
    const sel = try text.selection(comp.input);
    try w.beginObject(null);
    try w.string("input", input_text);
    try w.string("response", response);
    try w.uint("selection_start", sel.start);
    try w.uint("selection_end", sel.end);
    try w.boolean("composing", ui.composing);
    try w.endObject();
    return w.owned();
}

fn hwndStr(buf: []u8, hwnd: c.HWND) []const u8 {
    const v = @intFromPtr(hwnd orelse return "0");
    return std.fmt.bufPrint(buf, "{d}", .{v}) catch "0";
}

// -------------------------------------------------------------- entry

pub fn teardown(ui: *Ui) void {
    if (ui.provider) |p| {
        if (!p.join(3000)) {
            std.debug.print("provider coordinator did not stop within teardown bound{s}", .{"\n"});
        }
    }
    ui.commands.close();
    if (ui.records) |q| q.close();
    if (ui.composer) |comp| {
        _ = c.DestroyWindow(comp.hwnd);
    }
    if (ui.snapshot) |*snap| {
        snap.deinit(ui.gpa);
        ui.snapshot = null;
    }
    if (ui.surface) |s| {
        s.deinit();
        ui.surface = null;
    }
    if (ui.mascot != null) {
        _ = c.DestroyWindow(ui.mascot);
        ui.mascot = null;
    }
    ui.model.deinit(ui.gpa);
}

/// Decode the asset PNG to a premultiplied BGRA buffer via WIC.
/// The manifest SHA-256 pins the bytes; dimensions are returned for the
/// caller to cross-check against asset.pixel_width/pixel_height.
pub const DecodedPng = struct { pixels: []u8, w: usize, h: usize };
pub fn decodePng(gpa: Allocator, path: []const u8) !DecodedPng {
    var factory: ?*c.IWICImagingFactory = null;
    const hr = c.CoCreateInstance(&c.CLSID_WICImagingFactory, null, c.CLSCTX_INPROC_SERVER, &c.IID_IWICImagingFactory, @ptrCast(&factory));
    if (hr < 0 or factory == null) return error.WicFailed;
    defer _ = factory.?.lpVtbl.*.Release.?(factory.?);

    const wide_path = try win32.wideAlloc(gpa, path);
    defer gpa.free(wide_path);
    var decoder: ?*c.IWICBitmapDecoder = null;
    var hr2 = factory.?.lpVtbl.*.CreateDecoderFromFilename.?(factory.?, wide_path.ptr, null, c.GENERIC_READ, c.WICDecodeMetadataCacheOnLoad, &decoder);
    if (hr2 < 0 or decoder == null) return error.WicFailed;
    defer _ = decoder.?.lpVtbl.*.Release.?(decoder.?);

    var frame_count: c.UINT = 0;
    hr2 = decoder.?.lpVtbl.*.GetFrameCount.?(decoder.?, &frame_count);
    if (hr2 < 0 or frame_count < 1) return error.WicFailed;
    var frame: ?*c.IWICBitmapFrameDecode = null;
    hr2 = decoder.?.lpVtbl.*.GetFrame.?(decoder.?, 0, &frame);
    if (hr2 < 0 or frame == null) return error.WicFailed;
    defer _ = frame.?.lpVtbl.*.Release.?(frame.?);

    var converter: ?*c.IWICFormatConverter = null;
    hr2 = factory.?.lpVtbl.*.CreateFormatConverter.?(factory.?, &converter);
    if (hr2 < 0 or converter == null) return error.WicFailed;
    defer _ = converter.?.lpVtbl.*.Release.?(converter.?);

    // convert to premultiplied BGRA
    hr2 = converter.?.lpVtbl.*.Initialize.?(converter.?, @ptrCast(frame.?), &c.GUID_WICPixelFormat32bppPBGRA, c.WICBitmapDitherTypeNone, null, 0.0, c.WICBitmapPaletteTypeCustom);
    if (hr2 < 0) return error.WicFailed;
    var w: c.UINT = 0;
    var h: c.UINT = 0;
    hr2 = converter.?.lpVtbl.*.GetSize.?(converter.?, &w, &h);
    if (hr2 < 0 or w == 0 or h == 0) return error.WicFailed;
    const stride: usize = @as(usize, @intCast(w)) * 4;
    const out = try gpa.alloc(u8, stride * @as(usize, @intCast(h)));
    errdefer gpa.free(out);
    hr2 = converter.?.lpVtbl.*.CopyPixels.?(converter.?, null, @intCast(stride), @intCast(out.len), out.ptr);
    if (hr2 < 0) return error.WicFailed;
    return .{ .pixels = out, .w = @intCast(w), .h = @intCast(h) };
}
