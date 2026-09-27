"""Aggregate per-candidate evidence into RESULTS.json + report tables.

Reads every committed raw evidence directory under
benchmark/results/windows/raw/ and emits:

- benchmark/results/windows/RESULTS.json   (machine-readable summary)
- docs/BENCHMARK_RESULTS.md                (factual comparison report skeleton)

The report never declares a cross-platform winner: it reports Windows
Stage A evidence only.
"""
import json
import pathlib
import statistics
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
RAW = ROOT / "results" / "windows" / "raw"
RESULTS = ROOT / "results" / "windows" / "RESULTS.json"
REPORT = ROOT.parent / "docs" / "BENCHMARK_RESULTS.md"

CASES_T = ["T1", "T2", "T3", "T4", "T5", "T6", "T7", "T8", "T9", "T10",
           "T11", "T12", "T13", "T14"]
CASES_W = ["W1", "W2", "W3", "W4", "W5", "W6", "W7", "W8", "W9"]
CASES_P = [f"P{i}" for i in range(1, 13)]


def read_json(path):
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return None


def latest(raw, pattern):
    dirs = sorted(d for d in raw.glob(pattern) if d.is_dir()
                  and "quick" not in d.name)
    for d in reversed(dirs):
        r = read_json(d / "result.json")
        if r is not None:
            return r, d.name
    return None, None


def latest_benchmark(raw, name):
    """Benchmark results live in benchmark-*/<name>/result.json (orchestrated)
    or <name>-benchmark-*/result.json (single-candidate runs)."""
    candidates = []
    for batch in sorted(d for d in raw.glob("benchmark-*") if d.is_dir()):
        r = read_json(batch / name / "result.json")
        if r is not None:
            candidates.append((r, f"{batch.name}/{name}"))
    for d in sorted(raw.glob(f"{name}-benchmark-*")):
        if "quick" in d.name:
            continue
        r = read_json(d / "result.json")
        if r is not None:
            candidates.append((r, d.name))
    return candidates[-1] if candidates else (None, None)


def acceptance_summary(result):
    cases = {c["case"]: c["status"] for c in result.get("cases", [])}
    merged = {}
    for cid, status in cases.items():
        base = cid.split("b")[0].split("-")[0]
        order = {"UNTESTED": 0, "FAIL": 1, "PASS": 2}
        if base not in merged or order[status] > order[merged[base]]:
            merged[base] = status
    return merged


def provider_summary(result):
    cases = {c.get("case") or c.get("check"): c.get("status")
             for c in result.get("cases", result.get("checks", []))}
    return cases


