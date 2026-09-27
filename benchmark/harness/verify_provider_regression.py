import argparse
import base64
import ctypes
import hashlib
import json
import os
import pathlib
import queue
import re
import subprocess
import sys
import threading
import time
from ctypes import wintypes
from datetime import datetime, timezone

from verify_candidate_smoke import load, require


class ProcessEntry(ctypes.Structure):
    _fields_ = [("size", wintypes.DWORD), ("usage", wintypes.DWORD),
                ("pid", wintypes.DWORD), ("heap", ctypes.c_size_t),
                ("module", wintypes.DWORD), ("threads", wintypes.DWORD),
                ("parent", wintypes.DWORD), ("priority", wintypes.LONG),
                ("flags", wintypes.DWORD), ("name", wintypes.WCHAR * 260)]


class Native:
    def __init__(self):
        self.kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        self.user = ctypes.WinDLL("user32", use_last_error=True)
        signatures = {
            "QueryPerformanceCounter": ([ctypes.POINTER(ctypes.c_int64)], wintypes.BOOL),
            "QueryPerformanceFrequency": ([ctypes.POINTER(ctypes.c_int64)], wintypes.BOOL),
            "CreateToolhelp32Snapshot": ([wintypes.DWORD, wintypes.DWORD], wintypes.HANDLE),
            "Process32FirstW": ([wintypes.HANDLE, ctypes.POINTER(ProcessEntry)], wintypes.BOOL),
            "Process32NextW": ([wintypes.HANDLE, ctypes.POINTER(ProcessEntry)], wintypes.BOOL),
            "OpenProcess": ([wintypes.DWORD, wintypes.BOOL, wintypes.DWORD], wintypes.HANDLE),
            "CloseHandle": ([wintypes.HANDLE], wintypes.BOOL),
            "WaitForSingleObject": ([wintypes.HANDLE, wintypes.DWORD], wintypes.DWORD),
            "GetExitCodeProcess": ([wintypes.HANDLE, ctypes.POINTER(wintypes.DWORD)], wintypes.BOOL),
            "GetProcessTimes": ([wintypes.HANDLE] + [ctypes.POINTER(wintypes.FILETIME)] * 4, wintypes.BOOL),
            "TerminateProcess": ([wintypes.HANDLE, wintypes.UINT], wintypes.BOOL),
        }
        for name, (arguments, result) in signatures.items():
            function = getattr(self.kernel, name)
            function.argtypes, function.restype = arguments, result
        self.user.SendMessageTimeoutW.argtypes = [wintypes.HWND, wintypes.UINT, ctypes.c_size_t,
                                                ctypes.c_ssize_t, wintypes.UINT, wintypes.UINT,
                                                ctypes.POINTER(ctypes.c_size_t)]
        self.user.SendMessageTimeoutW.restype = ctypes.c_ssize_t
        self.user.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
        self.user.GetWindowThreadProcessId.restype = wintypes.DWORD
        self.user.SetThreadDpiAwarenessContext.argtypes = [ctypes.c_void_p]
        self.user.SetThreadDpiAwarenessContext.restype = ctypes.c_void_p
        self.user.GetCursorPos.argtypes = [ctypes.POINTER(wintypes.POINT)]
        self.user.GetCursorPos.restype = wintypes.BOOL
        self.user.SetCursorPos.argtypes = [ctypes.c_int, ctypes.c_int]
        self.user.SetCursorPos.restype = wintypes.BOOL
        self.user.MonitorFromPoint.argtypes = [wintypes.POINT, wintypes.DWORD]
        self.user.MonitorFromPoint.restype = wintypes.HANDLE
        self.user.GetDpiForWindow.argtypes = [wintypes.HWND]
        self.user.GetDpiForWindow.restype = wintypes.UINT
        self.frequency = self.counter("QueryPerformanceFrequency")

    def counter(self, name="QueryPerformanceCounter"):
        value = ctypes.c_int64()
        require(getattr(self.kernel, name)(ctypes.byref(value)) and value.value > 0, name + " failed")
        return value.value

    def children(self, pid):
        snapshot = self.kernel.CreateToolhelp32Snapshot(2, 0)
        require(snapshot and snapshot != ctypes.c_void_p(-1).value, "Process snapshot failed")
        rows = []
        try:
            entry = ProcessEntry()
            entry.size = ctypes.sizeof(entry)
            require(self.kernel.Process32FirstW(snapshot, ctypes.byref(entry)), "Process enumeration failed")
            while True:
                if entry.parent == pid:
                    rows.append({"pid": entry.pid, "parent_pid": entry.parent, "image_name": entry.name})
                if not self.kernel.Process32NextW(snapshot, ctypes.byref(entry)):
                    require(ctypes.get_last_error() == 18, "Process enumeration ended unexpectedly")
                    break
        finally:
            self.kernel.CloseHandle(snapshot)
        return rows

    def text(self, hwnd, pid, bound):
        owner = wintypes.DWORD()
        require(self.user.GetWindowThreadProcessId(hwnd, ctypes.byref(owner)) and owner.value == pid,
                "Text HWND is not owned by candidate")
        length = ctypes.c_size_t()
        require(self.user.SendMessageTimeoutW(hwnd, 0x000E, 0, 0, 2, 2000, ctypes.byref(length)),
                "Native text length query timed out")
        require(length.value <= bound, "Native text exceeds configured bound")
        buffer = ctypes.create_unicode_buffer(length.value + 1)
        copied = ctypes.c_size_t()
        require(self.user.SendMessageTimeoutW(hwnd, 0x000D, len(buffer), ctypes.addressof(buffer), 2, 2000,
                                             ctypes.byref(copied)), "Native text query timed out")
        require(copied.value <= length.value, "Native text copy exceeds buffer")
        return buffer.value.replace("\r\n", "\n").replace("\r", "\n")


