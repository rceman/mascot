import argparse
import ctypes
import json
import os
import pathlib
import subprocess
import sys
from datetime import datetime, timezone


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=pathlib.Path, required=True)
    parser.add_argument("--cwd", type=pathlib.Path, required=True)
    parser.add_argument("--env", action="append", default=[])
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    if os.name != "nt":
        parser.error("Native Windows execution is required.")
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    if not command or not args.cwd.is_dir():
        parser.error("A command and existing working directory are required.")
    overrides = {}
    allowed = {"RUSTFLAGS", "CARGO_TARGET_DIR", "GOTOOLCHAIN", "CGO_ENABLED", "GOCACHE", "GOMODCACHE"}
    for item in args.env:
        name, separator, value = item.partition("=")
        if not separator or name not in allowed:
            parser.error("Only the explicitly supported non-secret build controls may be recorded.")
        overrides[name] = value
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    for name in ("QueryPerformanceCounter", "QueryPerformanceFrequency"):
        function = getattr(kernel, name)
        function.argtypes = [ctypes.POINTER(ctypes.c_int64)]
        function.restype = ctypes.c_int

    def query(name):
        value = ctypes.c_int64()
        if not getattr(kernel, name)(ctypes.byref(value)) or value.value <= 0:
            raise ctypes.WinError(ctypes.get_last_error())
        return value.value

    frequency = query("QueryPerformanceFrequency")
    args.output.mkdir(exist_ok=False)
    record = {
        "schema": "mascot-native-command-record-1",
        "scope": "Build/debug command evidence; not candidate runtime benchmark data",
        "utc_started": datetime.now(timezone.utc).isoformat(),
        "argv": command,
        "cwd": str(args.cwd.resolve()),
        "environment_overrides": overrides,
        "clock": "Windows QueryPerformanceCounter",
        "qpc_frequency": frequency,
        "exit_code": None,
        "launch_error": None,
        "stdout": "stdout.log",
        "stderr": "stderr.log",
    }
    environment = os.environ.copy()
    environment.update(overrides)
    with (args.output / "stdout.log").open("xb") as stdout, (args.output / "stderr.log").open("xb") as stderr:
        start = query("QueryPerformanceCounter")
        try:
            result = subprocess.run(command, cwd=args.cwd, env=environment, stdout=stdout, stderr=stderr, check=False)
            record["exit_code"] = result.returncode
        except OSError as error:
            record["launch_error"] = str(error)
        end = query("QueryPerformanceCounter")
    record.update(qpc_start=str(start), qpc_end=str(end), elapsed_ms=(end - start) * 1000 / frequency,
                  utc_finished=datetime.now(timezone.utc).isoformat())
    (args.output / "command.json").write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8", newline="\n")
    print(json.dumps({"record": str(args.output / "command.json"), "exit_code": record["exit_code"]}))
    return record["exit_code"] if record["exit_code"] is not None else 1


if __name__ == "__main__":
    sys.exit(main())