def summarize_benchmark(result):
    if result is None:
        return None
    fresh = [x["startup_ms"] for x in result.get("fresh_launches", [])
             if "startup_ms" in x]
    warm_acts = result.get("warm_activations", {})
    warm = [x["hotkey_to_visible_ms"] for x in warm_acts.get("warm", [])]
    firsts = [x["hotkey_to_input_ready_ms"]
              for x in warm_acts.get("first_activations", [])]
    states = result.get("resource_states", {})
    r0 = states.get("R0_fresh_mascot", {}).get("samples") or []
    r3 = states.get("R3_warm_mascot", {})
    r4 = states.get("R4_streaming", {})
    r5 = states.get("R5_cancellation", {})

    def med(rows, key):
        vals = [r[key] for r in rows if r.get(key) is not None]
        return statistics.median(vals) if vals else None

    def pctl(vals, p):
        if not vals:
            return None
        vals = sorted(vals)
        return vals[min(len(vals) - 1, int(len(vals) * p / 100))]

    # Stability: private working set at op 0 vs op 100 per batch — a rising
    # floor across batches indicates leak; report the worst per-operation
    # start->end growth.
    stability_trend = {}
    for b in (result.get("stability") or {}).get("batches", []):
        op = b.get("operation")
        samples = [s for s in b.get("samples", [])
                   if s.get("private_working_set_bytes") is not None]
        if not samples:
            continue
        delta = (samples[-1]["private_working_set_bytes"] -
                 samples[0]["private_working_set_bytes"])
        cur = stability_trend.get(op)
        if cur is None or delta > cur:
            stability_trend[op] = delta

    return {
        "fresh_startup_median_ms": statistics.median(fresh) if fresh else None,
        "fresh_startup_range_ms": [min(fresh), max(fresh)] if fresh else None,
        "fresh_startup_samples": fresh,
        "first_activation_ms": warm_acts.get("first", {}).get(
            "hotkey_to_input_ready_ms"),
        "first_activation_median_ms": statistics.median(firsts) if firsts else None,
        "first_activation_range_ms": [min(firsts), max(firsts)] if firsts else None,
        "warm_activation_median_ms": statistics.median(warm) if warm else None,
        "warm_activation_p95_ms": pctl(warm, 95),
        "warm_activation_n": len(warm),
        "warm_activation_samples": warm,
        "r0_private_ws_median": med(r0, "private_working_set_bytes"),
        "r0_private_commit_median": med(r0, "private_commit_bytes"),
        "r0_idle_cpu_one_core_pct": states.get("R0_fresh_mascot", {}).get(
            "idle_cpu_one_core_pct"),
        "r3_warm_pws": med(r3.get("steady") or [],
                           "private_working_set_bytes"),
        "r4_observed_max_pws": r4.get("observed_max_pws"),
        "r4_observed_max_commit": r4.get("observed_max_commit"),
        "r4_chunks_received": r4.get("chunks_received"),
        "r4_transport_ms_median": r4.get("transport_ms_median"),
        "r4_transport_ms_p95": r4.get("transport_ms_p95"),
        "r4_submit_to_first_visible_ms": r4.get("submit_to_first_visible_ms"),
        "r5_pws_at_cancel": (r5.get("peak_at_cancel") or {}).get(
            "private_working_set_bytes"),
        "r5_pws_after_60s": ((r5.get("post") or {}).get("60s") or {}).get(
            "private_working_set_bytes"),
        "r6_paints_delta": states.get("R6_focused_idle", {}).get(
            "composer_paints_delta"),
        "r6_presents_delta": states.get("R6_focused_idle", {}).get(
            "mascot_presents_delta"),
        "stability_pws_growth_by_op": stability_trend,
        "post_reboot": (result.get("post_reboot_first_launches") or {})
            .get("status"),
    }


