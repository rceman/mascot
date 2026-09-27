"""Native Windows benchmark runner per docs/BENCHMARK_PROTOCOL.md.

Measures, for one candidate per invocation: fresh-process warm-cache
startup (external QPC at launch request -> first externally observed mascot
frame via screen-region pixel diff), first/warm composer activation
(external SendInput hotkey -> visible composer AND injected-text input
readiness), R0-R6 resource states, repeated-operation stability batches,
and the owned-process inventory.

OS-level process accounting (psapi/PDH/GetGuiResources/toolhelp) is
authoritative for headline resource data; candidate control 'state' fields
(redraws, queue depth) are recorded as diagnostics.

Observer contract (qualified per §4.6): visibility = pixel change inside
the expected window screen region relative to a pre-trigger baseline,
polled at ~10 ms -> observer resolution 10 ms, recorded as uncertainty.
Input readiness = an injected KEYEVENTF_UNICODE probe character read back
exactly through the candidate 'text' control reply.
"""
import argparse
import ctypes
import json
import pathlib
import statistics
import struct
import sys
import time
from ctypes import wintypes
from datetime import datetime, timezone

sys.path.insert(0, str(pathlib.Path(__file__).parent))
import verify_provider_regression as regression  # noqa: E402
from verify_candidate_smoke import load, require  # noqa: E402
import verify_acceptance as acceptance  # noqa: E402

KERNEL32 = ctypes.WinDLL("kernel32", use_last_error=True)
USER32 = ctypes.WinDLL("user32", use_last_error=True)
PSAPI = ctypes.WinDLL("psapi", use_last_error=True)
PDH = ctypes.WinDLL("pdh", use_last_error=True)


def sig(dll, name, args, rest):
    fn = getattr(dll, name)
    fn.argtypes, fn.restype = args, rest
    return fn


def qpc():
    value = ctypes.c_int64()
    sig(KERNEL32, "QueryPerformanceCounter", [ctypes.POINTER(ctypes.c_int64)],
        wintypes.BOOL)(ctypes.byref(value))
    return value.value


QPC_FREQ = None


def qpc_ms(start, end):
    global QPC_FREQ
    if QPC_FREQ is None:
        freq = ctypes.c_int64()
        sig(KERNEL32, "QueryPerformanceFrequency", [ctypes.POINTER(ctypes.c_int64)],
            wintypes.BOOL)(ctypes.byref(freq))
        QPC_FREQ = freq.value
    return (end - start) * 1000.0 / QPC_FREQ


class MemoryInfo(ctypes.Structure):
    _fields_ = [("cb", wintypes.DWORD), ("page_faults", wintypes.DWORD),
                ("peak_working_set", ctypes.c_size_t), ("working_set", ctypes.c_size_t),
                ("quota_peak_paged", ctypes.c_size_t), ("quota_paged", ctypes.c_size_t),
                ("quota_peak_nonpaged", ctypes.c_size_t),
                ("quota_nonpaged", ctypes.c_size_t),
                ("pagefile", ctypes.c_size_t), ("peak_pagefile", ctypes.c_size_t),
                ("private_usage", ctypes.c_size_t)]


