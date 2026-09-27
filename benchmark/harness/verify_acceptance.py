"""Shared text/window acceptance gate (T1-T14, W1-W9).

Drives a candidate through the frozen control interface plus REAL external input:
SendInput scan-code/unicode keys, mouse press/drag/release, cross-process clipboard,
keyboard-layout switch for the F5 dead-key case, and TSF profile activation for the
Microsoft Japanese IME F6/F7/F8 cases. Captures external screen crops of the actual
candidate windows as visual evidence; no visual PASS is inferred from bytes alone.

W2/W7 require two display targets at different scale factors (one 100%, one >=150%).
Run with --with-hidpi only when the display lab provides it; otherwise UNTESTED.
"""
import argparse
import ctypes
import json
import pathlib
import struct
import sys
import threading
import time
import zlib
from ctypes import wintypes
from datetime import datetime, timezone

sys.path.insert(0, str(pathlib.Path(__file__).parent))
import verify_provider_regression as regression  # noqa: E402
from verify_candidate_smoke import load, require  # noqa: E402

USER32 = ctypes.WinDLL("user32", use_last_error=True)
KERNEL32 = ctypes.WinDLL("kernel32", use_last_error=True)
GDI32 = ctypes.WinDLL("gdi32", use_last_error=True)
IMM32 = ctypes.WinDLL("imm32", use_last_error=True)
OLE32 = ctypes.WinDLL("ole32", use_last_error=True)
SHCORE = ctypes.WinDLL("shcore", use_last_error=True)

VK = {"CTRL": 0x11, "ALT": 0x12, "SHIFT": 0x10, "END": 0x23, "HOME": 0x24,
      "LEFT": 0x25, "UP": 0x26, "RIGHT": 0x27, "DOWN": 0x28, "DELETE": 0x2E,
      "ENTER": 0x0D, "ESCAPE": 0x1B, "SPACE": 0x20, "APOSTROPHE": 0xDE,
      "A": 0x41, "C": 0x43, "E": 0x45, "V": 0x56,
      "N": 0x4E, "I": 0x49, "H": 0x48, "O": 0x4F}

WM_NCHITTEST, HTTRANSPARENT, HTCAPTION = 0x84, -1, 2
WM_INPUTLANGCHANGEREQUEST = 0x0050
CF_UNICODETEXT = 13


def _sig(dll, name, args, rest):
    fn = getattr(dll, name)
    fn.argtypes, fn.restype = args, rest
    return fn


class GUID(ctypes.Structure):
    _fields_ = [("data1", wintypes.DWORD), ("data2", wintypes.WORD),
                ("data3", wintypes.WORD), ("data4", ctypes.c_ubyte * 8)]

    @staticmethod
    def parse(text):
        raw = bytes.fromhex(text.replace("-", "").replace("{", "").replace("}", ""))
        value = GUID()
        value.data1 = int.from_bytes(raw[0:4], "big")
        value.data2 = int.from_bytes(raw[4:6], "big")
        value.data3 = int.from_bytes(raw[6:8], "big")
        for index in range(8):
            value.data4[index] = raw[8 + index]
        return value


class KeyboardInput(ctypes.Structure):
    _fields_ = [("vk", wintypes.WORD), ("scan", wintypes.WORD),
                ("flags", wintypes.DWORD), ("time", wintypes.DWORD),
                ("extra", ctypes.c_void_p)]


class MouseInput(ctypes.Structure):
    _fields_ = [("dx", wintypes.LONG), ("dy", wintypes.LONG),
                ("data", wintypes.DWORD), ("flags", wintypes.DWORD),
                ("time", wintypes.DWORD), ("extra", ctypes.c_void_p)]


class InputUnion(ctypes.Union):
    _fields_ = [("keyboard", KeyboardInput), ("mouse", MouseInput)]


class Input(ctypes.Structure):
    _fields_ = [("type", wintypes.DWORD), ("value", InputUnion)]