def main():
    results = {"schema": "mascot-results-1",
               "fixture_version": json.loads(
                   (pathlib.Path(__file__).parent.parent / "manifest" /
                    "fixture.json").read_text(encoding="utf-8"))["version"],
               "stage": "windows-a", "candidates": {}}
    report_rows = {}
    for name in ("rust", "zig", "go"):
        cand = {}
        acc, acc_dir = latest(RAW, f"{name}-acceptance-*")
        if acc:
            cand["acceptance"] = {"dir": acc_dir, "status": acc["status"],
                                  "cases": acceptance_summary(acc)}
        prov, prov_dir = latest(RAW, f"{name}-provider-*")
        if prov:
            cand["provider_regression"] = {"dir": prov_dir,
                                           "status": prov.get("status")}
        smoke, smoke_dir = latest(RAW, f"{name}-smoke-*")
        if smoke:
            cand["smoke"] = {"dir": smoke_dir, "status": smoke.get("status")}
        bench, bench_dir = latest_benchmark(RAW, name)
        if bench:
            cand["benchmark"] = {"dir": bench_dir,
                                 "summary": summarize_benchmark(bench)}
        gate, gate_dir = latest(RAW, f"{name}-codex-gate-*")
        if gate:
            cand["codex_gate"] = {"dir": gate_dir, "status": gate.get("status"),
                                  "codex_version": (gate.get("codex") or {})
                                      .get("version")}
        eff = read_json(RAW / f"{name}-source-efficiency.json")
        if eff:
            f = eff.get("final", {})
            cand["source_efficiency"] = {
                "source_tokens_o200k": f.get("source_tokens_o200k_base"),
                "loc": f.get("nonblank_noncomment_loc"),
                "files": f.get("files"),
                "first_complete_tokens": (eff.get("first_complete") or {})
                    .get("source_tokens_o200k_base"),
                "churn_added": (eff.get("correction_churn") or {})
                    .get("tokens_added"),
                "churn_removed": (eff.get("correction_churn") or {})
                    .get("tokens_removed")}
        results["candidates"][name] = cand
        report_rows[name] = cand

    RESULTS.parent.mkdir(parents=True, exist_ok=True)
    RESULTS.write_text(json.dumps(results, indent=2, ensure_ascii=False) + "\n",
                       encoding="utf-8", newline="\n")

    lines = ["# Benchmark Results — Windows Stage A",
             "",
             f"Fixture: `{results['fixture_version']}` (commit-frozen). "
             "All evidence under `benchmark/results/windows/raw/`.",
             "",
             "## Correctness eligibility",
             "",
             "| Case | Rust | Zig | Go |", "|---|---|---|---|"]
    for cid in CASES_T + CASES_W:
        row = [cid]
        for name in ("rust", "zig", "go"):
            cell = report_rows[name].get("acceptance", {}) \
                .get("cases", {}).get(cid, "-")
            row.append(cell)
        lines.append("| " + " | ".join(row) + " |")
    lines += ["",
              "Provider/lifecycle (regression status per candidate):",
              "",
              "| Candidate | Provider regression | Smoke | Codex gate |",
              "|---|---|---|---|"]
    for name in ("rust", "zig", "go"):
        r = report_rows[name]
        lines.append("| {} | {} | {} | {} |".format(
            name,
            r.get("provider_regression", {}).get("status", "-"),
            r.get("smoke", {}).get("status", "-"),
            r.get("codex_gate", {}).get("status", "-")))
    lines += ["", "## Source/token efficiency (tiktoken/o200k_base)",
              "",
              "| Metric | Rust | Zig | Go |", "|---|---:|---:|---:|"]
    for key, label in (("source_tokens_o200k", "Final source tokens"),
                       ("loc", "Nonblank/noncomment LOC"),
                       ("files", "Source files"),
                       ("first_complete_tokens", "First-complete tokens"),
                       ("churn_added", "Correction tokens added"),
                       ("churn_removed", "Correction tokens removed")):
        row = [label]
        for name in ("rust", "zig", "go"):
            v = report_rows[name].get("source_efficiency", {}).get(key)
            row.append(str(v) if v is not None else "-")
        lines.append("| " + " | ".join(row) + " |")
    lines += ["", "## Benchmark metrics (see raw dirs for distributions)",
              "",
              "| Metric | Rust | Zig | Go |", "|---|---:|---:|---:|"]
    keys = [("fresh_startup_median_ms", "Fresh startup median ms"),
            ("first_activation_median_ms", "First activation median ms"),
            ("warm_activation_median_ms", "Warm activation median ms"),
            ("warm_activation_p95_ms", "Warm activation p95 ms"),
            ("r0_private_ws_median", "Fresh mascot PWS (bytes)"),
            ("r3_warm_pws", "Warm mascot PWS (bytes)"),
            ("r4_observed_max_pws", "Streaming observed peak PWS"),
            ("r4_transport_ms_median", "Chunk transport median ms"),
            ("r4_submit_to_first_visible_ms", "Submit->first visible ms"),
            ("r0_idle_cpu_one_core_pct", "Idle CPU % of one core"),
            ("r6_paints_delta", "R6 composer paints delta")]
    stability_keys = [("stability_pws_growth_by_op", "Stability PWS growth op->bytes")]
    for key, label in keys:
        row = [label]
        for name in ("rust", "zig", "go"):
            v = (report_rows[name].get("benchmark") or {}).get(
                "summary", {}).get(key)
            row.append(str(round(v, 2)) if isinstance(v, (int, float))
                       else ("-" if v is None else str(v)))
        lines.append("| " + " | ".join(row) + " |")
    for key, label in stability_keys:
        row = [label]
        for name in ("rust", "zig", "go"):
            v = (report_rows[name].get("benchmark") or {}).get(
                "summary", {}).get(key)
            row.append(str(v) if v is not None else "-")
        lines.append("| " + " | ".join(row) + " |")
    lines += ["",
              "Post-reboot first launches are **UNTESTED** — they require "
              "coordinated genuine boots and no automatic reboot was "
              "performed.",
              "",
              "Raw per-run samples are in each candidate's result.json; "
              "this report deliberately avoids a composite score and does not "
              "declare a cross-platform language winner."]
    REPORT.write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    print(f"wrote {RESULTS} and {REPORT}")


if __name__ == "__main__":
    main()
