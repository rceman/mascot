#!/usr/bin/env python3
"""Visual sanity-check capture per user request.

Captures the final correctness-ready candidates against the virtual
display's plain desktop background:
- mascot / composer-empty / composer-typed / response-streamed stills
- dpi-125 / dpi-150 stills (physical display, matching lab scale)
- a 20-45s ffmpeg screen recording of the scripted demo flow
No candidate changes; the captures are evidence, not retouched output.
"""
import ctypes
import json
import pathlib
import subprocess
import sys
import time
from ctypes import wintypes

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import verify_provider_regression as regression
from verify_acceptance import UiAutomation, png_encode, USER32, GDI32

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
RAW_OUT = ROOT / "benchmark" / "results" / "windows"
VIS = RAW_OUT / "visual"
MANIFEST = ROOT / "benchmark" / "manifest" / "fixture.json"
FFMPEG = "ffmpeg"
VIRTUAL = {"left": 3840, "top": 0, "w": 1024, "h": 768}   # phase A
VIRTUAL_4K = {"left": 3840, "top": 0, "w": 3840, "h": 2160}

TYPED = "Ārā līst, mēs turpinām. 世界 👨‍💻"


def sig(dll, name, args, rest):
    f = getattr(dll, name)
    f.argtypes = args
    f.restype = rest
    return f


def capture_region(x, y, width, height, path):
    """External screen-region crop (shows desktop behind transparent pixels)."""
    GetDC = sig(USER32, "GetDC", [wintypes.HWND], wintypes.HDC)
    ReleaseDC = sig(USER32, "ReleaseDC", [wintypes.HWND, wintypes.HDC],
                  ctypes.c_int)
    CreateCompatibleDC = sig(GDI32, "CreateCompatibleDC", [wintypes.HDC],
                             wintypes.HDC)
    CreateCompatibleBitmap = sig(GDI32, "CreateCompatibleBitmap",
                                 [wintypes.HDC, ctypes.c_int, ctypes.c_int],
                                 wintypes.HANDLE)
    SelectObject = sig(GDI32, "SelectObject",
                       [wintypes.HDC, wintypes.HANDLE], wintypes.HANDLE)
    BitBlt = sig(GDI32, "BitBlt",
                 [wintypes.HDC] + [ctypes.c_int] * 4 + [wintypes.HDC] +
                 [ctypes.c_int] * 2 + [wintypes.DWORD], wintypes.BOOL)
    GetDIBits = sig(GDI32, "GetDIBits",
                    [wintypes.HDC, wintypes.HANDLE, wintypes.UINT,
                     wintypes.UINT, ctypes.c_void_p, ctypes.c_void_p,
                     wintypes.UINT], ctypes.c_int)
    screen = GetDC(None)
    memory = CreateCompatibleDC(screen)
    bitmap = CreateCompatibleBitmap(screen, width, height)
    assert screen and memory and bitmap
    try:
        previous = SelectObject(memory, bitmap)
        assert BitBlt(memory, 0, 0, width, height, screen, x, y, 0x00CC0020)
        SelectObject(memory, previous)

        class BmpInfo(ctypes.Structure):
            _fields_ = [("size", wintypes.DWORD), ("width", wintypes.LONG),
                        ("height", wintypes.LONG), ("planes", wintypes.WORD),
                        ("bits", wintypes.WORD), ("compression", wintypes.DWORD),
                        ("size_image", wintypes.DWORD), ("xp", wintypes.LONG),
                        ("yp", wintypes.LONG), ("used", wintypes.DWORD),
                        ("important", wintypes.DWORD)]

        stride = (width * 3 + 3) & ~3
        info = BmpInfo(ctypes.sizeof(BmpInfo), width, -height, 1, 24,
                       0, 0, 0, 0, 0, 0)
        buf = (ctypes.c_ubyte * (stride * height))()
        assert GetDIBits(memory, bitmap, 0, height, buf, ctypes.byref(info),
                         0) == height
        rgb = bytearray()
        for row_y in range(height):
            row = buf[row_y * stride:(row_y + 1) * stride]
            for px in range(width):
                rgb += bytes((row[px * 3 + 2], row[px * 3 + 1], row[px * 3]))
        path.write_bytes(png_encode(bytes(rgb), width, height))
    finally:
        sig(GDI32, "DeleteObject", [wintypes.HANDLE], wintypes.BOOL)(bitmap)
        sig(GDI32, "DeleteDC", [wintypes.HDC], wintypes.BOOL)(memory)
        ReleaseDC(None, screen)