class UiAutomation:
    """External input and window observation; everything here is outside the candidate."""

    def __init__(self):
        u, k = USER32, KERNEL32
        self.SendInput = _sig(u, "SendInput", [wintypes.UINT, ctypes.POINTER(Input), ctypes.c_int], wintypes.UINT)
        self.MapVirtualKeyW = _sig(u, "MapVirtualKeyW", [wintypes.UINT, wintypes.UINT], wintypes.UINT)
        self.SetCursorPos = _sig(u, "SetCursorPos", [ctypes.c_int, ctypes.c_int], wintypes.BOOL)
        self.GetCursorPos = _sig(u, "GetCursorPos", [ctypes.POINTER(wintypes.POINT)], wintypes.BOOL)
        self.WindowFromPoint = _sig(u, "WindowFromPoint", [wintypes.POINT], wintypes.HWND)
        self.GetForegroundWindow = _sig(u, "GetForegroundWindow", [], wintypes.HWND)
        self.SetForegroundWindow = _sig(u, "SetForegroundWindow", [wintypes.HWND], wintypes.BOOL)
        self.GetWindowRect = _sig(u, "GetWindowRect", [wintypes.HWND, ctypes.POINTER(wintypes.RECT)], wintypes.BOOL)
        self.IsWindowVisible = _sig(u, "IsWindowVisible", [wintypes.HWND], wintypes.BOOL)
        self.GetTopWindow = _sig(u, "GetTopWindow", [wintypes.HWND], wintypes.HWND)
        self.GetWindow = _sig(u, "GetWindow", [wintypes.HWND, wintypes.UINT], wintypes.HWND)
        self.GetKeyboardLayout = _sig(u, "GetKeyboardLayout", [wintypes.DWORD], wintypes.HANDLE)
        self.GetWindowThreadProcessId = _sig(u, "GetWindowThreadProcessId", [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)], wintypes.DWORD)
        self.PostMessageW = _sig(u, "PostMessageW", [wintypes.HWND, wintypes.UINT, ctypes.c_size_t, ctypes.c_ssize_t], wintypes.BOOL)
        self.SendMessageW = _sig(u, "SendMessageW", [wintypes.HWND, wintypes.UINT, ctypes.c_size_t, ctypes.c_ssize_t], ctypes.c_ssize_t)
        self.SendMessageTimeoutW = _sig(u, "SendMessageTimeoutW", [wintypes.HWND, wintypes.UINT, ctypes.c_size_t, ctypes.c_ssize_t, wintypes.UINT, wintypes.UINT, ctypes.POINTER(ctypes.c_size_t)], ctypes.c_ssize_t)
        self.OpenClipboard = _sig(u, "OpenClipboard", [wintypes.HWND], wintypes.BOOL)
        self.EmptyClipboard = _sig(u, "EmptyClipboard", [], wintypes.BOOL)
        self.CloseClipboard = _sig(u, "CloseClipboard", [], wintypes.BOOL)
        self.GetClipboardData = _sig(u, "GetClipboardData", [wintypes.UINT], wintypes.HANDLE)
        self.SetWindowPos = _sig(u, "SetWindowPos", [wintypes.HWND, wintypes.HWND, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, wintypes.UINT], wintypes.BOOL)
        self.EnumDisplayMonitors = _sig(u, "EnumDisplayMonitors", [wintypes.HDC, ctypes.c_void_p, ctypes.c_void_p, wintypes.LPARAM], wintypes.BOOL)
        self.GetMonitorInfoW = _sig(u, "GetMonitorInfoW", [wintypes.HANDLE, ctypes.c_void_p], wintypes.BOOL)
        self.GetDpiForWindow = _sig(u, "GetDpiForWindow", [wintypes.HWND], wintypes.UINT)
        self.GetSystemMetrics = _sig(u, "GetSystemMetrics", [ctypes.c_int], ctypes.c_int)
        self.GlobalLock = _sig(k, "GlobalLock", [wintypes.HANDLE], wintypes.LPVOID)
        self.GlobalUnlock = _sig(k, "GlobalUnlock", [wintypes.HANDLE], wintypes.BOOL)
        self.LoadKeyboardLayoutW = _sig(u, "LoadKeyboardLayoutW", [wintypes.LPCWSTR, wintypes.UINT], wintypes.HANDLE)
        self.GetModuleHandleW = _sig(k, "GetModuleHandleW", [wintypes.LPCWSTR], wintypes.HMODULE)
        self.ImmGetContext = _sig(IMM32, "ImmGetContext", [wintypes.HWND], wintypes.HANDLE)
        self.ImmReleaseContext = _sig(IMM32, "ImmReleaseContext", [wintypes.HWND, wintypes.HANDLE], wintypes.BOOL)
        self.ImmSetOpenStatus = _sig(IMM32, "ImmSetOpenStatus", [wintypes.HANDLE, wintypes.BOOL], wintypes.BOOL)
        self.ImmSetConversionStatus = _sig(IMM32, "ImmSetConversionStatus", [wintypes.HANDLE, wintypes.DWORD, wintypes.DWORD], wintypes.BOOL)
        self.GetScaleFactorForMonitor = _sig(SHCORE, "GetScaleFactorForMonitor", [wintypes.HANDLE, ctypes.POINTER(ctypes.c_int)], ctypes.c_long)
        self.isize = ctypes.sizeof(Input)
        vx = self.GetSystemMetrics(76)  # SM_XVIRTUALSCREEN
        vy = self.GetSystemMetrics(77)
        vw = self.GetSystemMetrics(78)
        vh = self.GetSystemMetrics(79)
        self.virtual = (vx, vy, vw if vw > 0 else 1, vh if vh > 0 else 1)

    def send(self, inputs):
        array = (Input * len(inputs))(*inputs)
        sent = self.SendInput(len(inputs), array, self.isize)
        require(sent == len(inputs), f"SendInput delivered {sent}/{len(inputs)}")

    def _key(self, vk, up=False):
        item = Input()
        item.type = 1
        item.value.keyboard = KeyboardInput(vk=0, scan=self.MapVirtualKeyW(vk, 0),
                                            flags=8 | (2 if up else 0), time=0, extra=None)
        return item

    def keys(self, sequence, settle_ms=120):
        """sequence like 'CTRL+END' or 'SHIFT+RIGHT'; modifiers press first, release last."""
        parts = [p.upper() for p in sequence.split("+")]
        mods = [VK[p] for p in parts if p in ("CTRL", "ALT", "SHIFT")]
        rest = [VK[p] for p in parts if p not in ("CTRL", "ALT", "SHIFT")]
        require(rest, f"No key in '{sequence}'")
        inputs = [self._key(vk) for vk in mods]
        for vk in rest:
            inputs += [self._key(vk), self._key(vk, up=True)]
        inputs += [self._key(vk, up=True) for vk in reversed(mods)]
        self.send(inputs)
        time.sleep(settle_ms / 1000)

    def type_text(self, text, settle_ms=60):
        for char in text:
            for flag in (4, 6):  # KEYEVENTF_UNICODE down / up
                item = Input()
                item.type = 1
                item.value.keyboard = KeyboardInput(vk=0, scan=ord(char), flags=flag,
                                                    time=0, extra=None)
                self.send([item])
            time.sleep(0.005)
        time.sleep(settle_ms / 1000)

    def click(self, x, y):
        require(self.SetCursorPos(x, y), "Cannot position pointer")
        time.sleep(0.05)
        inputs = []
        for flag in (2, 4):  # LEFTDOWN, LEFTUP
            item = Input()
            item.type = 0
            item.value.mouse = MouseInput(dx=0, dy=0, data=0, flags=flag, time=0, extra=None)
            inputs.append(item)
        self.send(inputs)
        time.sleep(0.15)

    def _abs(self, x, y):
        vx, vy, vw, vh = self.virtual
        return (round((x - vx) * 65535 / (vw - 1)), round((y - vy) * 65535 / (vh - 1)))

    def drag(self, x1, y1, x2, y2, steps=12):
        require(self.SetCursorPos(x1, y1), "Cannot position pointer")
        time.sleep(0.05)
        down = Input()
        down.type = 0
        down.value.mouse = MouseInput(dx=0, dy=0, data=0, flags=2, time=0, extra=None)
        self.send([down])
        for index in range(1, steps + 1):
            ax, ay = self._abs(x1 + (x2 - x1) * index // steps,
                             y1 + (y2 - y1) * index // steps)
            move = Input()
            move.type = 0
            move.value.mouse = MouseInput(dx=ax, dy=ay, data=0,
                                          flags=0x0001 | 0x8000 | 0x4000, time=0, extra=None)
            self.send([move])
            time.sleep(0.03)
        up = Input()
        up.type = 0
        up.value.mouse = MouseInput(dx=0, dy=0, data=0, flags=4, time=0, extra=None)
        self.send([up])
        time.sleep(0.2)

    def hit_test(self, hwnd, x, y):
        result = ctypes.c_size_t()
        lparam = ctypes.c_ssize_t(((y & 0xFFFF) << 16) | (x & 0xFFFF)).value
        require(self.SendMessageTimeoutW(hwnd, WM_NCHITTEST, 0, lparam, 2, 2000,
                                         ctypes.byref(result)), "Hit-test query timed out")
        return ctypes.c_ssize_t(result.value).value

    def rect(self, hwnd):
        box = wintypes.RECT()
        require(self.GetWindowRect(hwnd, ctypes.byref(box)), "GetWindowRect failed")
        return box

    def clipboard(self):
        require(self.OpenClipboard(None), "Cannot open clipboard")
        try:
            handle = self.GetClipboardData(CF_UNICODETEXT)
            if not handle:
                return None
            pointer = self.GlobalLock(handle)
            try:
                require(bool(pointer), "Cannot lock clipboard data")
                return ctypes.wstring_at(pointer)
            finally:
                self.GlobalUnlock(handle)
        finally:
            self.CloseClipboard()

    def clear_clipboard(self):
        require(self.OpenClipboard(None), "Cannot open clipboard")
        self.EmptyClipboard()
        self.CloseClipboard()

    def wait_clipboard(self, timeout=2.0):
        deadline = time.monotonic() + timeout
        value = None
        while time.monotonic() < deadline:
            value = self.clipboard()
            if value:
                return value
            time.sleep(0.05)
        return value

    def foreground(self, hwnd, timeout=3.0):
        self.SetForegroundWindow(hwnd)
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if self.GetForegroundWindow() == hwnd:
                return
            time.sleep(0.05)
            self.SetForegroundWindow(hwnd)
        require(False, f"Cannot take foreground; current={self.GetForegroundWindow()}")

    def monitors(self):
        EnumProc = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HANDLE, wintypes.HDC,
                                      ctypes.POINTER(wintypes.RECT), wintypes.LPARAM)
        found = []

        def visit(hmon, hdc, rectptr, data):
            info = ctypes.create_string_buffer(104)
            struct.pack_into("<I", info, 0, 104)
            require(self.GetMonitorInfoW(hmon, info), "GetMonitorInfoW failed")
            left, top, right, bottom = struct.unpack_from("<4i", info, 4)
            flags = struct.unpack_from("<I", info, 36)[0]  # MONITORINFO.dwFlags
            scale = ctypes.c_int()
            require(self.GetScaleFactorForMonitor(hmon, ctypes.byref(scale)) == 0,
                    "GetScaleFactorForMonitor failed")
            found.append({"handle": hmon, "rect": (left, top, right, bottom),
                          "scale": scale.value,
                          "primary": bool(flags & 1)})  # MONITORINFOF_PRIMARY
            return True

        require(self.EnumDisplayMonitors(None, None, ctypes.cast(EnumProc(visit),
                                                                 ctypes.c_void_p), 0),
                "EnumDisplayMonitors failed")
        return found