class Child:
    def __init__(self, native, pid, parent, image):
        rows = native.children(parent)
        require(any(row["pid"] == pid and row["image_name"].casefold() == image.casefold() for row in rows),
                "Reported provider is not the expected owned child")
        self.native, self.pid = native, pid
        self.handle = native.kernel.OpenProcess(0x100000 | 0x1000 | 1, False, pid)
        require(self.handle, "Cannot open owned provider process")
        times = [wintypes.FILETIME() for _ in range(4)]
        if not native.kernel.GetProcessTimes(self.handle, *(ctypes.byref(value) for value in times)):
            self.close()
            raise AssertionError("Cannot query provider creation time")
        self.creation = (times[0].dwHighDateTime << 32) | times[0].dwLowDateTime

    def alive(self):
        value = self.native.kernel.WaitForSingleObject(self.handle, 0)
        require(value in (0, 258), "Provider process wait failed")
        return value == 258

    def exited(self, timeout_ms, expected=None):
        require(self.native.kernel.WaitForSingleObject(self.handle, timeout_ms) == 0,
                "Owned provider did not exit within deadline")
        observed = self.native.counter()
        code = wintypes.DWORD()
        require(self.native.kernel.GetExitCodeProcess(self.handle, ctypes.byref(code)), "Cannot query provider exit code")
        if expected is not None:
            require(code.value == expected, f"Provider exit {code.value}, expected {expected}")
        return {"pid": self.pid, "creation_filetime_100ns": str(self.creation),
                "exit_code": code.value, "exit_observed_qpc": str(observed)}

    def close(self):
        if self.handle:
            self.native.kernel.CloseHandle(self.handle)
            self.handle = None


class ExitWatch:
    def __init__(self, child, origin_qpc, timeout_ms, expected):
        self.results = queue.Queue(maxsize=1)
        self.origin = origin_qpc
        self.deadline = origin_qpc + timeout_ms * child.native.frequency // 1000

        def observe():
            try:
                remaining = max(0, self.deadline - child.native.counter())
                milliseconds = (remaining * 1000 + child.native.frequency - 1) // child.native.frequency
                row = child.exited(milliseconds, expected)
                require(int(row["exit_observed_qpc"]) <= self.deadline,
                        "Process exit was not externally observed within the shared deadline")
                row["deadline_origin_qpc"] = str(origin_qpc)
                self.results.put((row, None))
            except Exception as error:
                self.results.put((None, repr(error)))

        self.thread = threading.Thread(target=observe, daemon=True)
        self.thread.start()

    def finish(self):
        self.thread.join(timeout=3)
        require(not self.thread.is_alive(), "Exit observer did not finish")
        row, error = self.results.get_nowait()
        require(error is None, str(error))
        return row


