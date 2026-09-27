#!/usr/bin/env python3
"""Untimed Codex app-server compatibility gate (benchmark protocol §16).

Drives the candidate's --codex-gate mode against a pinned codex.exe,
records per-step evidence, and asserts the required lifecycle:
spawn -> initialize -> read-only streamed interaction -> clean teardown.
Model/network latency is not compared.
"""

import argparse
import hashlib
import json
import pathlib
import re
import subprocess
import sys
from datetime import datetime, timezone

REQUIRED_STEPS = ("spawn", "reply", "interaction", "teardown")


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def codex_version(exe):
    out = subprocess.run([str(exe), "--version"], capture_output=True,
                         text=True, timeout=30)
    return out.stdout.strip()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--exe", required=True, type=pathlib.Path)
    parser.add_argument("--codex", required=True, type=pathlib.Path)
    parser.add_argument("--output", required=True, type=pathlib.Path)
    parser.add_argument("--candidate", required=True, choices=["rust", "zig", "go"])
    args = parser.parse_args()
    assert not args.output.exists(), f"refuse to overwrite {args.output}"
    args.output.mkdir(parents=True)

    version = codex_version(args.codex)
    pin = {"path": str(args.codex), "sha256": sha256(args.codex),
           "version": version}

    log_path = args.output / "gate.stdout.ndjson"
    proc = subprocess.run(
        [str(args.exe), "--codex-gate", str(args.codex)],
        capture_output=True, text=True, timeout=120, encoding="utf-8",
        errors="replace")
    log_path.write_text(proc.stdout, encoding="utf-8", newline="\n")
    (args.output / "gate.stderr.log").write_text(proc.stderr, encoding="utf-8")

    events = []
    for line in proc.stdout.splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            events.append(json.loads(line))
        except json.JSONDecodeError:
            pass
    steps = [e.get("step") for e in events if e.get("event") == "codex_gate"]
    errors = []
    for required in REQUIRED_STEPS:
        if required not in steps:
            errors.append(f"missing step {required}")
    replies = [s for s in steps if s == "reply"]
    if len(replies) < 3:
        errors.append(f"expected 3 JSON-RPC replies, saw {len(replies)}")
    if "exited after stdin close" not in " ".join(
            str(e.get("detail", "")) for e in events
            if e.get("step") == "teardown") and proc.returncode != 0:
        errors.append("unclean teardown")
    final = next((e for e in reversed(events)
                  if e.get("step") == "result"), None)
    status = "PASS" if (proc.returncode == 0 and final
                        and final.get("detail") == "PASS"
                        and not errors) else "FAIL"
    result = {
        "schema": "mascot-codex-gate-1",
        "candidate": args.candidate,
        "codex": pin,
        "status": status,
        "errors": errors,
        "events": events,
        "exit_code": proc.returncode,
        "utc": datetime.now(timezone.utc).isoformat(),
        "scope": "untimed; model/network latency not measured or compared",
    }
    (args.output / "result.json").write_text(
        json.dumps(result, indent=2) + "\n", encoding="utf-8", newline="\n")
    print(json.dumps({"status": status, "errors": errors}))
    return 0 if status == "PASS" else 1


if __name__ == "__main__":
    sys.exit(main())