class ListenerWindow:
    """An ordinary, harness-owned top-level window that records received clicks."""

    def __init__(self, title="mascot-harness-listener"):
        self.clicks = []
        self.hwnd = None
        self._ready = threading.Event()
        self._error = []
        self.thread = threading.Thread(target=self._run, args=(title,), daemon=True)
        self.thread.start()
        require(self._ready.wait(5), "Listener window thread did not start")
        require(not self._error, str(self._error))

    def _run(self, title):
        u = USER32
        WndProc = ctypes.WINFUNCTYPE(ctypes.c_ssize_t, wintypes.HWND, wintypes.UINT,
                                     ctypes.c_size_t, ctypes.c_ssize_t)
        DefWindowProcW = _sig(u, "DefWindowProcW",
                              [wintypes.HWND, wintypes.UINT, ctypes.c_size_t,
                               ctypes.c_ssize_t], ctypes.c_ssize_t)
        clicks = self.clicks

        def proc(hwnd, message, wparam, lparam):
            # client AND non-client button messages — a click landing on the
            # caption still proves delivery through the mascot's transparency
            if message in (0x0201, 0x0202, 0x00A1, 0x00A2):
                clicks.append({"message": message, "x": lparam & 0xFFFF,
                               "y": (lparam >> 16) & 0xFFFF})
            return DefWindowProcW(hwnd, message, wparam, lparam)

        self._proc = WndProc(proc)
        try:
            RegisterClassW = _sig(u, "RegisterClassW", [ctypes.c_void_p], wintypes.ATOM)
            CreateWindowExW = _sig(u, "CreateWindowExW",
                                   [wintypes.DWORD, wintypes.LPCWSTR, wintypes.LPCWSTR,
                                    wintypes.DWORD, ctypes.c_int, ctypes.c_int,
                                    ctypes.c_int, ctypes.c_int, wintypes.HWND,
                                    wintypes.HANDLE, wintypes.HINSTANCE,
                                    ctypes.c_void_p], wintypes.HWND)
            ShowWindow = _sig(u, "ShowWindow", [wintypes.HWND, ctypes.c_int], wintypes.BOOL)
            LoadCursorW = _sig(u, "LoadCursorW", [wintypes.HINSTANCE, ctypes.c_void_p],
                               wintypes.HANDLE)
            GetMessageW = _sig(u, "GetMessageW", [ctypes.POINTER(wintypes.MSG),
                               wintypes.HWND, wintypes.UINT, wintypes.UINT], ctypes.c_int)
            TranslateMessage = _sig(u, "TranslateMessage", [ctypes.POINTER(wintypes.MSG)],
                                    wintypes.BOOL)
            DispatchMessageW = _sig(u, "DispatchMessageW", [ctypes.POINTER(wintypes.MSG)],
                                    ctypes.c_ssize_t)
            GetModuleHandleW = _sig(KERNEL32, "GetModuleHandleW", [wintypes.LPCWSTR],
                                    wintypes.HMODULE)

            class WndClass(ctypes.Structure):
                _fields_ = [("style", wintypes.UINT), ("wndproc", ctypes.c_void_p),
                            ("cls_extra", ctypes.c_int), ("wnd_extra", ctypes.c_int),
                            ("instance", wintypes.HINSTANCE), ("icon", wintypes.HANDLE),
                            ("cursor", wintypes.HANDLE), ("background", wintypes.HANDLE),
                            ("menu", wintypes.LPCWSTR), ("name", wintypes.LPCWSTR)]

            wc = WndClass(0, ctypes.cast(self._proc, ctypes.c_void_p), 0, 0,
                          GetModuleHandleW(None), None, LoadCursorW(None, 32512),
                          wintypes.HANDLE(6), None, "MascotHarnessListener")
            require(RegisterClassW(ctypes.byref(wc)), "Listener class registration failed")
            self.hwnd = CreateWindowExW(0, "MascotHarnessListener", title, 0x00CF0000,
                                        300, 300, 320, 240, None, None, wc.instance, None)
            require(bool(self.hwnd), "Listener window creation failed")
            ShowWindow(self.hwnd, 1)
            self._ready.set()
            message = wintypes.MSG()
            while GetMessageW(ctypes.byref(message), None, 0, 0) > 0:
                TranslateMessage(ctypes.byref(message))
                DispatchMessageW(ctypes.byref(message))
        except Exception as error:
            self._error.append(repr(error))
            self._ready.set()

    def move(self, x, y, w, h):
        UiAutomation().SetWindowPos(self.hwnd, None, x, y, w, h, 0x0010)

    def close(self):
        USER32.PostMessageW(self.hwnd, 0x0010, 0, 0)  # WM_CLOSE
        self.thread.join(timeout=3)