class Candidate:
    def __init__(self, executable, manifest_path, output, native, result):
        self.native, self.result, self.output = native, result, output
        self.manifest = load(manifest_path)
        self.root = manifest_path.parent.parent
        self.control = load(self.root / self.manifest["candidate_control"])
        self.sequence = 0
        self.replies, self.events, self.reader_errors, self.children = {}, [], [], []
        self.pending = set()
        self.exit_watches = []
        self.on_event = None
        self.messages = queue.Queue(maxsize=1024)
        self.stderr = (output / "application.stderr.log").open("xb")
        self.commands = (output / "commands.ndjson").open("x", encoding="utf-8", newline="\n")
        environment = os.environ.copy()
        for key in list(environment):
            if key.upper() in {"GOMAXPROCS", "GOGC", "GOMEMLIMIT", "GODEBUG", "GOTRACEBACK"}:
                del environment[key]
        environment.update(GOGC="100", GOMEMLIMIT="off", GODEBUG="", GOTRACEBACK="single")
        self.process = subprocess.Popen([str(executable), "--fixture", str(manifest_path), "--control"],
                                        cwd=executable.parent, env=environment, stdin=subprocess.PIPE,
                                        stdout=subprocess.PIPE, stderr=self.stderr,
                                        creationflags=subprocess.CREATE_NO_WINDOW)
        self.reader = threading.Thread(target=self.read_output, daemon=True)
        self.reader.start()

    def read_output(self):
        try:
            with (self.output / "application.stdout.ndjson").open("xb") as raw:
                while True:
                    line = self.process.stdout.readline(self.control["max_reply_bytes_including_lf"] + 1)
                    if not line:
                        break
                    stamp = self.native.counter()
                    raw.write(line)
                    raw.flush()
                    require(len(line) <= self.control["max_reply_bytes_including_lf"] and line.endswith(b"\n"),
                            "Oversized or incomplete control record")
                    self.messages.put_nowait((stamp, json.loads(line.decode("utf-8"))))
        except Exception as error:
            self.reader_errors.append(repr(error))
        finally:
            try:
                self.messages.put_nowait((self.native.counter(), {"reader_eof": True}))
            except queue.Full:
                self.reader_errors.append("Observer message queue overflow at EOF")

    def send(self, command, **fields):
        self.sequence += 1
        require(len(self.pending) < self.control["max_pending_commands"], "Observer exceeded pending-command bound")
        value = dict(token=self.sequence, command=command, **fields)
        wire = (json.dumps(value, ensure_ascii=False, separators=(",", ":")) + "\n").encode("utf-8")
        require(len(wire) <= self.control["max_command_bytes_including_lf"], "Observer command exceeds bound")
        stamp = self.native.counter()
        self.process.stdin.write(wire)
        self.process.stdin.flush()
        self.pending.add(self.sequence)
        self.commands.write(json.dumps({"sent_qpc": str(stamp), "command": value}, ensure_ascii=False) + "\n")
        self.commands.flush()
        return self.sequence, stamp

    def pump(self, timeout=10, eof_ok=False):
        require(not self.reader_errors, str(self.reader_errors))
        stamp, message = self.messages.get(timeout=timeout)
        if message.get("reader_eof"):
            require(eof_ok, "Candidate stdout ended before expected result")
            return False
        if "event" in message:
            timestamp = {"frame_received": "receipt_qpc", "chunk_accepted": "accepted_qpc", "terminal": "qpc"}
            require(message["event"] in timestamp, "Unexpected control event type")
            require(decimal(message[timestamp[message["event"]]]) <= stamp, "Event timestamp is later than observation")
            self.events.append(message)
            self.result["event_observations"].append({"observer_qpc": str(stamp), "event": message})
            if self.on_event is not None:
                self.on_event(message)
        else:
            token = message.get("token")
            require(type(token) is int and token in self.pending and token not in self.replies,
                    "Unsolicited or duplicate control reply")
            self.replies[token] = message
        return True

    def reply(self, token, timeout=10):
        deadline = time.monotonic() + timeout
        while token not in self.replies:
            self.pump(max(0.001, deadline - time.monotonic()))
            require(time.monotonic() <= deadline, "Control command deadline exceeded")
        reply = self.replies.pop(token)
        self.pending.remove(token)
        require(reply.get("ok") is True, str(reply))
        return reply

    def call(self, command, **fields):
        token, _ = self.send(command, **fields)
        return self.reply(token)

    def wait(self, predicate, timeout=10):
        deadline = time.monotonic() + timeout
        while not predicate():
            self.pump(max(0.001, deadline - time.monotonic()))
            require(time.monotonic() <= deadline, "Event deadline exceeded")

    def capture_child(self, state):
        pid = state["provider_pid"]
        require(pid > 0, "Provider PID was not observable")
        for child in self.children:
            if child.pid == pid and child.alive():
                return child
        child = Child(self.native, pid, self.process.pid, pathlib.Path(self.manifest["provider"]["path"]).name)
        self.children.append(child)
        self.result["provider_lifetimes"].append({"pid": pid, "creation_filetime_100ns": str(child.creation)})
        return child

    def close(self):
        cleanup, errors = [], []
        try:
            image = pathlib.Path(self.manifest["provider"]["path"]).name
            for row in self.native.children(self.process.pid):
                if row["image_name"].casefold() == image.casefold():
                    self.capture_child({"provider_pid": row["pid"]})
                else:
                    errors.append({"unrecognized_owned_child": row})
        except Exception as error:
            errors.append(repr(error))
        try:
            if self.process.poll() is None:
                self.process.stdin.close()
                try:
                    self.process.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    cleanup.append("forced candidate termination after failed check")
                    self.process.terminate()
                    self.process.wait(timeout=3)
        except Exception as error:
            errors.append(repr(error))
        for child in self.children:
            try:
                if child.alive():
                    cleanup.append({"forced_provider_termination": child.pid})
                    require(self.native.kernel.TerminateProcess(child.handle, 125), "Cannot clean up owned provider")
                    child.exited(2000)
            except Exception as error:
                errors.append(repr(error))
        for watch in self.exit_watches:
            watch.thread.join(timeout=3)
            if watch.thread.is_alive():
                errors.append("Exit observer still running; native handles retained")
        if not any(watch.thread.is_alive() for watch in self.exit_watches):
            for child in self.children:
                child.close()
        self.reader.join(timeout=3)
        if self.reader.is_alive():
            errors.append("Observer reader did not stop; stdout left owned by reader")
        else:
            self.process.stdout.close()
        if not self.process.stdin.closed:
            self.process.stdin.close()
        self.stderr.close()
        self.commands.close()
        self.result["cleanup"] = cleanup
        self.result["cleanup_errors"] = errors
        self.result["reader_errors"] = self.reader_errors