class ProcessSample:
    """OS-level accounting for one application process (provider excluded)."""

    def __init__(self, pid, native):
        self.pid = pid
        self.native = native
        self.open_process = sig(KERNEL32, "OpenProcess",
                                [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD],
                                wintypes.HANDLE)
        self.close = sig(KERNEL32, "CloseHandle", [wintypes.HANDLE], wintypes.BOOL)
        self.get_times = sig(KERNEL32, "GetProcessTimes",
                             [wintypes.HANDLE] + [ctypes.POINTER(wintypes.FILETIME)] * 4,
                             wintypes.BOOL)
        self.get_handles = sig(KERNEL32, "GetProcessHandleCount",
                               [wintypes.HANDLE, ctypes.POINTER(wintypes.DWORD)],
                               wintypes.BOOL)
        self.memory = sig(PSAPI, "GetProcessMemoryInfo",
                          [wintypes.HANDLE, ctypes.c_void_p, wintypes.DWORD], wintypes.BOOL)
        self.gui = sig(USER32, "GetGuiResources", [wintypes.HANDLE, wintypes.DWORD],
                       wintypes.DWORD)
        self.enum_windows = sig(USER32, "EnumThreadWindows",
                                [wintypes.DWORD, ctypes.c_void_p, ctypes.c_ssize_t],
                                wintypes.BOOL)
        # PDH: authoritative Working Set - Private
        self.pdh_query = ctypes.c_void_p()
        require(PDH.PdhOpenQueryW(None, 0, ctypes.byref(self.pdh_query)) == 0,
                "PdhOpenQueryW failed")
        self.pdh_counter = ctypes.c_void_p()
        path = f"\\Process({self._image_name(pid)})\\Working Set - Private"
        require(PDH.PdhAddEnglishCounterW(self.pdh_query, path, 0,
                                        ctypes.byref(self.pdh_counter)) == 0,
                f"PDH counter add failed for {path}")
        PDH.PdhCollectQueryData(self.pdh_query)

    def _image_name(self, pid):
        snapshot = self.native.kernel.CreateToolhelp32Snapshot(2, 0)
        entry = regression.ProcessEntry()
        entry.size = ctypes.sizeof(entry)
        name = None
        try:
            if self.native.kernel.Process32FirstW(snapshot, ctypes.byref(entry)):
                while True:
                    if entry.pid == pid:
                        name = entry.name
                        break
                    if not self.native.kernel.Process32NextW(snapshot, ctypes.byref(entry)):
                        break
        finally:
            self.native.kernel.CloseHandle(snapshot)
        require(name, f"Process {pid} not found")
        return name

    def handle(self, access=0x0410):  # QUERY_INFORMATION | VM_READ
        handle = self.open_process(access, False, self.pid)
        require(bool(handle), f"OpenProcess({self.pid}) failed")
        return handle

    def private_working_set(self):
        class Fmt(ctypes.Structure):
            _fields_ = [("status", wintypes.DWORD), ("type", wintypes.DWORD),
                        ("large", ctypes.c_int64)]
        require(PDH.PdhCollectQueryData(self.pdh_query) == 0, "PDH collect failed")
        value = Fmt()
        status = PDH.PdhGetFormattedCounterValue(self.pdh_counter, 0x00000100, None,
                                                 ctypes.byref(value))  # PDH_FMT_LARGE
        if status != 0:
            return None
        return value.large

    def sample(self):
        handle = self.handle()
        try:
            info = MemoryInfo()
            info.cb = ctypes.sizeof(info)
            require(self.memory(handle, ctypes.byref(info), info.cb), "Memory query failed")
            creation = exit_t = wintypes.FILETIME()
            kernel = user = wintypes.FILETIME()
            require(self.get_times(handle, ctypes.byref(creation), ctypes.byref(exit_t),
                                   ctypes.byref(kernel), ctypes.byref(user)),
                    "Process times query failed")
            handles = wintypes.DWORD()
            require(self.get_handles(handle, ctypes.byref(handles)),
                    "Handle count failed")
            user_objects = self.gui(handle, 1)
            gdi_objects = self.gui(handle, 0)
            user_peak = self.gui(handle, 3)
            gdi_peak = self.gui(handle, 2)
        finally:
            self.close(handle)
        threads = self.native_threads()
        windows = self.live_windows(threads)
        return {
            "qpc": str(qpc()), "pid": self.pid,
            "private_working_set_bytes": self.private_working_set(),
            "private_commit_bytes": info.private_usage,
            "working_set_bytes": info.working_set,
            "peak_working_set_bytes": info.peak_working_set,
            "peak_pagefile_bytes": info.peak_pagefile,
            "user_cpu_100ns": self._ft(user), "kernel_cpu_100ns": self._ft(kernel),
            "threads": len(threads), "kernel_handles": handles.value,
            "user_objects": user_objects, "gdi_objects": gdi_objects,
            "user_objects_peak": user_peak, "gdi_objects_peak": gdi_peak,
            "live_windows": windows,
            "live_children": len(self.native.children(self.pid)),
        }

    def _ft(self, ft):
        return (ft.dwHighDateTime << 32) | ft.dwLowDateTime

    def native_threads(self):
        snapshot = self.native.kernel.CreateToolhelp32Snapshot(4, 0)  # TH32CS_SNAPTHREAD
        require(snapshot and snapshot != ctypes.c_void_p(-1).value, "Thread snapshot failed")

        class ThreadEntry(ctypes.Structure):
            _fields_ = [("size", wintypes.DWORD), ("usage", wintypes.DWORD),
                        ("pid", wintypes.DWORD), ("owner", wintypes.DWORD),
                        ("base_pri", wintypes.LONG), ("delta", wintypes.LONG),
                        ("flags", wintypes.DWORD)]
        rows = []
        try:
            entry = ThreadEntry()
            entry.size = ctypes.sizeof(entry)
            fn = self.native.kernel.Thread32First
            fn.argtypes = [wintypes.HANDLE, ctypes.POINTER(ThreadEntry)]
            fn.restype = wintypes.BOOL
            nxt = self.native.kernel.Thread32Next
            nxt.argtypes = fn.argtypes
            nxt.restype = wintypes.BOOL
            if fn(snapshot, ctypes.byref(entry)):
                while True:
                    if entry.owner == self.pid:
                        rows.append(entry.pid)
                    if not nxt(snapshot, ctypes.byref(entry)):
                        break
        finally:
            self.native.kernel.CloseHandle(snapshot)
        return rows

    def live_windows(self, threads):
        count = [0]
        Proc = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, ctypes.c_ssize_t)

        def visit(hwnd, param):
            count[0] += 1
            return True

        callback = Proc(visit)
        for tid in threads:
            self.enum_windows(tid, ctypes.cast(callback, ctypes.c_void_p), 0)
        return count[0]

    def close(self):
        if self.pdh_query:
            PDH.PdhCloseQuery(self.pdh_query)
            self.pdh_query = None