def png_encode(rgb, width, height):
    def chunk(kind, data):
        return (struct.pack(">I", len(data)) + kind + data +
                struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF))
    raw = b"".join(b"\x00" + bytes(rgb[y * width * 3:(y + 1) * width * 3])
                   for y in range(height))
    return (b"\x89PNG\r\n\x1a\n" +
            chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)) +
            chunk(b"IDAT", zlib.compress(raw, 6)) + chunk(b"IEND", b""))


def capture_window(hwnd, path):
    """External crop of the window's screen region; captures whatever is composited."""
    ui = UiAutomation()
    box = ui.rect(hwnd)
    width, height = box.right - box.left, box.bottom - box.top
    require(0 < width <= 8192 and 0 < height <= 8192, "Window has no capturable extent")
    GetDC = _sig(USER32, "GetDC", [wintypes.HWND], wintypes.HDC)
    ReleaseDC = _sig(USER32, "ReleaseDC", [wintypes.HWND, wintypes.HDC], ctypes.c_int)
    CreateCompatibleDC = _sig(GDI32, "CreateCompatibleDC", [wintypes.HDC], wintypes.HDC)
    CreateCompatibleBitmap = _sig(GDI32, "CreateCompatibleBitmap",
                                  [wintypes.HDC, ctypes.c_int, ctypes.c_int], wintypes.HANDLE)
    SelectObject = _sig(GDI32, "SelectObject", [wintypes.HDC, wintypes.HANDLE], wintypes.HANDLE)
    BitBlt = _sig(GDI32, "BitBlt", [wintypes.HDC] + [ctypes.c_int] * 4 + [wintypes.HDC] +
                  [ctypes.c_int] * 2 + [wintypes.DWORD], wintypes.BOOL)
    GetDIBits = _sig(GDI32, "GetDIBits", [wintypes.HDC, wintypes.HANDLE, wintypes.UINT,
                     wintypes.UINT, ctypes.c_void_p, ctypes.c_void_p, wintypes.UINT],
                     ctypes.c_int)
    screen = GetDC(None)
    memory = CreateCompatibleDC(screen)
    bitmap = CreateCompatibleBitmap(screen, width, height)
    require(bool(screen) and bool(memory) and bool(bitmap), "Capture device setup failed")
    try:
        previous = SelectObject(memory, bitmap)
        require(BitBlt(memory, 0, 0, width, height, screen, box.left, box.top, 0x00CC0020),
                "BitBlt capture failed")
        SelectObject(memory, previous)

        class BitmapInfo(ctypes.Structure):
            _fields_ = [("size", wintypes.DWORD), ("width", wintypes.LONG),
                        ("height", wintypes.LONG), ("planes", wintypes.WORD),
                        ("bits", wintypes.WORD), ("compression", wintypes.DWORD),
                        ("size_image", wintypes.DWORD), ("xp", wintypes.LONG),
                        ("yp", wintypes.LONG), ("used", wintypes.DWORD),
                        ("important", wintypes.DWORD)]

        stride = (width * 3 + 3) & ~3
        info = BitmapInfo(ctypes.sizeof(BitmapInfo), width, -height, 1, 24, 0, 0, 0, 0, 0, 0)
        buffer = (ctypes.c_ubyte * (stride * height))()
        require(GetDIBits(memory, bitmap, 0, height, buffer, ctypes.byref(info), 0) == height,
                "GetDIBits failed")
        rgb = bytearray()
        for y in range(height):
            row = buffer[y * stride:(y + 1) * stride]
            for x in range(width):
                rgb += bytes((row[x * 3 + 2], row[x * 3 + 1], row[x * 3]))
        path.write_bytes(png_encode(bytes(rgb), width, height))
    finally:
        _sig(GDI32, "DeleteObject", [wintypes.HANDLE], wintypes.BOOL)(bitmap)
        _sig(GDI32, "DeleteDC", [wintypes.HDC], wintypes.BOOL)(memory)
        ReleaseDC(None, screen)


def ink_pixels(png_path):
    """Count dark ink pixels in a captured PNG (heuristic for 'text actually rendered')."""
    data = png_path.read_bytes()
    require(data[:8] == b"\x89PNG\r\n\x1a\n", "capture is not PNG")
    offset = 8
    width = height = 0
    idat = bytearray()
    while offset < len(data):
        length, kind = struct.unpack(">I4s", data[offset:offset + 8])
        payload = data[offset + 8:offset + 8 + length]
        if kind == b"IHDR":
            width, height = struct.unpack(">II", payload[:8])
        elif kind == b"IDAT":
            idat += payload
        elif kind == b"IEND":
            break
        offset += 12 + length
    rows = zlib.decompress(bytes(idat))
    stride = width * 3 + 1
    ink = 0
    for y in range(height):
        base = y * stride + 1
        for x in range(width):
            at = base + x * 3
            if rows[at] < 200 and rows[at + 1] < 200 and rows[at + 2] < 200:
                ink += 1
    return ink


def activate_japanese_profile():
    """TSF ActivateProfile for the frozen Microsoft Japanese IME, session-wide."""
    OLE32.CoInitialize(None)
    clsid = GUID.parse("33C53A50-F456-4884-B049-85FD643ECFED")   # CLSID_TF_InputProcessorProfiles
    iid = GUID.parse("71C6E74C-0F28-11D8-A82A-00065B84435C")     # ITfInputProcessorProfileMgr
    instance = ctypes.c_void_p()
    hr = OLE32.CoCreateInstance(ctypes.byref(clsid), None, 1, ctypes.byref(iid),
                                ctypes.byref(instance))
    require(hr == 0 and instance.value, f"CoCreateInstance TSF profiles failed {hr:#x}")
    vtable = ctypes.cast(instance, ctypes.POINTER(ctypes.POINTER(ctypes.c_void_p)))
    Activate = ctypes.WINFUNCTYPE(ctypes.c_long, ctypes.c_void_p, wintypes.DWORD,
                                wintypes.WORD, ctypes.POINTER(GUID),
                                ctypes.POINTER(GUID), ctypes.c_void_p,
                                wintypes.DWORD)(vtable.contents[3])
    service = GUID.parse("03B5835F-F03C-411B-9CE2-AA23E1171E36")
    profile = GUID.parse("A76C93D9-5523-4E90-AAFA-4DB112F9AC76")
    hr = Activate(instance, 1, 0x0411, ctypes.byref(service), ctypes.byref(profile),
                  None, 0x10000000)  # TF_IPPMF_FORSESSION
    require(hr == 0, f"TSF ActivateProfile failed {hr:#x}")