def decimal(value):
    require(isinstance(value, str) and value.isascii() and value.isdecimal(), "Invalid QPC decimal string")
    number = int(value)
    require(0 < number < 2 ** 63, "Invalid QPC value")
    return number


def verify_event_stream(events, request_id, expected_count, frequency):
    require(all(type(row.get("request_id")) is int and row["request_id"] == request_id for row in events),
            "Unexpected request in event interval")
    require(all(row["event"] in ("frame_received", "chunk_accepted", "terminal") for row in events), "Unknown event")
    receipts = [row for row in events if row["event"] == "frame_received"]
    accepted = [row for row in events if row["event"] == "chunk_accepted"]
    terminals = [row for row in events if row["event"] == "terminal"]
    require(all(type(row["seq"]) is int for row in receipts + accepted), "Sequence must be an integer")
    require([row["seq"] for row in receipts] == list(range(expected_count)), "Receipt sequence differs")
    require([row["seq"] for row in accepted] == list(range(expected_count)), "Accepted sequence differs")
    previous_receipt = previous_acceptance = 0
    for receipt, acceptance in zip(receipts, accepted):
        require(type(receipt["qpc_frequency"]) is int and receipt["qpc_frequency"] == frequency, "QPC frequency differs")
        require(len(receipt["emit_qpc"]) == 20, "Emission stamp lost its frozen width")
        received_qpc, accepted_qpc = decimal(receipt["receipt_qpc"]), decimal(acceptance["accepted_qpc"])
        require(decimal(receipt["emit_qpc"]) <= received_qpc <= accepted_qpc, "Event clocks are not ordered")
        require(received_qpc >= previous_receipt and accepted_qpc >= previous_acceptance, "Event clock moved backward")
        previous_receipt, previous_acceptance = received_qpc, accepted_qpc
    for terminal in terminals:
        require(decimal(terminal["qpc"]) >= previous_acceptance, "Terminal precedes accepted chunk")
    return terminals


