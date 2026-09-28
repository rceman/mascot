#!/usr/bin/env python3
"""Test-execution timing for the Phase 11 efficiency deliverable.

Measures candidate-side development feedback loops on the same machine,
final correctness-ready source state. Times are wall-clock medians with
ranges; shared harness/provider overhead is included but the fixture is
identical for all candidates. Does NOT include the long runtime benchmark.
"""
import json
import pathlib
import subprocess
import sys
import time

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
RAW = ROOT / "benchmark" / "results" / "windows" / "raw"
MANIFEST = ROOT / "benchmark" / "manifest" / "fixture.json"
CODEX = (r"C:\Users\therceman\AppData\Roaming\npm\node_modules\@openai\codex"
         r"\vendor\x86_64-pc-windows-msvc\codex\codex.exe")
ZIG = r"W:\devin_folder\tools\zig-x86_64-windows-0.15.2\zig.exe"

CANDS = {
    "rust": {
        "exe": ROOT / "out" / "rust" / "mascot.exe",
        "dir": ROOT / "rust",
        "unit": ["cargo", "test", "--manifest-path", "rust/Cargo.toml"],
        "incremental_cmd": ["cargo", "build", "--release",
                            "--manifest-path", "rust/Cargo.toml"],
        "touch": ROOT / "rust" / "src" / "text.rs",
    },
    "zig": {
        "exe": ROOT / "out" / "zig" / "mascot.exe",
        "dir": ROOT / "zig",
        "unit": [ZIG, "build", "test"],
        "incremental_cmd": [ZIG, "build", "-Doptimize=ReleaseSafe"],
        "touch": ROOT / "zig" / "src" / "text.zig",
    },
    "go": {
        "exe": ROOT / "out" / "go" / "mascot.exe",
        "dir": ROOT / "go",
        "unit": ["go", "test", "./..."],
        "incremental_cmd": ["go", "build", "-trimpath", "-buildvcs=false",
                            "-ldflags", "-H windowsgui -s -w -buildid=",
                            "-o", "bin/mascot.exe", "."],
        "touch": ROOT / "go" / "text.go",
    },
}


def timed(cmd, cwd=None):
    t = time.perf_counter()
    proc = subprocess.run(cmd, cwd=cwd, capture_output=True,
                          text=True, timeout=1800)
    return (time.perf_counter() - t) * 1000.0, proc.returncode


def run_series(name, fn, reps):
    times, codes = [], []
    for i in range(reps):
        ms, code = fn()
        times.append(round(ms, 1))
        codes.append(code)
        print(f"    {name} rep{i}: {ms:.0f} ms (exit {code})", flush=True)
    times_sorted = sorted(times)
    med = times_sorted[len(times) // 2] if len(times) % 2 else \
        (times_sorted[len(times) // 2 - 1] + times_sorted[len(times) // 2]) / 2
    return {"n": reps, "times_ms": times, "median_ms": round(med, 1),
            "min_ms": min(times), "max_ms": max(times),
            "exit_codes": codes, "all_pass": all(c == 0 for c in codes)}


def main():
    which = sys.argv[1] if len(sys.argv) > 1 else "all"
    out_dir = RAW / "test-timing-001"
    out_dir.mkdir(parents=True, exist_ok=True)
    results = {"schema": "mascot-test-timing-1",
               "utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
               "host": "windows benchmark host, Balanced power plan",
               "fixture": "windows-v1.0.2", "per_candidate": {}}

    for cand, c in CANDS.items():
        print(f"=== {cand} ===", flush=True)
        r = {}
        exe = c["exe"]
        # 1. no-op incremental build x5
        r["incremental_noop_build"] = run_series(
            "noop", lambda: timed(c["incremental_cmd"], cwd=c["dir"]), 5)
        # 2. one-file-change incremental build x3
        def one_file():
            c["touch"].touch()
            return timed(c["incremental_cmd"], cwd=c["dir"])
        r["incremental_one_file_build"] = run_series("onefile", one_file, 3)
        # 3. unit/self tests x5
        r["unit_tests"] = run_series("unit",
                                     lambda: timed(c["unit"], cwd=c["dir"]), 5)
        # 4. provider regression x5
        i = [0]
        def provider():
            i[0] += 1
            return timed([sys.executable,
                          str(ROOT / "benchmark/harness/"
                              "verify_provider_regression.py"),
                          "--exe", str(exe), "--manifest", str(MANIFEST),
                          "--output", str(out_dir / f"{cand}-provtime-{i[0]}"),
                          "--candidate", cand], cwd=ROOT)
        r["provider_regression"] = run_series("provider", provider, 5)
        # 5. smoke x5
        i[0] = 0
        def smoke():
            i[0] += 1
            return timed([sys.executable,
                          str(ROOT / "benchmark/harness/"
                              "verify_candidate_smoke.py"),
                          "--exe", str(exe), "--manifest", str(MANIFEST),
                          "--output", str(out_dir / f"{cand}-smoketime-{i[0]}")],
                         cwd=ROOT)
        r["smoke"] = run_series("smoke", smoke, 5)
        # 6. acceptance x3 (UI suite, virtual display; W2/W7 untested w/o lab)
        i[0] = 0
        def accept():
            i[0] += 1
            return timed([sys.executable,
                          str(ROOT / "benchmark/harness/"
                              "verify_acceptance.py"),
                          "--exe", str(exe), "--manifest", str(MANIFEST),
                          "--output", str(out_dir / f"{cand}-acctime-{i[0]}"),
                          "--candidate", cand, "--launch-point", "4352", "384"],
                         cwd=ROOT)
        r["acceptance"] = run_series("acceptance", accept, 3)
        # 7. codex gate x5
        def gate():
            return timed([str(exe), "--codex-gate", CODEX], cwd=ROOT)
        r["codex_gate"] = run_series("codex-gate", gate, 5)
        # 8. edit -> incremental build -> targeted unit test loop x3
        def loop():
            c["touch"].touch()
            ms1, code1 = timed(c["incremental_cmd"], cwd=c["dir"])
            ms2, code2 = timed(c["unit"], cwd=c["dir"])
            return ms1 + ms2, code1 | code2
        r["edit_build_test_loop"] = run_series("loop", loop, 3)
        results["per_candidate"][cand] = r
        (out_dir / f"{cand}-timing.json").write_text(
            json.dumps(r, indent=2) + "\n", encoding="utf-8")
        print(f"--- {cand} done ---", flush=True)

    (out_dir / "result.json").write_text(
        json.dumps(results, indent=2) + "\n", encoding="utf-8")
    print("wrote", out_dir / "result.json")


if __name__ == "__main__":
    main()