class DEVMODEW(ctypes.Structure):
    _fields_ = [("dmDeviceName", wintypes.WCHAR * 32),
                ("dmSpecVersion", wintypes.WORD), ("dmDriverVersion", wintypes.WORD),
                ("dmSize", wintypes.WORD), ("dmDriverExtra", wintypes.WORD),
                ("dmFields", wintypes.DWORD),
                ("dmPosition_x", wintypes.LONG), ("dmPosition_y", wintypes.LONG),
                ("dmDisplayOrientation", wintypes.DWORD),
                ("dmDisplayFixedOutput", wintypes.DWORD),
                ("dmColor", wintypes.WORD), ("dmDuplex", wintypes.WORD),
                ("dmYResolution", wintypes.WORD), ("dmTTOption", wintypes.WORD),
                ("dmCollate", wintypes.WORD),
                ("dmFormName", wintypes.WCHAR * 32),
                ("dmLogPixels", wintypes.WORD),
                ("dmBitsPerPel", wintypes.DWORD), ("dmPelsWidth", wintypes.DWORD),
                ("dmPelsHeight", wintypes.DWORD), ("dmDisplayFlags", wintypes.DWORD),
                ("dmDisplayFrequency", wintypes.DWORD),
                ("dmICMMethod", wintypes.DWORD), ("dmICMIntent", wintypes.DWORD),
                ("dmMediaType", wintypes.DWORD), ("dmDitherType", wintypes.DWORD),
                ("dmReserved1", wintypes.DWORD), ("dmReserved2", wintypes.DWORD),
                ("dmPanningWidth", wintypes.DWORD), ("dmPanningHeight", wintypes.DWORD)]


class DISPLAY_DEVICEW(ctypes.Structure):
    _fields_ = [("cb", wintypes.DWORD), ("DeviceName", wintypes.WCHAR * 32),
                ("DeviceString", wintypes.WCHAR * 128),
                ("StateFlags", wintypes.DWORD),
                ("DeviceID", wintypes.WCHAR * 128),
                ("DeviceKey", wintypes.WCHAR * 128)]


def set_virtual_size(w, h):
    """ChangeDisplaySettingsExW on the usbmmidd virtual display (3840,0 origin)."""
    idx, name = 0, None
    while True:
        dev = DISPLAY_DEVICEW()
        dev.cb = ctypes.sizeof(DISPLAY_DEVICEW)
        if not USER32.EnumDisplayDevicesW(None, idx, ctypes.byref(dev), 0):
            break
        idx += 1
        dm = DEVMODEW()
        dm.dmSize = ctypes.sizeof(DEVMODEW)
        if USER32.EnumDisplaySettingsW(dev.DeviceName, -1, ctypes.byref(dm)) and \
           dm.dmPosition_x == VIRTUAL["left"] and dm.dmPosition_y == VIRTUAL["top"]:
            name = dev.DeviceName
            break
    assert name, "virtual display not found"
    dm = DEVMODEW()
    dm.dmSize = ctypes.sizeof(DEVMODEW)
    assert USER32.EnumDisplaySettingsW(name, -1, ctypes.byref(dm))
    dm.dmPelsWidth = w
    dm.dmPelsHeight = h
    dm.dmFields = 0x00080000 | 0x00100000  # DM_PELSWIDTH | DM_PELSHEIGHT
    rc = USER32.ChangeDisplaySettingsExW(name, ctypes.byref(dm), None, 0, None)
    assert rc == 0, f"ChangeDisplaySettingsExW rc={rc}"
    return name