def verify_events(events, request_id, expected_count, kind, frequency):
    terminals = verify_event_stream(events, request_id, expected_count, frequency)
    require(len(terminals) == 1 and terminals[0]["kind"] == kind and type(terminals[0]["last_seq"]) is int
            and terminals[0]["last_seq"] == expected_count - 1, "Terminal count/kind/sequence differs")
    return terminals[0]


def expected_text(scenario, request_id, chunks, vectors, frame_limit):
    if scenario == "maximum":
        vector = next(row for row in vectors if row["name"] == "maximum_valid")
        wire = base64.b64decode(vector["expected_frames_base64"][0], validate=True)
        require(len(wire) == frame_limit, "Invalid frozen maximum vector")
        reference = json.loads(wire)
        require(set(reference["text"]) == {"x"} and reference["seq"] == 0, "Unexpected maximum-frame template")
        size = len(reference["text"]) + len(str(reference["id"])) - len(str(request_id))
        return "x" * size, 1
    count = {"cancel": 50, "unexpected_exit": 7, "oversized": 0, "backpressure": 256}.get(scenario, 100)
    return "".join(chunks[index % len(chunks)] for index in range(count)), count


def run_case(candidate, scenario, previous_child, hidden=False):
    manifest, native = candidate.manifest, candidate.native
    event_start = len(candidate.events)
    before = candidate.call("state")["state"]
    candidate.call("show")
    fixture = load(candidate.root / manifest["text_fixture"])
    candidate.call("set_text", text=fixture["F10"])
    candidate.call("scenario", name=scenario)
    cancel, hide = {}, {}
    request_id = None
    exit_watch = None
    kind = "failed" if scenario in ("unexpected_exit", "oversized") else "cancelled" if scenario == "cancel" else "complete"

    def on_event(event):
        nonlocal exit_watch
        if request_id is not None and event.get("request_id") != request_id:
            raise AssertionError("Event from another request reached the active case")
        if hidden and event.get("event") == "chunk_accepted" and event.get("seq") == 0:
            require(not hide, "Duplicate hidden-stream trigger")
            token, stamp = candidate.send("hide")
            hide.update(token=token, sent_qpc=stamp)
        if scenario == "cancel" and event.get("event") == "chunk_accepted" and event.get("seq") == 49:
            require(not cancel, "Duplicate canonical cancellation trigger")
            token, stamp = candidate.send("cancel")
            cancel.update(token=token, sent_qpc=stamp)
        if kind == "failed" and event.get("event") == "terminal":
            require(previous_child is not None and exit_watch is None, "Exceptional terminal lacks unique known child lifetime")
            exit_watch = ExitWatch(previous_child, decimal(event["qpc"]), manifest["protocol"]["shutdown_timeout_ms"],
                                   23 if scenario == "unexpected_exit" else None)
            candidate.exit_watches.append(exit_watch)

    candidate.on_event = on_event
    submitted = candidate.call("submit")["state"]
    request_id = submitted["request_id"]
    require(type(request_id) is int and request_id > 0, "Invalid request ID")
    require(submitted["request_count"] == before["request_count"] + 1, "Request count differs after submit")
    terminals = lambda: [row for row in candidate.events[event_start:] if row.get("event") == "terminal" and row.get("request_id") == request_id]
    candidate.wait(lambda: bool(terminals()))
    candidate.on_event = None
    if cancel:
        candidate.reply(cancel["token"])
    if hidden:
        require(hide, "Active hide was not exercised")
        hide_state = candidate.reply(hide["token"])["state"]
        require(hide_state["composer_visible"] is False and hide_state["provider_state"] == "streaming",
                "Hide did not occur while the request remained active")
    state = candidate.call("state")["state"]
    expected, count = expected_text(scenario, request_id, load(candidate.root / "fixtures/chunks.json"),
                                    load(candidate.root / manifest["decoder_vectors"]),
                                    manifest["protocol"]["max_stdout_frame_bytes_including_lf"])
    terminal = verify_events(candidate.events[event_start:], request_id, count, kind, native.frequency)
    require(state["provider_state"] == kind and state["last_seq"] == count - 1, "Terminal state differs")
    require(state["response_utf8_bytes"] == len(expected.encode("utf-8")), "Response byte count differs")
    require(state["request_count"] == before["request_count"] + 1, "Unexpected automatic replay")
    require(0 <= state["queued_provider_frames"] <= state["queue_capacity_frames"] and state["queue_capacity_frames"] > 0,
            "Invalid queue bound")
    require(not state["cache_counts"].get("run_invalid"), "Candidate reported invalid instrumentation")
    snapshot = candidate.call("text")["text"]
    require(snapshot["input"] == fixture["F10"] and snapshot["response"] == expected, "Control text snapshot differs")
    native_body = native.text(int(state["response_hwnd"]), candidate.process.pid,
                              2 * manifest["ui"]["response_limit_utf8_bytes"] + 8192)
    require(native_body.endswith(expected), "Actual native response does not contain the exact current body")
    prefix = native_body[:-len(expected)] if expected else native_body
    history = load(candidate.root / "fixtures/history.json")
    require(re.fullmatch(r"\s*" + r"\s*".join(re.escape(value) for value in history) + r"\s*", prefix),
            "Native view does not contain only the four fixed history messages before the current response")
    if hidden:
        require(state["composer_visible"] is False, "Streaming or cancellation unexpectedly showed composer")
        candidate.call("show")
        require(candidate.call("text")["text"]["response"] == expected, "Reopen lost hidden response")
    record = {"scenario": scenario, "hidden": hidden, "request_id": request_id, "terminal": terminal,
              "response_sha256": hashlib.sha256(expected.encode("utf-8")).hexdigest(),
              "response_utf8_bytes": len(expected.encode("utf-8")), "chunks": count, "state": state,
              "native_response_verified": True, "status": "PASS_PROVIDER_REGRESSION_ONLY"}
    if hidden:
        record["hide_command_qpc"] = str(hide["sent_qpc"])
        record["active_hide_state"] = hide_state
    if cancel:
        duration = (decimal(terminal["qpc"]) - cancel["sent_qpc"]) * 1000 / native.frequency
        require(0 <= duration <= manifest["protocol"]["cancel_timeout_ms"], "Cancellation exceeded shared timeout")
        record["cancel_command_to_terminal_ms"] = duration
    if kind == "failed":
        require(exit_watch is not None, "Exceptional process-exit observer was not armed")
        record["provider_exit"] = exit_watch.finish()
        deadline = time.monotonic() + 2
        while state["provider_pid"] != 0:
            require(time.monotonic() < deadline, "Candidate retained failed-session PID")
            state = candidate.call("state")["state"]
            time.sleep(0.01)
        require(not native.children(candidate.process.pid), "Descendant remained after exceptional teardown")
        require(state["request_count"] == before["request_count"] + 1, "Failed request was replayed")
        record["post_teardown_state"] = state
        child = None
    else:
        child = candidate.capture_child(state)
        require(child.alive(), "Provider did not persist after ordinary terminal")
        if previous_child is not None:
            require(child is previous_child, "Ordinary request replaced provider process lifetime")
        children = native.children(candidate.process.pid)
        require([row["pid"] for row in children] == [child.pid], "Unexpected candidate-owned helper process")
        record["provider_creation_filetime_100ns"] = str(child.creation)
    record["event_start"] = event_start
    record["event_end"] = len(candidate.events)
    case_events = candidate.events[event_start:record["event_end"]]
    require(all(row.get("request_id") == request_id for row in case_events), "Stale or unrelated events reached case")
    verify_events(case_events, request_id, count, kind, native.frequency)
    candidate.result["cases"].append(record)
    return child