class RegionObserver:
    """External visibility observer: pixel diff inside a screen region.

    Resolution = poll interval (~10 ms). A 'visible' transition is a
    sustained >=threshold change; documented as observer uncertainty.
    """

    RESOLUTION_MS = 10.0

    def __init__(self, ui):
        self.ui = ui

    def grab(self, left, top, width, height):
        GetDC = sig(USER32, "GetDC", [wintypes.HWND], wintypes.HDC)
        CreateDC_mem = sig(GDI32 := ctypes.WinDLL("gdi32", use_last_error=True),
                           "CreateCompatibleDC", [wintypes.HDC], wintypes.HDC)
        CreateBmp = sig(GDI32, "CreateCompatibleBitmap",
                        [wintypes.HDC, ctypes.c_int, ctypes.c_int], wintypes.HANDLE)
        Select = sig(GDI32, "SelectObject", [wintypes.HDC, wintypes.HANDLE],
                     wintypes.HANDLE)
        Blt = sig(GDI32, "BitBlt", [wintypes.HDC] + [ctypes.c_int] * 4 +
                  [wintypes.HDC] + [ctypes.c_int] * 2 + [wintypes.DWORD], wintypes.BOOL)
        bits = sig(GDI32, "GetDIBits", [wintypes.HDC, wintypes.HANDLE, wintypes.UINT,
                   wintypes.UINT, ctypes.c_void_p, ctypes.c_void_p, wintypes.UINT],
                   ctypes.c_int)
        screen = GetDC(None)
        memory = CreateDC_mem(screen)
        bmp = CreateBmp(screen, width, height)
        try:
            old = Select(memory, bmp)
            require(Blt(memory, 0, 0, width, height, screen, left, top, 0x00CC0020),
                    "observer BitBlt failed")
            Select(memory, old)
            info = ctypes.create_string_buffer(40)
            struct.pack_into("<IiiHHIIiiII", info, 0, 40, width, -height, 1, 24,
                             0, 0, 0, 0, 0, 0)
            stride = (width * 3 + 3) & ~3
            buf = (ctypes.c_ubyte * (stride * height))()
            require(bits(memory, bmp, 0, height, buf, info, 0) == height, "GetDIBits")
            return bytes(buf), stride, width, height
        finally:
            sig(GDI32, "DeleteObject", [wintypes.HANDLE], wintypes.BOOL)(bmp)
            sig(GDI32, "DeleteDC", [wintypes.HDC], wintypes.BOOL)(memory)
            sig(USER32, "ReleaseDC", [wintypes.HWND, wintypes.HDC],
                ctypes.c_int)(None, screen)

    def changed_pixels(self, before, after):
        (b_buf, stride, w, h), (a_buf, _, _, _) = before, after
        changed = 0
        for y in range(h):
            for x in range(0, w * 3, 3):
                i = y * stride + x
                if abs(b_buf[i] - a_buf[i]) > 24 or abs(b_buf[i + 1] - a_buf[i + 1]) > 24 \
                        or abs(b_buf[i + 2] - a_buf[i + 2]) > 24:
                    changed += 1
        return changed

    def wait_change(self, region, baseline, min_pixels=64, timeout_s=10.0):
        """Returns (visible_qpc, changed_pixel_count) or raises."""
        left, top, w, h = region
        deadline = time.monotonic() + timeout_s
        while True:
            sample = self.grab(left, top, w, h)
            stamp = qpc()
            changed = self.changed_pixels(baseline, sample)
            if changed >= min_pixels:
                return stamp, changed
            require(time.monotonic() < deadline, "Observer saw no visible change")


