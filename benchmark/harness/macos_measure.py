#!/usr/bin/env python3
"""macOS Stage B measurement harness.

Collects the handoff's minimum measurement set for one candidate:
launch-to-visible latency, first/warm composer activation, mascot-only and
composer-open RSS + physical footprint, streaming peak, idle CPU, thread
count, child-process inventory, clean shutdown, and short show/hide and
submit/cancel stability loops.

Measurement sources: CGWindowList bounds helper (visibility + latency),
`ps` (RSS, threads), `footprint --noCategories -f bytes` (physical
footprint), `top -l 2` (CPU%), pgrep (children).

Usage:
    python3 macos_measure.py <rust|go> <app-binary> <manifest> <out-dir>
"""

import json
import os
import queue
import re
import subprocess
import sys
import threading
import time

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
BOUNDS = os.path.join(REPO, "benchmark", "harness", "macos_window_bounds")


def rss_bytes(pid):
    out = subprocess.run(["ps", "-o", "rss=", "-p", str(pid)],
                         capture_output=True, text=True).stdout.strip()
    return int(out) * 1024 if out else None


def footprint_bytes(pid):
    out = subprocess.run(
        ["footprint", "--noCategories", "-f", "bytes", str(pid)],
        capture_output=True, text=True)
    m = re.search(r"phys_footprint:\s*(\d+)\s*B", out.stdout)
    return int(m.group(1)) if m else None


def thread_count(pid):
    out = subprocess.run(["ps", "-M", "-p", str(pid)],
                         capture_output=True, text=True).stdout
    return max(0, len(out.strip().splitlines()) - 1)


def cpu_percent(pid, settle_s=1.0):
    # top -l 2 reports a delta sample; take the second snapshot's process row.
    out = subprocess.run(
        ["top", "-l", "2", "-pid", str(pid), "-stats", "pid,cpu", "-s", "1"],
        capture_output=True, text=True, timeout=10).stdout
    procs = re.findall(rf"^\s*{pid}\*?\s+(\d+\.\d+)", out, re.M)
    if procs:
        return float(procs[-1])
    return None


def children(pid):
    result = []
    frontier = [pid]
    seen = set()
    while frontier:
        p = frontier.pop()
        if p in seen:
            continue
        seen.add(p)
        out = subprocess.run(["pgrep", "-P", str(p)],
                             capture_output=True, text=True).stdout.split()
        for c in out:
            c = int(c)
            result.append(c)
            frontier.append(c)
    return result


def windows(pid):
    out = subprocess.run([BOUNDS, str(pid)],
                         capture_output=True, text=True).stdout
    wins = []
    for line in out.strip().splitlines():
        try:
            wins.append(json.loads(line))
        except json.JSONDecodeError:
            pass
    return wins


def mascot_visible(pid):
    return any(w["w"] == 64 and w["h"] == 64 for w in windows(pid))


def composer_visible(pid):
    return any(w["w"] == 640 for w in windows(pid))


def wait_until(pred, timeout=10.0, interval=0.005):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        if pred():
            return time.monotonic()
        time.sleep(interval)
    return None


class Driver:
    def __init__(self, binary, manifest):
        self.proc = subprocess.Popen(
            [binary, "--fixture", manifest, "--control"],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=subprocess.PIPE, text=True,
            env={"PATH": "/usr/bin:/bin", "HOME": os.environ.get("HOME", "/tmp"),
                 "LANG": "en_US.UTF-8"})
        self.records = queue.Queue()
        self.all_records = []
        threading.Thread(target=self._reader, daemon=True).start()

    def _reader(self):
        for line in self.proc.stdout:
            line = line.strip()
            if line:
                self.records.put(line)

    def send(self, cmd):
        self.proc.stdin.write(json.dumps(cmd) + "\n")
        self.proc.stdin.flush()

    def wait_reply(self, timeout=15):
        end = time.time() + timeout
        while time.time() < end:
            try:
                line = self.records.get(timeout=0.3)
            except queue.Empty:
                continue
            self.all_records.append(line)
            try:
                rec = json.loads(line)
            except json.JSONDecodeError:
                continue
            if "ok" in rec:
                return rec
        return None

    def wait_terminal(self, timeout=30):
        end = time.time() + timeout
        while time.time() < end:
            try:
                line = self.records.get(timeout=0.3)
            except queue.Empty:
                continue
            self.all_records.append(line)
            try:
                rec = json.loads(line)
            except json.JSONDecodeError:
                continue
            if rec.get("event") == "terminal":
                return rec
        return None