def shutdown(candidate, child, token=None, stamp=None, watch=None):
    require(child is not None, "Missing persistent provider before shutdown")
    if token is None:
        token, stamp = candidate.send("shutdown")
        watch = ExitWatch(child, stamp, candidate.manifest["protocol"]["shutdown_timeout_ms"], 0)
        candidate.exit_watches.append(watch)
    reply = candidate.reply(token, timeout=5)
    require(candidate.process.wait(timeout=3) == 0, "Candidate shutdown exit was nonzero")
    while candidate.pump(timeout=3, eof_ok=True):
        pass
    require(not candidate.pending, "Missing command reply at shutdown")
    require(not candidate.native.children(candidate.process.pid), "Descendant remained after app shutdown")
    candidate.result["shutdown"] = {"command_qpc": str(stamp), "provider": watch.finish(), "reply": reply}


def active_shutdown(candidate, child):
    event_start = len(candidate.events)
    candidate.call("scenario", name="cancel")
    trigger = {}

    def on_event(event):
        if event.get("event") == "chunk_accepted" and event.get("seq") == 49:
            require(not trigger, "Duplicate shutdown barrier trigger")
            token, stamp = candidate.send("shutdown")
            watch = ExitWatch(child, stamp, candidate.manifest["protocol"]["shutdown_timeout_ms"], 0)
            candidate.exit_watches.append(watch)
            trigger.update(token=token, stamp=stamp, watch=watch)

    candidate.on_event = on_event
    submitted = candidate.call("submit")["state"]
    candidate.wait(lambda: bool(trigger))
    candidate.on_event = None
    shutdown(candidate, child, **trigger)
    events = candidate.events[event_start:]
    terminals = verify_event_stream(events, submitted["request_id"], 50, candidate.native.frequency)
    require(len(terminals) <= 1, "Duplicate active-shutdown terminal")
    if terminals:
        require(terminals[0]["kind"] == "cancelled" and type(terminals[0]["last_seq"]) is int
                and terminals[0]["last_seq"] == 49, "Unexpected active-shutdown terminal")
    candidate.result["active_shutdown"] = {"request_id": submitted["request_id"], "accepted_chunks": 50,
                                           "terminals": terminals, "event_start": event_start,
                                           "event_end": len(candidate.events),
                                           "rule": "Shutdown at frozen barrier; ACK-only fixture shutdown needs no per-request terminal"}