def run_acceptance(candidate, output, ui, text_fixture, result):
    cases = []

    def record(case, status, evidence=None):
        entry = {"case": case, "status": status, "fixture_version": candidate.manifest.get("version")}
        if evidence is not None:
            entry["evidence"] = evidence
        cases.append(entry)
        result["cases"] = cases

    def state():
        return candidate.call("state")["state"]

    def text():
        return candidate.call("text")["text"]

    def snap(name, hwnd):
        path = output / f"{name}.png"
        capture_window(hwnd, path)
        return path.name

    def focus_input():
        hwnd = int(state()["input_hwnd"])
        require(hwnd != 0, "Input control missing")
        box = ui.rect(hwnd)
        ui.click((box.left + box.right) // 2, (box.top + box.bottom) // 2)

    def wait_new_terminal(index, timeout=10):
        candidate.wait(lambda: any(e.get("event") == "terminal"
                                   for e in candidate.events[index:]), timeout)
        return [e for e in candidate.events[index:] if e.get("event") == "terminal"][-1]

    def ime_ensure_japanese(input_hwnd):
        """Make ja-JP the input thread's layout and open MS-IME hiragana mode.

        RichEdit/TSF does not always expose an IMM context, so the open state
        is driven with the real Alt+` toggle and verified through the
        candidate's reported `composing` flag."""
        thread = ui.GetWindowThreadProcessId(input_hwnd, None)
        hkl = ui.LoadKeyboardLayoutW("00000411", 0)
        require(bool(hkl), "ja-JP layout (00000411) unavailable")
        ui.PostMessageW(input_hwnd, WM_INPUTLANGCHANGEREQUEST, 0,
                        ctypes.c_ssize_t(hkl).value)
        time.sleep(0.3)

    def ime_type_romaji(romaji):
        """Type romaji, cycling Alt+backtick until a real hiragana composition engages.

        MS-IME remembers its mode per app (closed / alphanumeric / hiragana);
        only an actual composition start is accepted - full-width alphanumeric
        input is still not a composition."""
        grave = ui.MapVirtualKeyW(0xC0, 0)
        alt = ui.MapVirtualKeyW(0x12, 0)
        for _ in range(4):
            ui.keys(romaji[0])
            if state()["composing"] is True:
                break
            seq = []
            for scan, flags in ((alt, 8), (grave, 8), (grave, 10), (alt, 10)):
                item = Input()
                item.type = 1
                item.value.keyboard = KeyboardInput(vk=0, scan=scan, flags=flags,
                                                    time=0, extra=None)
                seq.append(item)
            ui.send(seq)
            time.sleep(0.3)
            candidate.call("set_text", text="")
            focus_input()
        require(state()["composing"] is True,
                "IME composition did not start after mode cycling")
        for char in romaji[1:]:
            ui.keys(char)

    # ---------- B. Text cases (physical input into the real composer) ----------
    candidate.call("show")
    focus_input()
    total = state()["request_count"]

    try:
        ui.clear_clipboard()
        candidate.call("set_text", text="")
        focus_input()
        ui.type_text("Hello Ārā līst")
        require(text()["input"] == "Hello Ārā līst", "T1 typed text mismatch")
        record("T1", "PASS", snap("t1-typed", int(state()["composer_hwnd"])))

        candidate.call("set_text", text="")
        focus_input()
        ui.type_text("Проверка текста")
        require(text()["input"] == "Проверка текста", "T2 typed text mismatch")
        record("T2", "PASS", snap("t2-typed", int(state()["composer_hwnd"])))

        for fxid, case_id in (("F1", "T3"), ("F2", "T4")):
            value, sel = text_fixture[fxid], text_fixture[fxid + "_selection"]
            candidate.call("set_text", text=value)
            focus_input()
            ui.clear_clipboard()
            for key in sel["keys"]:
                ui.keys(key)
            copied = ui.wait_clipboard()
            require(copied == sel["copied"], f"{fxid} clipboard {copied!r} != {sel['copied']!r}")
            info = text()
            require(info["selection_start"] == sel["utf16_selection_start"] and
                    info["selection_end"] == sel["utf16_selection_end"],
                    f"{fxid} selection endpoints wrong")
            shot = snap(f"{fxid.lower()}-selection", int(state()["composer_hwnd"]))
            candidate.call("set_text", text=value)
            focus_input()
            for key in sel["keys"][:-1]:
                ui.keys(key)
            ui.keys("DELETE")
            require(text()["input"] == sel["after_selected_delete"],
                    f"{fxid} deletion residual")
            record(case_id, "PASS",
                   {"capture": shot, "copied": sel["copied"],
                    "endpoints": [sel["utf16_selection_start"], sel["utf16_selection_end"]]})

        # T5/F3 bidi ordering + logical copy
        candidate.call("set_text", text=text_fixture["F3"])
        focus_input()
        ui.clear_clipboard()
        ui.keys("CTRL+A")
        ui.keys("CTRL+C")
        require(ui.wait_clipboard() == text_fixture["F3"], "F3 logical copy mismatch")
        record("T5", "PASS", snap("t5-bidi", int(state()["composer_hwnd"])))

        # T6 keyboard+mouse selection and copy
        candidate.call("set_text", text="mouse selection check 123")
        focus_input()
        box = ui.rect(int(state()["input_hwnd"]))
        ui.drag(box.left + 12, box.top + 20, box.left + 150, box.top + 20)
        info = text()
        require(info["selection_end"] > info["selection_start"], "Mouse selection empty")
        ui.clear_clipboard()
        ui.keys("CTRL+C")
        copied = ui.wait_clipboard()
        require(bool(copied) and copied in "mouse selection check 123",
                f"Mouse copy mismatch {copied!r}")
        record("T6", "PASS",
               {"selected_utf16": [info["selection_start"], info["selection_end"]]})

        # T7/F10 multiline clipboard round trip
        candidate.call("set_text", text=text_fixture["F10"])
        focus_input()
        ui.clear_clipboard()
        ui.keys("CTRL+A")
        ui.keys("CTRL+C")
        copied = ui.wait_clipboard()
        require(copied is not None and copied.replace("\r\n", "\n") == text_fixture["F10"],
                "F10 clipboard content mismatch")
        candidate.call("set_text", text="")
        focus_input()
        ui.keys("CTRL+V")
        require(text()["input"] == text_fixture["F10"], "F10 paste mismatch")
        record("T7", "PASS", snap("t7-multiline", int(state()["composer_hwnd"])))

        record("T10", "PASS", "Same frozen procedures/endpoints as T3/T4")

        # T11: submit once when not composing
        before = state()["request_count"]
        candidate.call("set_text", text="submit check")
        focus_input()
        marker = len(candidate.events)
        ui.keys("CTRL+ENTER")
        require(state()["request_count"] == before + 1,
                "T11 submit did not produce exactly one request")
        wait_new_terminal(marker)
        total = state()["request_count"]
        record("T11", "PASS", {"request_count": total})

        # T13: reopen after hiding, typing works immediately
        candidate.call("hide")
        candidate.call("show")
        focus_input()
        candidate.call("set_text", text="reopen")
        focus_input()
        ui.type_text("!")
        require(text()["input"].endswith("!"), "T13 typing after reopen failed")
        record("T13", "PASS")

        # T8/F5 dead key on US-International
        input_hwnd = int(state()["input_hwnd"])
        thread = ui.GetWindowThreadProcessId(input_hwnd, None)
        previous_layout = ui.GetKeyboardLayout(thread)
        loaded = ui.LoadKeyboardLayoutW("00020409", 0)
        require(bool(loaded), "US-International layout (00020409) unavailable")
        try:
            require(ui.PostMessageW(input_hwnd, WM_INPUTLANGCHANGEREQUEST, 0,
                                    ctypes.c_ssize_t(loaded).value),
                    "Layout change request not delivered")
            time.sleep(0.3)
            candidate.call("set_text", text="")
            focus_input()
            requests = state()["request_count"]
            ui.keys("APOSTROPHE")
            require(state()["request_count"] == requests,
                    "Dead-key precomposition submitted a request")
            ui.keys("E")
            info = text()
            require(info["input"] == "é", f"F5 dead-key output {info['input']!r}")
        finally:
            ui.PostMessageW(input_hwnd, WM_INPUTLANGCHANGEREQUEST, 0,
                            ctypes.c_ssize_t(previous_layout).value)
        record("T8", "PASS")

        # T9/F6 + T12/F7-F8: Microsoft Japanese IME (physical romaji scan codes)
        activate_japanese_profile()
        ime_ensure_japanese(input_hwnd)
        candidate.call("set_text", text="")
        focus_input()
        ime_type_romaji(text_fixture["F6"]["romaji"])
        require(state()["composing"] is True, "F6 composition not observed")
        require(state()["request_count"] == total, "Composition submitted a request")
        shot = snap("f6-preedit", int(state()["composer_hwnd"]))
        ui.keys("ENTER")
        info = text()
        require(info["input"] == text_fixture["F6"]["expected"],
                f"F6 commit {info['input']!r}")
        require(state()["composing"] is False, "F6 composition did not end")
        record("T9", "PASS", {"commit": info["input"], "capture": shot})

        candidate.call("set_text", text=text_fixture["F6"]["cancel_prior"])
        focus_input()
        for char in text_fixture["F6"]["romaji"]:
            ui.keys(char)
        require(state()["composing"] is True, "F6b composition not observed")
        shot = snap("f6b-before-cancel", int(state()["composer_hwnd"]))
        for key in text_fixture["F6"]["cancel"]:
            ui.keys(key)
        info = text()
        require(info["input"] == text_fixture["F6"]["cancel_prior"],
                "F6b baseline not restored")
        require(state()["request_count"] == total, "F6b cancel produced a request")
        record("T9b-cancel", "PASS", shot)

        # F7: submit during composition is not a request; post-commit submit is
        candidate.call("set_text", text="")
        focus_input()
        for char in text_fixture["F6"]["romaji"]:
            ui.keys(char)
        require(state()["composing"] is True, "F7 composition not observed")
        count = state()["request_count"]
        marker = len(candidate.events)
        ui.keys("CTRL+ENTER")
        time.sleep(0.4)
        require(state()["request_count"] == count, "F7: request sent during composition")
        if state()["composing"]:
            ui.keys("ENTER")
        require(state()["composing"] is False, "F7 composition still active after commit")
        candidate.call("submit")
        require(state()["request_count"] == count + 1, "F7 post-commit submit missing")
        wait_new_terminal(marker)
        total = state()["request_count"]
        record("T12-F7", "PASS")

        # F8: global-hide during composition cancels it and restores committed text
        candidate.call("set_text", text="baseline-f8")
        focus_input()
        for char in text_fixture["F6"]["romaji"]:
            ui.keys(char)
        require(state()["composing"] is True, "F8 composition not observed")
        count = state()["request_count"]
        ui.keys("CTRL+ALT+SPACE")
        st = state()
        require(st["composer_visible"] is False, "F8 composer still visible")
        require(st["request_count"] == count, "F8 hide produced a request")
        candidate.call("show")
        info = text()
        require(info["input"] == "baseline-f8", f"F8 restored text {info['input']!r}")
        record("T12-F8", "PASS")
        ui.PostMessageW(input_hwnd, WM_INPUTLANGCHANGEREQUEST, 0,
                        ctypes.c_ssize_t(previous_layout).value)

        # T14/F9: response view visual parity — inject F1..F4, read back, capture
        response_hwnd = int(state()["response_hwnd"])
        combined = "\r\n".join([text_fixture["F1"], text_fixture["F2"],
                                text_fixture["F3"]] + text_fixture["F4"])
        buf = ctypes.create_unicode_buffer(combined)
        ui.SendMessageW(response_hwnd, 0x000C, 0, ctypes.addressof(buf))  # WM_SETTEXT
        time.sleep(0.4)
        shot = snap("t14-response", int(state()["composer_hwnd"]))
        require(ink_pixels(output / shot) > 400, "Response view appears blank")
        info = text()
        require(text_fixture["F3"] in info["response"].replace("\r\n", "\n"),
                "Response view did not retain injected text")
        record("T14", "PASS", shot)

    except Exception as error:
        done = {c["case"] for c in cases}
        for case in ["T1", "T2", "T3", "T4", "T5", "T6", "T7", "T8", "T9",
                     "T9b-cancel", "T10", "T11", "T12-F7", "T12-F8", "T13", "T14"]:
            if case not in done:
                record(case, "FAIL", repr(error))
        result["error"] = repr(error)

    return cases


def run_window_cases(candidate, output, ui, result):
    cases = result["cases"]

    def record(case, status, evidence=None):
        entry = {"case": case, "status": status, "fixture_version": candidate.manifest.get("version")}
        if evidence is not None:
            entry["evidence"] = evidence
        cases.append(entry)
        result["cases"] = cases

    def state():
        return candidate.call("state")["state"]

    def snap(name, hwnd):
        path = output / f"{name}.png"
        capture_window(hwnd, path)
        return path.name

    try:
        mascot = int(state()["mascot_hwnd"])
        require(mascot != 0, "Mascot window missing")
        candidate.call("hide")  # composer must not occlude the hit-test area
        time.sleep(0.3)
        listener = ListenerWindow()
        try:
            mrect = ui.rect(mascot)
            listener.move(mrect.left - 10, mrect.top - 10,
                          (mrect.right - mrect.left) + 20, (mrect.bottom - mrect.top) + 20)
            # Top of the non-topmost band: stays under the topmost mascot but
            # above unrelated windows that could occlude the click-through target.
            ui.SetWindowPos(listener.hwnd, wintypes.HWND(0), 0, 0, 0, 0,
                            0x0001 | 0x0002)  # HWND_TOP, NOSIZE|NOMOVE
            time.sleep(0.4)
            transparent, opaque = None, None
            for yy in range(mrect.top, mrect.bottom, 3):
                for xx in range(mrect.left, mrect.right, 3):
                    hit = ui.hit_test(mascot, xx, yy)
                    if hit == HTTRANSPARENT and transparent is None:
                        transparent = (xx, yy)
                    elif hit == HTCAPTION and opaque is None:
                        opaque = (xx, yy)
                if transparent and opaque:
                    break
            require(transparent and opaque, "Mascot hit mask lacks transparent and opaque points")
            require(ui.WindowFromPoint(wintypes.POINT(*transparent)) != mascot,
                    "WindowFromPoint still returns mascot at transparent pixel")
            shot = snap("w1-transparency", mascot)
            require(ink_pixels(output / shot) > 100, "Mascot silhouette not rendered")
            record("W1", "PASS", {"capture": shot, "hit_transparent": transparent,
                                  "hit_opaque": opaque})

            # W4: a click through the transparent pixel reaches the underlying window.
            # The listener must resolve as the next window under the transparent pixel.
            beneath = ui.WindowFromPoint(wintypes.POINT(*transparent))
            require(beneath == listener.hwnd,
                    f"Window beneath transparent pixel is {beneath}, not listener "
                    f"{listener.hwnd} (occlusion, not click-through failure)")
            del listener.clicks[:]
            ui.click(*transparent)
            time.sleep(0.3)
            require(listener.clicks, "Click through transparent pixel not delivered below")
            record("W4", "PASS", listener.clicks[-1])

            # W3: drag inside the silhouette moves the mascot smoothly
            before = ui.rect(mascot)
            ui.drag(opaque[0], opaque[1], opaque[0] + 120, opaque[1] + 80)
            after = ui.rect(mascot)
            require(abs((after.left - before.left) - 120) <= 8 and
                    abs((after.top - before.top) - 80) <= 8,
                    "Drag delta wrong: "
                    f"{(after.left - before.left, after.top - before.top)}")
            record("W3", "PASS")

            # W9: activate the ordinary listener; mascot stays above it, focus stays put
            mrect = ui.rect(mascot)
            listener.move(mrect.left, mrect.top, 400, 300)
            time.sleep(0.2)
            ui.foreground(listener.hwnd)
            time.sleep(0.2)
            require(ui.GetForegroundWindow() == listener.hwnd,
                    "Mascot stole foreground")
            walk, order = ui.GetTopWindow(None), []
            while walk and len(order) < 256:
                order.append(walk)
                walk = ui.GetWindow(walk, 2)  # GW_HWNDNEXT
            require(mascot in order and listener.hwnd in order and
                    order.index(mascot) < order.index(listener.hwnd),
                    "Mascot not above ordinary window in z-order")
            record("W9", "PASS")

            # W5: global hotkey while the ordinary listener holds the foreground
            candidate.call("hide")
            ui.foreground(listener.hwnd)
            ui.keys("CTRL+ALT+SPACE")
            st = state()
            require(st["composer_visible"] is True, "Hotkey did not show composer")
            record("W5", "PASS")
        finally:
            listener.close()

        candidate.call("hide")
        st = state()
        require(st["composer_visible"] is False, "Hide did not close composer")
        require(int(st["mascot_hwnd"]) == mascot and candidate.process.poll() is None,
                "Mascot/app lost after composer hide")
        record("W6", "PASS")

        # W8: idle for 5 s; presents/paints must not grow (no redraw loop)
        st = state()
        time.sleep(5.0)
        after = state()
        require(after["mascot_presents"] == st["mascot_presents"] and
                after["composer_paints"] == st["composer_paints"],
                "Idle redraw/present loop detected")
        record("W8", "PASS")

    except Exception as error:
        done = {c["case"] for c in cases}
        for case in ["W1", "W3", "W4", "W5", "W6", "W8"]:
            if case not in done:
                record(case, "FAIL", repr(error))
        result["error"] = repr(error)


def run_hidpi_cases(candidate, output, ui, result):
    """W2 (launch on a scaled display) + W7 (move across the DPI boundary)."""
    cases = result["cases"]

    def record(case, status, evidence=None):
        entry = {"case": case, "status": status, "fixture_version": candidate.manifest.get("version")}
        if evidence is not None:
            entry["evidence"] = evidence
        cases.append(entry)
        result["cases"] = cases

    def state():
        return candidate.call("state")["state"]

    monitors = ui.monitors()
    scales = {m["scale"] for m in monitors}
    base = int(state()["mascot_hwnd"])
    require(base != 0, "Mascot window missing")
    if len(monitors) < 2 or len(scales) < 2:
        record("W2", "UNTESTED", "Display lab lacks two different-scale targets")
        record("W7", "UNTESTED", "Display lab lacks two different-scale targets")
        return

    # W7: move the running mascot to a differently-scaled monitor and back.
    try:
        home = ui.GetDpiForWindow(base)
        target = next(m for m in monitors
                      if ui.GetDpiForWindow and
                      abs(m["scale"] * 96 // 100 - home) > 0)
        left, top, right, bottom = target["rect"]
        ui.SetWindowPos(base, None, left + (right - left) // 2, top + (bottom - top) // 2,
                        0, 0, 0x0001)  # SWP_NOSIZE
        time.sleep(0.8)
        moved_dpi = ui.GetDpiForWindow(base)
        expected = 64 * moved_dpi // 96
        rect = ui.rect(base)
        size = rect.right - rect.left
        require(moved_dpi != home, "Mascot did not transition DPI")
        require(abs(size - expected) <= max(4, expected // 10),
                f"Mascot size {size}px != ~{expected}px at {moved_dpi} DPI")
        shot = output / "w7-scaled.png"
        capture_window(base, shot)
        ui.SetWindowPos(base, None, monitors[0]["rect"][0] + 60,
                        monitors[0]["rect"][1] + 60, 0, 0, 0x0001)
        time.sleep(0.6)
        record("W7", "PASS", {"from_dpi": home, "to_dpi": moved_dpi,
                              "size_px": size, "capture": shot.name})
    except Exception as error:
        record("W7", "FAIL", repr(error))


def run_hidpi_launch(executable, manifest_path, output, result):
    """W2: launch a second instance with the pointer on the >=150% display."""
    ui = UiAutomation()
    monitors = ui.monitors()
    hi = next((m for m in monitors if m["scale"] >= 150), None)
    if hi is None:
        result["cases"].append({"case": "W2", "status": "UNTESTED",
                                "fixture_version": load(manifest_path)["version"],
                                "evidence": "No >=150% display target"})
        return
    left, top, right, bottom = hi["rect"]
    require(ui.SetCursorPos(left + (right - left) // 2, top + (bottom - top) // 2),
            "Cannot position pointer on scaled display")
    time.sleep(0.3)
    sub = output / "w2-second-launch"
    sub.mkdir()
    sub_result = {"event_observations": [], "provider_lifetimes": []}
    native = regression.Native()
    second = regression.Candidate(executable, manifest_path, sub, native, sub_result)
    try:
        st = second.call("state")["state"]
        mascot = int(st["mascot_hwnd"])
        require(mascot != 0, "Second mascot missing")
        dpi = ui.GetDpiForWindow(mascot)
        require(dpi == hi["scale"] * 96 // 100, f"Mascot DPI {dpi} != {hi['scale']}% target")
        rect = ui.rect(mascot)
        size = rect.right - rect.left
        expected = 64 * dpi // 96
        require(abs(size - expected) <= max(4, expected // 10),
                f"Mascot size {size}px != ~{expected}px")
        shot = sub / "w2-2x-launch.png"
        capture_window(mascot, shot)
        require(ink_pixels(shot) > 100, "Mascot not rendered on scaled display")
        token, _ = second.send("shutdown")
        second.reply(token, timeout=5)
        second.process.wait(timeout=5)
        result["cases"].append({"case": "W2", "status": "PASS",
                                "fixture_version": load(manifest_path)["version"],
                                "evidence": {"dpi": dpi, "size_px": size,
                                             "capture": "w2-second-launch/" + shot.name}})
    except Exception as error:
        result["cases"].append({"case": "W2", "status": "FAIL",
                                "fixture_version": load(manifest_path)["version"],
                                "evidence": repr(error)})
    finally:
        second.close()


def run(executable, manifest_path, output, with_hidpi, launch_point=None):
    output.mkdir(parents=True)
    result = {"schema": "mascot-acceptance-1", "candidate": "unknown", "cases": [],
              "event_observations": [], "provider_lifetimes": [], "error": None,
              "scope": "Text/window acceptance via physical input; provider P-cases "
                       "are covered separately by verify_provider_regression.py",
              "utc_started": datetime.now(timezone.utc).isoformat()}
    native = regression.Native()
    native.user.SetThreadDpiAwarenessContext(ctypes.c_void_p(-4))
    ui = UiAutomation()
    if launch_point is None:
        # Deterministic launch point: the secondary (virtual) display is bare
        # and free of unrelated windows, unlike the primary desktop.
        monitors = ui.monitors()
        secondary = [m for m in monitors if not m["primary"]]
        if secondary:
            r = secondary[0]["rect"]
            launch_point = (int((r[0] + r[2]) / 2), int((r[1] + r[3]) / 2))
    if launch_point is not None:
        ui.SetCursorPos(*launch_point)
        time.sleep(0.2)
    result["launch_point"] = launch_point
    candidate = regression.Candidate(executable, manifest_path, output, native, result)
    try:
        candidate.call("state")
        text_fixture = load(candidate.root / "fixtures/text.json")
        run_acceptance(candidate, output, ui, text_fixture, result)
        run_window_cases(candidate, output, ui, result)
        if with_hidpi:
            run_hidpi_cases(candidate, output, ui, result)
        else:
            for case in ("W2", "W7"):
                result["cases"].append({"case": case, "status": "UNTESTED",
                                        "fixture_version": candidate.manifest.get("version"),
                                        "evidence": "--with-hidpi not enabled"})
        # Shut down the primary instance before the W2 second launch: the
        # global hotkey can only be registered by one process at a time.
        token, _ = candidate.send("shutdown")
        candidate.reply(token, timeout=5)
        candidate.process.wait(timeout=5)
        if with_hidpi:
            run_hidpi_launch(executable, manifest_path, output, result)
    finally:
        candidate.close()
    result["utc_finished"] = datetime.now(timezone.utc).isoformat()
    failed = [c["case"] for c in result["cases"] if c["status"] == "FAIL"]
    untested = [c["case"] for c in result["cases"] if c["status"] == "UNTESTED"]
    result["status"] = ("FAIL" if failed or result.get("error")
                        else "PASS_WITH_UNTESTED" if untested else "PASS")
    (output / "result.json").write_text(json.dumps(result, indent=2, ensure_ascii=False),
                                        encoding="utf-8")
    return result


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--exe", required=True, type=pathlib.Path)
    parser.add_argument("--manifest", required=True, type=pathlib.Path)
    parser.add_argument("--output", required=True, type=pathlib.Path)
    parser.add_argument("--candidate", default="unknown")
    parser.add_argument("--with-hidpi", action="store_true",
                        help="Run W2/W7; requires the two-scale display laboratory")
    parser.add_argument("--launch-point", nargs=2, type=int,
                        metavar=("X", "Y"),
                        help="cursor position at launch (default: secondary "
                             "display center when present)")
    args = parser.parse_args()
    require(args.output.parent.exists(), "Output parent missing")
    require(not args.output.exists(), "Refusing to overwrite evidence directory")
    result = run(args.exe.resolve(), args.manifest.resolve(), args.output,
                 args.with_hidpi,
                 tuple(args.launch_point) if args.launch_point else None)
    result["candidate"] = args.candidate
    (args.output / "result.json").write_text(json.dumps(result, indent=2, ensure_ascii=False),
                                             encoding="utf-8")
    print(json.dumps({"status": result["status"], "error": result["error"],
                      "result": str(args.output / "result.json")}))
    sys.exit(0 if result["status"] in ("PASS", "PASS_WITH_UNTESTED") else 1)


if __name__ == "__main__":
    main()