def launch_candidate(executable, manifest_path, output, native, result, launch_point):
    """Spawn with the pointer at launch_point (external startup clock origin)."""
    ui = acceptance.UiAutomation()
    require(ui.SetCursorPos(*launch_point), "Cannot position pointer for launch")
    time.sleep(0.1)
    # Observer baseline: region where the mascot will appear (near cursor).
    margin = 220
    region = (launch_point[0] - margin, launch_point[1] - margin,
              margin * 2, margin * 2)
    observer = RegionObserver(ui)
    baseline = observer.grab(*region)
    start = qpc()
    candidate = regression.Candidate(executable, manifest_path, output, native, result)
    stamp, changed = observer.wait_change(region, baseline)
    return candidate, {"launch_qpc": str(start), "visible_qpc": str(stamp),
                       "startup_ms": qpc_ms(start, stamp),
                       "observer_resolution_ms": RegionObserver.RESOLUTION_MS,
                       "changed_pixels": changed}


def sample_row(sampler, candidate_state=None):
    row = sampler.sample()
    if candidate_state is not None:
        row["redraws"] = candidate_state.get("composer_paints")
        row["presents"] = candidate_state.get("mascot_presents")
        row["bounded_queue_depth"] = candidate_state.get("queued_provider_frames")
    return row


def base_result(candidate_name):
    return {"schema": "mascot-benchmark-1", "candidate": candidate_name,
            "utc_started": datetime.now(timezone.utc).isoformat(),
            "observer": {"procedure": "screen-region pixel diff >=64 px, "
                         "polled ~10ms + injected-text input-readiness probe",
                         "resolution_ms": RegionObserver.RESOLUTION_MS,
                         "uncertainty": ">= one poll interval (~10 ms); "
                         "differences below this are not decisive"},
            "fresh_launches": [], "warm_activations": {},
            "resource_states": {}, "stability": {}, "process_inventory": [],
            "errors": []}


def fresh_launch_once(executable, manifest_path, output, native, launch_point,
                      index, result):
    run_dir = output / f"launch-{index:03d}"
    run_dir.mkdir()
    holder = {"event_observations": [], "provider_lifetimes": []}
    try:
        candidate, timing = launch_candidate(executable, manifest_path, run_dir,
                                             native, holder, launch_point)
        timing["candidate_state_pid"] = candidate.process.pid
        result["fresh_launches"].append(timing)
        token, _ = candidate.send("shutdown")
        try:
            candidate.reply(token, timeout=5)
        except Exception:
            pass
        candidate.process.wait(timeout=5)
    except Exception as error:
        result["fresh_launches"].append({"index": index, "error": repr(error)})
        result["errors"].append(f"launch {index}: {error!r}")