def crop_pad(ui, hwnd, pad, path, extra=None):
    box = ui.rect(hwnd)
    x = max(0, box.left - pad)
    y = max(0, box.top - pad)
    w = box.right - box.left + 2 * pad
    h = box.bottom - box.top + 2 * pad
    if extra:  # union with another window rect (composer alongside mascot)
        bx = ui.rect(extra)
        x2 = min(x, bx.left - pad)
        y2 = min(y, bx.top - pad)
        w = max(x + w, bx.right + pad) - x2
        h = max(y + h, bx.bottom + pad) - y2
        x, y = x2, y2
    capture_region(x, y, w, h, path)


def state(candidate):
    return candidate.call("state")["state"]


def dpi_capture(which, scale):
    """Launch at cursor on the virtual display and capture mascot+composer."""
    cand_dir = VIS / which
    work = cand_dir / "_work"
    (work / f"dpi{scale}").mkdir(parents=True, exist_ok=True)
    native = regression.Native()
    native.user.SetThreadDpiAwarenessContext(ctypes.c_void_p(-4))
    ui = UiAutomation()
    result = {"event_observations": []}
    exe = ROOT / "out" / which / "mascot.exe"
    ui.SetCursorPos(VIRTUAL["left"] + 900, VIRTUAL["top"] + 700)
    candidate = regression.Candidate(exe, MANIFEST, work / f"dpi{scale}",
                                     native, result)
    try:
        time.sleep(1.0)
        st = state(candidate)
        mascot = int(st["mascot_hwnd"])
        candidate.call("show")
        time.sleep(0.8)
        st = state(candidate)
        crop_pad(ui, mascot, 40, cand_dir / f"dpi-{scale}.png",
                 extra=int(st["composer_hwnd"]))
        candidate.send("shutdown")
        for _ in range(12):
            if candidate.pump(timeout=4, eof_ok=True) is False:
                break
    finally:
        try:
            candidate.process.terminate()
        except Exception:
            pass
        candidate.process.wait(timeout=10)
    print(f"{which}: dpi-{scale} captured", flush=True)


