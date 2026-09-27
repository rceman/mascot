import argparse
import base64
import ctypes
import hashlib
import json
import os
import pathlib
import queue
import subprocess
import sys
import threading
import time
from ctypes import wintypes
from datetime import datetime, timezone


def load(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def vector_peak(vector, limit):
    buffered = peak = 0
    for encoded in vector["fragments_base64"]:
        for byte in base64.b64decode(encoded, validate=True):
            if buffered == limit:
                return peak
            buffered += 1
            peak = max(peak, buffered)
            if byte == 10:
                buffered = 0
    return peak


class LogFont(ctypes.Structure):
    _fields_ = [(name, wintypes.LONG) for name in
                ("height", "width", "escapement", "orientation", "weight")] + [
        (name, wintypes.BYTE) for name in
        ("italic", "underline", "strikeout", "charset", "out_precision",
         "clip_precision", "quality", "pitch_family")] + [("face", wintypes.WCHAR * 32)]


def observe_font(user, hwnd):
    user.SendMessageTimeoutW.argtypes = [wintypes.HWND, wintypes.UINT, ctypes.c_size_t,
                                       ctypes.c_ssize_t, wintypes.UINT, wintypes.UINT,
                                       ctypes.POINTER(ctypes.c_size_t)]
    user.SendMessageTimeoutW.restype = ctypes.c_ssize_t
    font_handle = ctypes.c_size_t()
    require(user.SendMessageTimeoutW(hwnd, 0x0031, 0, 0, 2, 2000, ctypes.byref(font_handle)),
            "WM_GETFONT did not complete")
    gdi = ctypes.WinDLL("gdi32", use_last_error=True)
    gdi.GetObjectW.argtypes = [wintypes.HANDLE, ctypes.c_int, ctypes.c_void_p]
    gdi.GetObjectW.restype = ctypes.c_int
    font = LogFont()
    if not font_handle.value or gdi.GetObjectW(font_handle.value, ctypes.sizeof(font), ctypes.byref(font)) != ctypes.sizeof(font):
        return {"available": False, "reason": "Control font object not queryable from observer; visual font verification still required"}
    return {"available": True, "height": font.height, "face": font.face}


def smoke(executable, manifest_path, output, result, expected_dpi):
    manifest = load(manifest_path)
    root = manifest_path.parent.parent
    vectors_path = root / manifest["decoder_vectors"]
    vectors = load(vectors_path)
    result.update(fixture_version=manifest["version"], executable=str(executable),
                  executable_sha256=hashlib.sha256(executable.read_bytes()).hexdigest())
    run = subprocess.run([str(executable), "--decode-vectors", str(vectors_path)],
                         stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30, check=False,
                         creationflags=subprocess.CREATE_NO_WINDOW)
    (output / "decoder.stdout.ndjson").write_bytes(run.stdout)
    (output / "decoder.stderr.log").write_bytes(run.stderr)
    require(run.returncode == 0, f"Decoder exit {run.returncode}")
    rows = [json.loads(line) for line in run.stdout.decode("utf-8").splitlines() if line.strip()]
    require(len(rows) == len(vectors), "Wrong decoder result count")
    limit = manifest["protocol"]["max_stdout_frame_bytes_including_lf"]
    for row, vector in zip(rows, vectors):
        require(row["name"] == vector["name"], "Wrong decoder vector order/name")
        require(row["frames_base64"] == vector["expected_frames_base64"], vector["name"] + " frames differ")
        require(row["rejected"] is vector["reject"], vector["name"] + " rejection differs")
        require(row["peak_buffer_bytes"] == vector_peak(vector, limit), vector["name"] + " peak differs")
        result["checks"].append({"check": "decoder:" + vector["name"], "status": "PASS"})
    user = ctypes.WinDLL("user32", use_last_error=True)
    user.IsWindow.argtypes = [wintypes.HWND]
    user.IsWindow.restype = wintypes.BOOL
    user.IsWindowVisible.argtypes = [wintypes.HWND]
    user.IsWindowVisible.restype = wintypes.BOOL
    user.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
    user.GetWindowThreadProcessId.restype = wintypes.DWORD
    user.GetDpiForWindow.argtypes = [wintypes.HWND]
    user.GetDpiForWindow.restype = wintypes.UINT
    user.GetClientRect.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.RECT)]
    user.GetClientRect.restype = wintypes.BOOL
    user.GetWindowRect.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.RECT)]
    user.GetWindowRect.restype = wintypes.BOOL
    user.ClientToScreen.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.POINT)]
    user.ClientToScreen.restype = wintypes.BOOL
    user.GetClassNameW.argtypes = [wintypes.HWND, wintypes.LPWSTR, ctypes.c_int]
    user.GetClassNameW.restype = ctypes.c_int
    control = load(root / manifest["candidate_control"])
    environment = os.environ.copy()
    for key in list(environment):
        if key.upper() in {"GOMAXPROCS", "GOGC", "GOMEMLIMIT", "GODEBUG", "GOTRACEBACK"}:
            del environment[key]
    environment.update(GOGC="100", GOMEMLIMIT="off", GODEBUG="", GOTRACEBACK="single")
    with (output / "application.stderr.log").open("xb") as stderr:
        process = subprocess.Popen([str(executable), "--fixture", str(manifest_path), "--control"],
                                   cwd=executable.parent, env=environment, stdin=subprocess.PIPE,
                                   stdout=subprocess.PIPE, stderr=stderr,
                                   creationflags=subprocess.CREATE_NO_WINDOW)
        messages = queue.Queue(maxsize=256)

        def read_output():
            try:
                with (output / "application.stdout.ndjson").open("xb") as raw:
                    while True:
                        line = process.stdout.readline(control["max_reply_bytes_including_lf"] + 1)
                        if not line:
                            break
                        raw.write(line)
                        raw.flush()
                        require(len(line) <= control["max_reply_bytes_including_lf"], "Oversized control reply")
                        messages.put_nowait(json.loads(line.decode("utf-8")))
            except Exception as error:
                messages.put_nowait({"reader_error": repr(error)})
            finally:
                messages.put_nowait({"reader_eof": True})

        reader = threading.Thread(target=read_output, daemon=True)
        reader.start()
        token = 0

        def call(command, **fields):
            nonlocal token
            token += 1
            process.stdin.write((json.dumps(dict(token=token, command=command, **fields), ensure_ascii=False) + "\n").encode("utf-8"))
            process.stdin.flush()
            deadline = time.monotonic() + 15
            while True:
                message = messages.get(timeout=max(0, deadline - time.monotonic()))
                require(not message.get("reader_error"), str(message))
                require(not message.get("reader_eof"), "Application stdout ended before reply")
                if "event" in message:
                    result["events"].append(message)
                    continue
                require(message.get("token") == token, "Unexpected reply token")
                require(message.get("ok") is True, str(message))
                return message

        def window(value):
            require(isinstance(value, str) and value.isdecimal(), "HWND is not a decimal string")
            handle = int(value)
            require(bool(user.IsWindow(handle)), "Reported HWND is not a real live window")
            pid = wintypes.DWORD()
            require(user.GetWindowThreadProcessId(handle, ctypes.byref(pid)) != 0 and pid.value == process.pid,
                    "Reported HWND is not owned by this application")
            return handle

        try:
            state = call("state")["state"]
            require(state["pid"] == process.pid, "Wrong application PID")
            mascot = window(state["mascot_hwnd"])
            mascot_dpi = user.GetDpiForWindow(mascot)
            require(mascot_dpi > 0, "Cannot query mascot DPI")
            if expected_dpi is not None:
                require(mascot_dpi == expected_dpi, "Mascot did not launch at the requested display scale")
            result["initial_state"] = state
            result["mascot_dpi"] = mascot_dpi
            require(state["composer_hwnd"] == "0" and state["provider_pid"] == 0, "Unexpected eager text/provider creation")
            require(state["mascot_presents"] >= 1, "No mascot presentation call reported")
            require(state["provider_state"] == "idle" and state["request_count"] == 0, "Unexpected initial provider state")
            result["checks"].append({"check": "native mascot handle and lazy initial state", "status": "PASS"})
            state = call("show")["state"]
            composer = window(state["composer_hwnd"])
            input_hwnd = window(state["input_hwnd"])
            response_hwnd = window(state["response_hwnd"])
            require(state["composer_visible"] and user.IsWindowVisible(composer), "Composer not marked visible by Windows")
            dpi = user.GetDpiForWindow(composer)
            rectangle = wintypes.RECT()
            require(dpi > 0 and user.GetClientRect(composer, ctypes.byref(rectangle)), "Cannot query composer geometry")
            result["shown_state"] = state
            result["composer_geometry"] = {"dpi": dpi, "client_width": rectangle.right - rectangle.left,
                                           "client_height": rectangle.bottom - rectangle.top}
            if expected_dpi is not None:
                require(dpi == expected_dpi, "Composer opened on a different display scale")
            scale = lambda value: (value * dpi + 48) // 96
            require(rectangle.right - rectangle.left == scale(manifest["ui"]["composer_client_width_dip"]), "Composer client width differs")
            require(rectangle.bottom - rectangle.top == scale(manifest["ui"]["composer_client_height_dip"]), "Composer client height differs")
            fixture = load(root / manifest["text_fixture"])
            origin = wintypes.POINT(0, 0)
            require(user.ClientToScreen(composer, ctypes.byref(origin)), "Cannot query client origin")
            margin = manifest["ui"]["margin_dip"]
            width = manifest["ui"]["composer_client_width_dip"] - 2 * margin
            result["native_controls"] = {}
            for name, hwnd, top, height in (
                ("input", input_hwnd, margin, manifest["ui"]["input_height_dip"]),
                ("response", response_hwnd, 2 * margin + manifest["ui"]["input_height_dip"], manifest["ui"]["response_height_dip"])):
                class_name = ctypes.create_unicode_buffer(256)
                require(user.GetClassNameW(hwnd, class_name, len(class_name)), "Cannot query control class")
                require(user.GetWindowRect(hwnd, ctypes.byref(rectangle)), "Cannot query child geometry")
                geometry = [rectangle.left - origin.x, rectangle.top - origin.y,
                            rectangle.right - rectangle.left, rectangle.bottom - rectangle.top]
                font = observe_font(user, hwnd)
                result["native_controls"][name] = {"class": class_name.value, "geometry": geometry, "font": font}
                require(class_name.value.upper() == "RICHEDIT50W", "Unexpected native text control")
                require(geometry == [scale(margin), scale(top), scale(width), scale(height)], name + " geometry differs")
                if font["available"]:
                    require(font["height"] == -scale(fixture["font"]["size_dip"]), name + " font height differs")
                    require(font["face"].casefold() == fixture["font"]["family"].casefold(), name + " primary font differs")
            call("set_text", text=fixture["F10"])
            text = call("text")["text"]
            require(text["input"] == fixture["F10"], "Native input did not preserve F10 logical text")
            require(text["response"] == "" and not text["composing"], "Unexpected initial response/composition")
            result["checks"].append({"check": "native control handles, fixed geometry and programmatic text round-trip", "status": "PASS"})
            state = call("hide")["state"]
            require(not state["composer_visible"] and not user.IsWindowVisible(composer), "Hide did not hide the composer")
            require(user.IsWindowVisible(mascot) and state["provider_pid"] == 0 and state["request_count"] == 0,
                    "Hide affected the mascot or unexpectedly submitted a request")
            call("shutdown")
            require(process.wait(timeout=5) == 0, "Application did not exit zero")
            result["checks"].append({"check": "hide and no-provider shutdown", "status": "PASS"})
        finally:
            if process.poll() is None:
                process.stdin.close()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.terminate()
                    process.wait(timeout=5)
            reader.join(timeout=5)
            require(not reader.is_alive(), "Control reader did not reach EOF")
            process.stdout.close()
            if not process.stdin.closed:
                process.stdin.close()
        result["status"] = "PASS_SMOKE_ONLY"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--exe", type=pathlib.Path, required=True)
    parser.add_argument("--manifest", type=pathlib.Path, required=True)
    parser.add_argument("--output", type=pathlib.Path, required=True)
    parser.add_argument("--launch-point", type=int, nargs=2, metavar=("X", "Y"))
    parser.add_argument("--expected-dpi", type=int, choices=(96, 192))
    args = parser.parse_args()
    if os.name != "nt":
        parser.error("Native Windows is required.")
    args.output.mkdir(exist_ok=False)
    result = {"schema": "mascot-candidate-smoke-2", "utc_started": datetime.now(timezone.utc).isoformat(),
              "scope": "Targeted implementation smoke, not full candidate correctness or visible timing qualification",
              "observer_sha256": hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest(),
              "expected_dpi": args.expected_dpi, "launch_point": args.launch_point,
              "status": "FAIL", "checks": [], "events": [], "error": None}
    user = ctypes.WinDLL("user32", use_last_error=True)
    user.SetThreadDpiAwarenessContext.argtypes = [ctypes.c_void_p]
    user.SetThreadDpiAwarenessContext.restype = ctypes.c_void_p
    user.GetThreadDpiAwarenessContext.argtypes = []
    user.GetThreadDpiAwarenessContext.restype = ctypes.c_void_p
    user.AreDpiAwarenessContextsEqual.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
    user.AreDpiAwarenessContextsEqual.restype = wintypes.BOOL
    user.GetCursorPos.argtypes = [ctypes.POINTER(wintypes.POINT)]
    user.GetCursorPos.restype = wintypes.BOOL
    user.SetCursorPos.argtypes = [ctypes.c_int, ctypes.c_int]
    user.SetCursorPos.restype = wintypes.BOOL
    user.MonitorFromPoint.argtypes = [wintypes.POINT, wintypes.DWORD]
    user.MonitorFromPoint.restype = wintypes.HANDLE
    previous_dpi = None
    previous_cursor = None
    try:
        previous_dpi = user.SetThreadDpiAwarenessContext(ctypes.c_void_p(-4))
        require(previous_dpi and user.AreDpiAwarenessContextsEqual(user.GetThreadDpiAwarenessContext(), ctypes.c_void_p(-4)),
                "Cannot establish per-monitor-v2 observer context")
        result["observer_dpi_context"] = "PER_MONITOR_AWARE_V2"
        if args.launch_point is not None:
            point = wintypes.POINT(*args.launch_point)
            require(user.MonitorFromPoint(point, 0), "Launch point is not on an active monitor")
            saved = wintypes.POINT()
            require(user.GetCursorPos(ctypes.byref(saved)), "Cannot save cursor position")
            previous_cursor = saved
            require(user.SetCursorPos(point.x, point.y), "Cannot position launch cursor")
        smoke(args.exe.resolve(), args.manifest.resolve(), args.output, result, args.expected_dpi)
    except Exception as error:
        result["error"] = repr(error)
    finally:
        if previous_cursor is not None and not user.SetCursorPos(previous_cursor.x, previous_cursor.y):
            result["status"] = "FAIL"
            result["cursor_restore_error"] = ctypes.get_last_error()
        if previous_dpi is not None and not user.SetThreadDpiAwarenessContext(previous_dpi):
            result["status"] = "FAIL"
            result["dpi_context_restore_error"] = ctypes.get_last_error()
    result["utc_finished"] = datetime.now(timezone.utc).isoformat()
    (args.output / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8", newline="\n")
    print(json.dumps({"status": result["status"], "error": result["error"], "result": str(args.output / "result.json")}))
    return 0 if result["status"] == "PASS_SMOKE_ONLY" else 1


if __name__ == "__main__":
    sys.exit(main())