def run_candidate_benchmarks(executable, manifest_path, output, candidate_name,
                             launch_point, config, skip_fresh=False):
    """Per-candidate session phase. Returns the raw result dict."""
    native = regression.Native()
    native.user.SetThreadDpiAwarenessContext(ctypes.c_void_p(-4))
    ui = acceptance.UiAutomation()
    result = base_result(candidate_name)
    output.mkdir(parents=True)

    # ---------- Fresh-process startups (warm OS cache) ----------
    if not skip_fresh:
        for index in range(config["fresh_launches"]):
            fresh_launch_once(executable, manifest_path, output, native,
                              launch_point, index, result)

    # ---------- Activation + resource states in one process ----------
    run_dir = output / "session"
    run_dir.mkdir()
    holder = {"event_observations": [], "provider_lifetimes": []}
    candidate, startup = launch_candidate(executable, manifest_path, run_dir,
                                          native, holder, launch_point)
    sampler = ProcessSample(candidate.process.pid, native)
    result["session_startup"] = startup
    observer = RegionObserver(ui)

    def state():
        return candidate.call("state")["state"]

    def collect(seconds, interval=1.0, fn=None):
        rows, deadline = [], time.monotonic() + seconds
        while time.monotonic() < deadline:
            rows.append(sample_row(sampler, state() if fn is None else fn))
            time.sleep(interval)
        return rows

    try:
        # R0: fresh mascot-only — settle 60 s then collect 30 s
        time.sleep(config["settle_s"])
        result["resource_states"]["R0_fresh_mascot"] = {
            "samples": collect(config["collect_s"]),
            "state": state()}

        # W-armed idle CPU baseline over collection window
        rows = result["resource_states"]["R0_fresh_mascot"]["samples"]
        if len(rows) >= 2:
            cpu = int(rows[-1]["user_cpu_100ns"]) + int(rows[-1]["kernel_cpu_100ns"]) - \
                int(rows[0]["user_cpu_100ns"]) - int(rows[0]["kernel_cpu_100ns"])
            wall = int(rows[-1]["qpc"]) - int(rows[0]["qpc"])
            idle_pct = (cpu / 10_000_000) / (wall / QPC_FREQ) * 100
            result["resource_states"]["R0_fresh_mascot"]["idle_cpu_one_core_pct"] = \
                round(idle_pct, 4)

        # First composer activation: external hotkey -> visible + input-ready.
        # Visibility is strictly the external pixel-diff (control-reported
        # composer_visible is diagnostic only, per benchmark protocol §4.6).
        def activation():
            st = state()
            mrect = ui.rect(int(st["mascot_hwnd"]))
            watch = (mrect.left - 700, mrect.top - 520, 1400, 1040)
            baseline = observer.grab(*watch)
            start = qpc()
            ui.keys("CTRL+ALT+SPACE")
            visible, _ = observer.wait_change(watch, baseline,
                                              min_pixels=2000, timeout_s=5)
            # input readiness probe: inject a unicode char, read back exactly
            input_hwnd = int(state()["input_hwnd"])
            box = ui.rect(input_hwnd)
            ui.click((box.left + box.right) // 2, (box.top + box.bottom) // 2)
            marker = "!"
            before = candidate.call("text")["text"]["input"]
            ui.type_text(marker, settle_ms=10)
            deadline = time.monotonic() + 3
            ready = None
            while time.monotonic() < deadline:
                if candidate.call("text")["text"]["input"] == before + marker:
                    ready = qpc()
                    break
                time.sleep(0.01)
            require(ready, "Input-readiness probe not accepted")
            return {"hotkey_qpc": str(start), "visible_qpc": str(visible),
                    "input_ready_qpc": str(ready),
                    "hotkey_to_visible_ms": qpc_ms(start, visible),
                    "hotkey_to_input_ready_ms": qpc_ms(start, ready),
                    "observer": "external pixel diff only; control replies are diagnostic"}

        warm = {"first_activation": activation(), "warm": []}
        result["warm_activations"]["first"] = warm["first_activation"]
        time.sleep(0.3)
        # R1: settle after first open, collect
        time.sleep(config["settle_s"])
        result["resource_states"]["R1_first_composer"] = {
            "samples": collect(config["collect_s"]), "state": state()}

        # R2: warm open after a hide
        candidate.call("hide")
        time.sleep(0.5)
        candidate.call("show")
        time.sleep(config["settle_s"])
        result["resource_states"]["R2_warm_composer"] = {
            "samples": collect(config["collect_s"]), "state": state()}

        # R4: normal streaming, 100 ms sampling
        candidate.call("set_text", text="streaming benchmark")
        candidate.call("scenario", name="normal")
        marker = len(candidate.events)
        candidate.call("submit")
        stream_rows = [sample_row(sampler, state())]
        def pump_until_terminal():
            base = len(candidate.events)
            while not any(e.get("event") == "terminal" for e in candidate.events[marker:]):
                candidate.pump(5)
                stream_rows.append(sample_row(sampler, state()))
        pump_until_terminal()
        result["resource_states"]["R4_streaming"] = {
            "samples_100ms": stream_rows,
            "observed_max_pws": max(r["private_working_set_bytes"] or 0
                                    for r in stream_rows),
            "observed_max_commit": max(r["private_commit_bytes"]
                                       for r in stream_rows)}

        # R5: cancellation
        candidate.call("scenario", name="cancel")
        marker = len(candidate.events)
        candidate.call("submit")
        candidate.wait(lambda: any(e.get("event") == "chunk_accepted" and
                                   e.get("seq") == 49
                                   for e in candidate.events[marker:]))
        candidate.call("cancel")
        candidate.wait(lambda: any(e.get("event") == "terminal"
                                   for e in candidate.events[marker:]))
        r5 = {"peak_at_cancel": sample_row(sampler, state()), "post": {}}
        for label, delay in (("1s", 1), ("10s", 9), ("60s", 50)):
            time.sleep(delay)
            r5["post"][label] = sample_row(sampler, state())
        result["resource_states"]["R5_cancellation"] = r5

        # R3: warm mascot-only after use (post-hide decay)
        candidate.call("hide")
        r3 = {"post_hide": {}}
        for label, delay in (("1s", 1), ("10s", 9), ("60s", 50)):
            time.sleep(delay)
            r3["post_hide"][label] = sample_row(sampler, state())
        r3["steady"] = collect(config["collect_s"])
        result["resource_states"]["R3_warm_mascot"] = r3

        # Warm activation reps across this lifetime
        candidate.call("show")
        count = config["warm_reps"] // max(1, config["lifetimes"])
        for _ in range(count):
            candidate.call("hide")
            time.sleep(0.15)
            warm["warm"].append(activation())
        result["warm_activations"]["warm"] = warm["warm"]

        # R6: focused composer idle (long window)
        st = state()
        t0 = time.monotonic()
        before_r6 = sample_row(sampler, st)
        while time.monotonic() - t0 < config["r6_s"]:
            time.sleep(10)
        after_r6 = sample_row(sampler, state())
        result["resource_states"]["R6_focused_idle"] = {
            "duration_s": config["r6_s"], "before": before_r6, "after": after_r6,
            "composer_paints_delta": (after_r6.get("redraws") or 0) -
                                     (before_r6.get("redraws") or 0),
            "mascot_presents_delta": (after_r6.get("presents") or 0) -
                                     (before_r6.get("presents") or 0)}

        # Stability §7: three successive batches of 100 ops per operation
        # type (open/close, submit/complete, cancellation); sample every 10;
        # fixed content vs bounded-varying content alternating by batch.
        def op_open_close(op, batch):
            candidate.call("set_text",
                           text="fixed stability text" if batch != 1
                           else f"stability op {op} batch {batch}")
            candidate.call("show")
            candidate.call("hide")

        def op_submit(op, batch):
            candidate.call("set_text",
                           text="fixed stability text" if batch != 1
                           else f"stability op {op} batch {batch}")
            candidate.call("scenario", name="normal")
            marker = len(candidate.events)
            candidate.call("submit")
            candidate.wait(lambda: any(e.get("event") == "terminal"
                                       for e in candidate.events[marker:]),
                           timeout=15)

        def op_cancel(op, batch):
            candidate.call("set_text",
                           text="fixed stability text" if batch != 1
                           else f"stability op {op} batch {batch}")
            candidate.call("scenario", name="cancel")
            marker = len(candidate.events)
            candidate.call("submit")
            candidate.wait(lambda: any(e.get("event") == "chunk_accepted"
                                       and e.get("seq") == 49
                                       for e in candidate.events[marker:]),
                           timeout=15)
            candidate.call("cancel")
            candidate.wait(lambda: any(e.get("event") == "terminal"
                                       for e in candidate.events[marker:]),
                           timeout=15)

        stability = {"batches": []}
        for kind, op_fn in (("open_close", op_open_close),
                            ("submit_complete", op_submit),
                            ("cancel", op_cancel)):
            for batch in range(3):
                rows = []
                for op in range(100):
                    if op % 10 == 0:
                        rows.append({"op": op, **sample_row(sampler, state())})
                    op_fn(op, batch)
                rows.append({"op": 100, **sample_row(sampler, state())})
                stability["batches"].append({"operation": kind,
                                             "batch": batch,
                                             "content": "varying" if batch == 1
                                                        else "fixed",
                                             "samples": rows})
        result["stability"] = stability

        # Process inventory
        result["process_inventory"] = [
            {"pid": candidate.process.pid, "parent_pid": None, "role": "candidate",
             "counted": True},
        ] + [{"pid": row["pid"], "parent_pid": candidate.process.pid,
              "role": "provider", "counted": False,
              "exclusion_reason": "shared mock provider fixture"}
             for row in native.children(candidate.process.pid)]

        token, _ = candidate.send("shutdown")
        candidate.reply(token, timeout=5)
        candidate.process.wait(timeout=5)
        result["process_exit_code"] = candidate.process.returncode
    finally:
        sampler.close()
        candidate.close()

    result["utc_finished"] = datetime.now(timezone.utc).isoformat()
    (output / "result.json").write_text(json.dumps(result, indent=2) + "\n",
                                        encoding="utf-8", newline="\n")
    return result


def summarize(result):
    fresh = [x["startup_ms"] for x in result["fresh_launches"] if "startup_ms" in x]
    warm = [x["hotkey_to_visible_ms"] for x in
            result["warm_activations"].get("warm", [])]
    out = {"candidate": result["candidate"],
           "fresh_startup_n": len(fresh),
           "fresh_startup_median_ms": statistics.median(fresh) if fresh else None,
           "fresh_startup_range_ms": [min(fresh), max(fresh)] if fresh else None,
           "first_activation_ms":
               result["warm_activations"].get("first", {}).get("hotkey_to_input_ready_ms"),
           "warm_activation_n": len(warm),
           "warm_activation_median_ms": statistics.median(warm) if warm else None,
           "warm_activation_p95_ms":
               (statistics.quantiles(warm, n=20)[18] if len(warm) >= 20 else None)}
    return out


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--exe", type=pathlib.Path)
    parser.add_argument("--exe-list", nargs="*", default=[],
                        metavar="NAME=PATH",
                        help="orchestrate mode: one NAME=PATH per candidate")
    parser.add_argument("--manifest", required=True, type=pathlib.Path)
    parser.add_argument("--output", required=True, type=pathlib.Path)
    parser.add_argument("--candidate")
    parser.add_argument("--launch-point", nargs=2, type=int, required=True,
                        metavar=("X", "Y"))
    parser.add_argument("--fresh-launches", type=int, default=30)
    parser.add_argument("--lifetimes", type=int, default=3,
                        help="Process lifetimes for warm-activation reps")
    parser.add_argument("--warm-reps", type=int, default=99)
    parser.add_argument("--settle-s", type=float, default=60.0)
    parser.add_argument("--collect-s", type=float, default=30.0)
    parser.add_argument("--r6-s", type=float, default=600.0)
    parser.add_argument("--quick", action="store_true",
                        help="Diagnostic pass: 3 launches, 6 warm reps, 5 s settles")
    parser.add_argument("--orchestrate", action="store_true",
                        help="Balanced all-candidate run; --exe repeated as NAME=PATH")
    args = parser.parse_args()

    if args.orchestrate:
        # exe args carry NAME=PATH pairs; runs interleaved fresh launches in
        # manifest balanced_blocks order, then each candidate's session phase.
        require(not args.output.exists(), "Refusing to overwrite evidence directory")
        args.output.mkdir(parents=True)
        manifest = load(args.manifest)
        blocks = manifest["schedule"]["balanced_blocks"]
        native = regression.Native()
        native.user.SetThreadDpiAwarenessContext(ctypes.c_void_p(-4))
        candidates = {}
        for spec in args.exe_list:
            name, _, path = spec.partition("=")
            path = pathlib.Path(path).resolve()
            out = args.output / name
            out.mkdir(parents=True)
            candidates[name] = {"exe": path, "output": out,
                                "result": base_result(name)}
        n = 3 if args.quick else args.fresh_launches
        orders = []
        for rep in range(n):
            order = blocks[rep % len(blocks)]
            orders.append(order)
            for name in order:
                if name in candidates:
                    fresh_launch_once(candidates[name]["exe"], args.manifest,
                                      candidates[name]["output"], native,
                                      tuple(args.launch_point), rep,
                                      candidates[name]["result"])
        config = {"fresh_launches": n, "lifetimes": args.lifetimes,
                  "warm_reps": 6 if args.quick else args.warm_reps,
                  "settle_s": 5.0 if args.quick else args.settle_s,
                  "collect_s": 10.0 if args.quick else args.collect_s,
                  "r6_s": 30.0 if args.quick else args.r6_s,
                  "balanced_launch_order": orders}
        for name, entry in candidates.items():
            session_result = run_candidate_benchmarks(
                entry["exe"], args.manifest, entry["output"] / "session-run",
                name, tuple(args.launch_point), config, skip_fresh=True)
            entry["result"].update({k: v for k, v in session_result.items()
                                    if k != "fresh_launches"})
            entry["result"]["fresh_launch_order"] = orders
            entry["result"]["config"] = config
            (entry["output"] / "result.json").write_text(
                json.dumps(entry["result"], indent=2) + "\n",
                encoding="utf-8", newline="\n")
        print(json.dumps({name: summarize(entry["result"])
                          for name, entry in candidates.items()}))
        return

    require(args.exe is not None and args.candidate is not None,
            "--exe and --candidate required for single-candidate mode")
    require(args.output.parent.exists(), "Output parent missing")
    require(not args.output.exists(), "Refusing to overwrite evidence directory")
    config = {"fresh_launches": 3 if args.quick else args.fresh_launches,
              "lifetimes": args.lifetimes,
              "warm_reps": 6 if args.quick else args.warm_reps,
              "settle_s": 5.0 if args.quick else args.settle_s,
              "collect_s": 10.0 if args.quick else args.collect_s,
              "r6_s": 30.0 if args.quick else args.r6_s}
    result = run_candidate_benchmarks(args.exe.resolve(), args.manifest.resolve(),
                                      args.output, args.candidate,
                                      tuple(args.launch_point), config)
    result["config"] = config
    (args.output / "result.json").write_text(json.dumps(result, indent=2) + "\n",
                                             encoding="utf-8", newline="\n")
    print(json.dumps(summarize(result)))


if __name__ == "__main__":
    main()