def main():
    which = sys.argv[1]
    if which == "dpi":
        # args: dpi <cand> <scale>
        dpi_capture(sys.argv[2], sys.argv[3])
        return
    cand_dir = VIS / which
    cand_dir.mkdir(parents=True, exist_ok=True)
    work = cand_dir / "_work"
    work.mkdir(exist_ok=True)

    native = regression.Native()
    native.user.SetThreadDpiAwarenessContext(ctypes.c_void_p(-4))
    ui = UiAutomation()
    result = {"event_observations": []}
    exe = ROOT / "out" / which / "mascot.exe"

    # ---- Phase A: stills + video at 100% on the virtual display ----
    ui.SetCursorPos(VIRTUAL["left"] + 512, VIRTUAL["top"] + 300)
    candidate = regression.Candidate(exe, MANIFEST, work, native, result)
    try:
        time.sleep(0.8)
        st = state(candidate)  # first call doubles as readiness check
        mascot = int(st["mascot_hwnd"])
        crop_pad(ui, mascot, 48, cand_dir / "mascot.png")

        candidate.send("show")
        time.sleep(0.6)
        st = state(candidate)
        composer = int(st["composer_hwnd"])
        crop_pad(ui, composer, 24, cand_dir / "composer-empty.png")

        candidate.send("set_text", text=TYPED)
        time.sleep(0.5)
        crop_pad(ui, composer, 24, cand_dir / "composer-typed.png")

        # streamed response: submit, capture mid-stream, wait for terminal
        base = len(candidate.events)
        candidate.send("set_text", text="visual check")
        time.sleep(0.3)
        candidate.send("submit")
        deadline = time.time() + 12
        captured = False
        while time.time() < deadline:
            candidate.pump(timeout=5)
            new = candidate.events[base:]
            chunks = [e for e in new if e.get("event") == "chunk_accepted"]
            term = [e for e in new if e.get("event") == "terminal"]
            if not captured and len(chunks) >= 6 and not term:
                crop_pad(ui, composer, 24, cand_dir / "response-streamed.png")
                captured = True
            if term:
                break
        if not captured:
            crop_pad(ui, composer, 24, cand_dir / "response-streamed.png")
        candidate.send("hide")
        time.sleep(0.4)
        candidate.send("shutdown")
        for _ in range(20):
            if candidate.pump(timeout=5, eof_ok=True) is False:
                break
    finally:
        try:
            candidate.process.terminate()
        except Exception:
            pass
        candidate.process.wait(timeout=10)
    print(f"{which}: stills captured", flush=True)

    # ---- video: record virtual display while scripted input plays ----
    mp4 = cand_dir / f"{which}-demo.mp4"
    ff = subprocess.Popen(
        [FFMPEG, "-y", "-f", "gdigrab", "-framerate", "15",
         "-offset_x", str(VIRTUAL["left"]), "-offset_y", str(VIRTUAL["top"]),
         "-video_size", f"{VIRTUAL['w']}x{VIRTUAL['h']}",
         "-i", "desktop", "-c:v", "libx264", "-preset", "fast",
         "-crf", "22", "-pix_fmt", "yuv420p", str(mp4)],
        stdin=subprocess.PIPE, stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL)
    try:
        time.sleep(1.2)  # let gdigrab settle
        ui.SetCursorPos(VIRTUAL["left"] + 512, VIRTUAL["top"] + 300)
        result2 = {"event_observations": []}
        (work / "vid").mkdir(exist_ok=True)
        cand2 = regression.Candidate(exe, MANIFEST, work / "vid", native,
                                     result2)
        time.sleep(2.5)  # mascot appears and settles on video
        st = state(cand2)
        mascot = int(st["mascot_hwnd"])
        box = ui.rect(mascot)
        cx, cy = (box.left + box.right) // 2, (box.top + box.bottom) // 2
        # drag mascot
        ui.drag(cx, cy, cx + 140, cy + 70, steps=30)
        time.sleep(1.5)
        # click-through demo: right-click a transparent corner pixel;
        # the desktop (not the mascot) receives it -> desktop context menu
        ui.SetCursorPos(box.left + 4, box.top + 4)
        time.sleep(0.15)
        from verify_acceptance import Input, MouseInput
        inputs = []
        for flag in (0x0008, 0x0010):             # RIGHTDOWN, RIGHTUP
            it = Input()
            it.type = 0
            it.value.mouse = MouseInput(dx=0, dy=0, data=0, flags=flag,
                                        time=0, extra=None)
            inputs.append(it)
        ui.send(inputs)
        time.sleep(1.4)
        ui.keys("ESCAPE", settle_ms=700)          # dismiss context menu
        time.sleep(0.6)
        ui.keys("CTRL+ALT+SPACE", settle_ms=900)  # global hotkey -> composer
        time.sleep(1.2)
        st = state(cand2)
        ui.type_text(TYPED, settle_ms=90)
        time.sleep(1.2)
        ui.keys("CTRL+HOME", settle_ms=500)       # caret to start
        ui.keys("SHIFT+END", settle_ms=1600)      # selection visible
        ui.keys("CTRL+ENTER", settle_ms=900)      # submit
        time.sleep(6.0)                            # streamed response visible
        ui.keys("CTRL+ALT+SPACE", settle_ms=900)  # hide composer
        time.sleep(1.5)
        cand2.send("shutdown")
        for _ in range(12):
            if cand2.pump(timeout=4, eof_ok=True) is False:
                break
    finally:
        try:
            ff.stdin.write(b"q\n")
            ff.stdin.flush()
        except Exception:
            pass
        try:
            ff.wait(timeout=15)
        except Exception:
            ff.kill()
    print(f"{which}: video captured", flush=True)


if __name__ == "__main__":
    main()