def run(executable, manifest_path, output, native, result, shutdown_active=False):
    candidate = Candidate(executable, manifest_path, output, native, result)
    try:
        initial = candidate.call("state")["state"]
        require(initial["provider_pid"] == 0 and initial["request_count"] == 0, "Unexpected initial provider")
        require(native.user.GetDpiForWindow(int(initial["mascot_hwnd"])) == 96, "Expected 100-percent launch display")
        child = None
        cases = (("normal", False),) if shutdown_active else (
            ("normal", False), ("normal", False), ("fragmented", False), ("client_request", False),
            ("cancel", False), ("normal", False), ("stderr", False), ("backpressure", False),
            ("maximum", False), ("normal", True), ("cancel", True), ("unexpected_exit", False),
            ("normal", False), ("oversized", False), ("normal", False))
        for scenario, hidden in cases:
            child = run_case(candidate, scenario, child, hidden)
        if shutdown_active:
            active_shutdown(candidate, child)
        else:
            shutdown(candidate, child)
            require(len(candidate.events) == result["cases"][-1]["event_end"], "Unexpected late events after final ordinary terminal")
        for case in result["cases"]:
            verify_events(candidate.events[case["event_start"]:case["event_end"]], case["request_id"],
                          case["chunks"], case["terminal"]["kind"], native.frequency)
    finally:
        candidate.close()
    require(not result["cleanup"] and not result["cleanup_errors"] and not result["reader_errors"],
            "Observer cleanup or reader failed")
    result["status"] = "PASS_PROVIDER_REGRESSION_ONLY"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--exe", type=pathlib.Path, required=True)
    parser.add_argument("--manifest", type=pathlib.Path, required=True)
    parser.add_argument("--output", type=pathlib.Path, required=True)
    parser.add_argument("--candidate", choices=("rust", "zig", "go"), required=True)
    parser.add_argument("--shutdown-active", action="store_true")
    args = parser.parse_args()
    if os.name != "nt":
        parser.error("Native Windows is required")
    args.output.mkdir(exist_ok=False)
    manifest_path = args.manifest.resolve()
    executable = args.exe.resolve()
    manifest = load(manifest_path)
    root = manifest_path.parent.parent
    result = {"schema": "mascot-provider-regression-1", "candidate": args.candidate,
              "mode": "active_shutdown" if args.shutdown_active else "scenario_sweep",
              "command": sys.argv, "launch_point": [4352, 384], "expected_dpi": 96,
              "candidate_runtime_environment": {"GOMAXPROCS": "unset", "GOGC": "100", "GOMEMLIMIT": "off",
                                                "GODEBUG": "", "GOTRACEBACK": "single"},
              "fixture_version": manifest["version"], "executable": str(executable),
              "executable_sha256": hashlib.sha256(executable.read_bytes()).hexdigest(),
              "observer_sha256": hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest(),
              "observer_import_sha256": {"verify_candidate_smoke.py": hashlib.sha256(
                  pathlib.Path(__file__).with_name("verify_candidate_smoke.py").read_bytes()).hexdigest()},
              "scope": "Targeted provider/control regression only; not full acceptance, visual or performance qualification",
              "limitations": ["No physical keyboard/IME or visual rendering judgement", "No stability/leak verdict",
                              "Protocol timestamps are diagnostic, not visible endpoints",
                              "Exceptional reap deadline observed relative to reported failed terminal, not independently instrumented decoder rejection",
                              "Active-shutdown mode covers the frozen barrier, not arbitrary instruction-boundary races"],
              "utc_started": datetime.now(timezone.utc).isoformat(), "status": "FAIL",
              "cases": [], "event_observations": [], "provider_lifetimes": [], "error": None}
    native = Native()
    result["qpc_frequency"] = native.frequency
    previous_dpi = None
    cursor = wintypes.POINT()
    cursor_saved = False
    try:
        for path, expected in manifest["files_sha256"].items():
            require(hashlib.sha256((root / path).read_bytes()).hexdigest() == expected, "Frozen fixture hash mismatch: " + path)
        require(hashlib.sha256(pathlib.Path(manifest["provider"]["path"]).read_bytes()).hexdigest() == manifest["provider"]["sha256"],
                "Provider binary differs from frozen fixture")
        previous_dpi = native.user.SetThreadDpiAwarenessContext(ctypes.c_void_p(-4))
        require(previous_dpi, "Cannot set observer DPI context")
        require(native.user.MonitorFromPoint(wintypes.POINT(4352, 384), 0), "Frozen launch display is not active")
        cursor_saved = bool(native.user.GetCursorPos(ctypes.byref(cursor)))
        require(cursor_saved and native.user.SetCursorPos(4352, 384), "Cannot use frozen 100-percent launch display")
        run(executable, manifest_path, args.output, native, result, args.shutdown_active)
    except Exception as error:
        result["error"] = repr(error)
    finally:
        if cursor_saved and not native.user.SetCursorPos(cursor.x, cursor.y):
            result["status"] = "FAIL"
            result["cursor_restore_error"] = ctypes.get_last_error()
        if previous_dpi and not native.user.SetThreadDpiAwarenessContext(previous_dpi):
            result["status"] = "FAIL"
            result["dpi_restore_error"] = ctypes.get_last_error()
    result["utc_finished"] = datetime.now(timezone.utc).isoformat()
    (args.output / "result.json").write_text(json.dumps(result, indent=2, ensure_ascii=False) + "\n", encoding="utf-8", newline="\n")
    print(json.dumps({"status": result["status"], "error": result["error"], "result": str(args.output / "result.json")}))
    return 0 if result["status"] == "PASS_PROVIDER_REGRESSION_ONLY" else 1


if __name__ == "__main__":
    sys.exit(main())