def measure(name, binary, manifest, out_dir):
    results = {"candidate": name, "binary": os.path.abspath(binary),
               "measurement_sources": {
                   "visibility": "CGWindowListCopyWindowInfo bounds helper",
                   "rss": "ps -o rss (bytes)",
                   "physical_footprint": "footprint --noCategories -f bytes",
                   "cpu": "top -l 2 delta sample",
                   "threads": "ps -M line count",
                   "children": "recursive pgrep -P",
                   "clock": "time.monotonic_ns"},
               "runs": []}

    # ---- run 1: launch, activations, idle/composer resources, streaming peak
    d = Driver(binary, manifest)
    pid = d.proc.pid
    run = {"exit_code": None}
    results["runs"].append(run)

    t0 = time.monotonic()
    vis = wait_until(lambda: mascot_visible(pid), timeout=10)
    run["launch_to_visible_ms"] = round((vis - t0) * 1000, 1) if vis else None

    time.sleep(1.0)  # settle to steady idle
    run["idle_mascot"] = {
        "rss_bytes": rss_bytes(pid),
        "physical_footprint_bytes": footprint_bytes(pid),
        "threads": thread_count(pid),
        "cpu_percent": cpu_percent(pid),
        "children": children(pid),
    }

    t0 = time.monotonic()
    d.send({"token": "m1", "command": "show"})
    vis = wait_until(lambda: composer_visible(pid), timeout=10)
    run["first_composer_activation_ms"] = round((vis - t0) * 1000, 1) if vis else None
    d.wait_reply()

    time.sleep(1.0)
    run["composer_open"] = {
        "rss_bytes": rss_bytes(pid),
        "physical_footprint_bytes": footprint_bytes(pid),
        "threads": thread_count(pid),
        "cpu_percent": cpu_percent(pid),
        "children": children(pid),
    }

    # warm activations
    warms = []
    for _ in range(5):
        d.send({"token": "h", "command": "hide"})
        d.wait_reply()
        wait_until(lambda: not composer_visible(pid), timeout=5)
        t0 = time.monotonic()
        d.send({"token": "s", "command": "show"})
        vis = wait_until(lambda: composer_visible(pid), timeout=10)
        if vis:
            warms.append(round((vis - t0) * 1000, 1))
        d.wait_reply()
    run["warm_composer_activation_ms"] = warms

    # streaming peak
    d.send({"token": "sc", "command": "scenario", "name": "normal"})
    d.wait_reply()
    d.send({"token": "tx", "command": "set_text", "text": "measure"})
    d.wait_reply()
    d.send({"token": "sub", "command": "submit"})
    d.wait_reply()
    peak = {"rss": 0, "footprint": 0}
    term = None
    while term is None:
        r = rss_bytes(pid) or 0
        f = footprint_bytes(pid) or 0
        peak["rss"] = max(peak["rss"], r)
        peak["footprint"] = max(peak["footprint"], f)
        term = d.wait_terminal(timeout=5)
    run["streaming_peak"] = {
        "rss_bytes": peak["rss"],
        "physical_footprint_bytes": peak["footprint"],
        "threads": thread_count(pid),
        "children": children(pid),
    }

    # ---- stability: 10x show/hide then 10x submit/cancel
    stability = {"show_hide_failures": 0, "submit_cancel_failures": 0,
                 "rss_before": rss_bytes(pid)}
    for _ in range(10):
        d.send({"token": "h", "command": "hide"})
        if not (d.wait_reply() or {}).get("ok"):
            stability["show_hide_failures"] += 1
        d.send({"token": "s", "command": "show"})
        if not (d.wait_reply() or {}).get("ok"):
            stability["show_hide_failures"] += 1
    for _ in range(10):
        d.send({"token": "sc", "command": "scenario", "name": "cancel"})
        d.wait_reply()
        d.send({"token": "tx", "command": "set_text", "text": "x"})
        d.wait_reply()
        d.send({"token": "sub", "command": "submit"})
        if not (d.wait_reply() or {}).get("ok"):
            stability["submit_cancel_failures"] += 1
            d.wait_terminal(timeout=10)
            continue
        time.sleep(0.15)
        d.send({"token": "cn", "command": "cancel"})
        if not (d.wait_reply() or {}).get("ok"):
            stability["submit_cancel_failures"] += 1
        term = d.wait_terminal(timeout=10)
        if not term or term.get("kind") != "cancelled":
            stability["submit_cancel_failures"] += 1
    stability["rss_after"] = rss_bytes(pid)
    stability["rss_delta_bytes"] = (
        (stability["rss_after"] or 0) - (stability["rss_before"] or 0))
    run["stability"] = stability

    # clean shutdown
    t0 = time.monotonic()
    d.send({"token": "fin", "command": "shutdown"})
    d.wait_reply(timeout=15)
    try:
        d.proc.wait(timeout=15)
        run["clean_shutdown"] = d.proc.returncode == 0
        run["shutdown_ms"] = round((time.monotonic() - t0) * 1000, 1)
    except subprocess.TimeoutExpired:
        d.proc.kill()
        run["clean_shutdown"] = False
    run["exit_code"] = d.proc.returncode
    err = d.proc.stderr.read()
    run["stderr_bytes"] = len(err)

    raw_path = os.path.join(out_dir, "records.ndjson")
    with open(raw_path, "w") as f:
        for line in d.all_records:
            f.write(line + "\n")

    # ---- run 2: second launch latency (median of 3 fresh launches)
    latencies = []
    for _ in range(3):
        d2 = Driver(binary, manifest)
        t0 = time.monotonic()
        vis = wait_until(lambda: mascot_visible(d2.proc.pid), timeout=10)
        if vis:
            latencies.append(round((vis - t0) * 1000, 1))
        d2.send({"token": "fin", "command": "shutdown"})
        d2.wait_reply(timeout=15)
        try:
            d2.proc.wait(timeout=15)
        except subprocess.TimeoutExpired:
            d2.proc.kill()
    run["repeat_launch_to_visible_ms"] = latencies

    with open(os.path.join(out_dir, "measurements.json"), "w") as f:
        json.dump(results, f, indent=2)
    print(json.dumps(results, indent=2))
    return 0


def main():
    if len(sys.argv) != 5:
        print(__doc__)
        return 64
    name, binary, manifest, out_dir = sys.argv[1:5]
    os.makedirs(out_dir, exist_ok=True)
    return measure(name, binary, manifest, out_dir)


if __name__ == "__main__":
    sys.exit(main())
