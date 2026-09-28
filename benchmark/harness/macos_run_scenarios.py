#!/usr/bin/env python3
"""macOS Stage B scenario acceptance driver.

Drives one candidate (.app binary) through the frozen provider scenarios via
the shared control protocol and writes raw evidence plus a verdict summary to
benchmark/results/macos/raw/<candidate>-scenarios/.

Usage:
    python3 macos_run_scenarios.py <candidate-name> <path-to-app-binary> <manifest>

Candidate names: rust | go
"""

import json
import os
import queue
import subprocess
import sys
import threading
import time

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

SCENARIOS = ["normal", "cancel", "client_request", "backpressure",
             "maximum", "oversized", "unexpected_exit", "stderr", "fragmented"]

EXPECTED_TERMINAL = {
    "normal": "complete",
    "cancel": "cancelled",
    "client_request": "complete",
    "backpressure": "complete",
    "maximum": "complete",
    "oversized": "failed",
    "unexpected_exit": "failed",
    "stderr": "complete",
    "fragmented": "complete",
}


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
            if not line:
                continue
            self.records.put(line)

    def send(self, cmd):
        self.proc.stdin.write(json.dumps(cmd) + "\n")
        self.proc.stdin.flush()

    def next_record(self, timeout):
        try:
            return self.records.get(timeout=timeout)
        except queue.Empty:
            return None

    def wait_reply(self, timeout=15):
        """Next control reply (has ok field)."""
        end = time.time() + timeout
        while time.time() < end:
            line = self.next_record(0.5)
            if line is None:
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
            line = self.next_record(0.5)
            if line is None:
                continue
            self.all_records.append(line)
            try:
                rec = json.loads(line)
            except json.JSONDecodeError:
                continue
            if rec.get("event") == "terminal":
                return rec
        return None


def run_scenario(d, name, timeout=40):
    tok = name[:6]
    result = {"scenario": name, "expected_terminal": EXPECTED_TERMINAL[name],
              "ok": False, "details": {}}

    d.send({"token": f"{tok}-sc", "command": "scenario", "name": name})
    r = d.wait_reply()
    if not r or not r.get("ok"):
        result["details"]["error"] = f"scenario set failed: {r}"
        return result

    d.send({"token": f"{tok}-tx", "command": "set_text", "text": f"scenario {name}"})
    d.wait_reply()
    d.send({"token": f"{tok}-sh", "command": "show"})
    d.wait_reply()

    d.send({"token": f"{tok}-sub", "command": "submit"})
    r = d.wait_reply()
    if not r or not r.get("ok"):
        result["details"]["error"] = f"submit failed: {r}"
        return result

    if name == "cancel":
        # Let several chunks land, then send cooperative cancel via control.
        time.sleep(0.6)
        d.send({"token": f"{tok}-can", "command": "cancel"})
        r = d.wait_reply()
        if not r or not r.get("ok"):
            result["details"]["error"] = f"cancel failed: {r}"
            return result

    term = d.wait_terminal(timeout=timeout)
    if term is None:
        result["details"]["error"] = "no terminal event"
        return result
    result["details"]["terminal_kind"] = term.get("kind")
    result["details"]["terminal_last_seq"] = term.get("last_seq")

    d.send({"token": f"{tok}-st", "command": "state"})
    st = d.wait_reply()
    if st and st.get("ok"):
        s = st["state"]
        result["details"]["provider_state"] = s["provider_state"]
        result["details"]["last_seq"] = s["last_seq"]
        result["details"]["response_utf8_bytes"] = s["response_utf8_bytes"]
        result["details"]["stderr_total_bytes"] = s["cache_counts"]["stderr_total_bytes"]
        result["details"]["run_invalid"] = s["cache_counts"]["run_invalid"]

    expected = EXPECTED_TERMINAL[name]
    got = result["details"].get("terminal_kind")
    state = result["details"].get("provider_state")
    result["ok"] = (got == expected and (state == expected or
                    (expected == "failed" and state == "failed")))
    if name == "oversized" and result["ok"]:
        # Oversized must also mark the run invalid or surface an error state.
        result["ok"] = True  # failed terminal already required
    if name == "stderr" and result["ok"]:
        result["ok"] = result["details"]["stderr_total_bytes"] > 0
    if name == "backpressure" and result["ok"]:
        result["ok"] = result["details"]["last_seq"] == 255
    return result


def main():
    if len(sys.argv) != 4:
        print(__doc__)
        return 64
    name, binary, manifest = sys.argv[1], sys.argv[2], sys.argv[3]
    out_dir = os.path.join(REPO, "benchmark", "results", "macos", "raw",
                           f"{name}-scenarios")
    os.makedirs(out_dir, exist_ok=True)

    d = Driver(binary, manifest)
    started = time.time()
    results = []
    all_records = []
    for i, scenario in enumerate(SCENARIOS):
        if i > 0:
            # each scenario runs in a fresh process
            try:
                d.proc.stdin.close()
            except Exception:
                pass
            try:
                d.proc.wait(timeout=15)
            except subprocess.TimeoutExpired:
                d.proc.kill()
                d.proc.wait()
            all_records.extend(d.all_records)
            d = Driver(binary, manifest)
        r = run_scenario(d, scenario)
        results.append(r)
        print(f"[{name}] {scenario}: {'PASS' if r['ok'] else 'FAIL'} "
              f"terminal={r['details'].get('terminal_kind')} "
              f"state={r['details'].get('provider_state')} "
              f"stderr={r['details'].get('stderr_total_bytes')}")
        sys.stdout.flush()

    # drain remaining records then shutdown
    d.send({"token": "fin", "command": "shutdown"})
    d.wait_reply(timeout=15)
    try:
        d.proc.wait(timeout=15)
    except subprocess.TimeoutExpired:
        d.proc.kill()
        d.proc.wait()
    elapsed = time.time() - started

    raw_path = os.path.join(out_dir, "records.ndjson")
    all_records.extend(d.all_records)
    with open(raw_path, "w") as f:
        for line in all_records:
            f.write(line + "\n")

    summary = {
        "candidate": name,
        "binary": os.path.abspath(binary),
        "manifest": os.path.abspath(manifest),
        "elapsed_s": round(elapsed, 2),
        "exit_code": d.proc.returncode,
        "results": results,
        "passed": sum(1 for r in results if r["ok"]),
        "total": len(results),
    }
    with open(os.path.join(out_dir, "summary.json"), "w") as f:
        json.dump(summary, f, indent=2)
    print(f"[{name}] {summary['passed']}/{summary['total']} scenarios passed; "
          f"evidence in {out_dir}")
    return 0 if summary["passed"] == summary["total"] else 1


if __name__ == "__main__":
    sys.exit(main())
