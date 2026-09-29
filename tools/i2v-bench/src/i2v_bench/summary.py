import json
from pathlib import Path

from .util import write_json

COLUMNS = [
    ("profile", "model/checkpoint"),
    ("backend", "backend"),
    ("quantization", "quant"),
    ("resolution", "resolution"),
    ("frames_fps", "frames/fps"),
    ("steps", "steps"),
    ("peak_vram_mb", "peak VRAM MB"),
    ("generation_s", "gen s"),
    ("model_load_s", "load s"),
    ("completed", "completed"),
    ("orientation_drift", "orientation drift"),
    ("non_target_motion", "non-target motion"),
    ("identity_notes", "identity notes"),
    ("local_motion_notes", "local-motion notes"),
]


def _row(r: dict) -> dict:
    res = r.get("resources", {})
    tim = r.get("timing_s", {})
    review = r.get("visual_review", {})
    return {
        "profile": r.get("profile"),
        "run_id": r.get("run_id"),
        "mode": r.get("mode"),
        "model_id": r.get("model", {}).get("id"),
        "backend": r.get("model", {}).get("backend"),
        "quantization": r.get("model", {}).get("quantization"),
        "resolution": f"{r.get('resolution', {}).get('width')}x{r.get('resolution', {}).get('height')}",
        "resolution_comparable": r.get("resolution", {}).get("cross_model_resolution_comparable"),
        "frames_fps": f"{r.get('frames')}/{r.get('output_fps')}",
        "steps": r.get("steps"),
        "peak_vram_mb": res.get("peak_vram_mb"),
        "baseline_vram_mb": res.get("baseline_vram_mb"),
        "torch_allocator_peak_mb": r.get("torch_allocator_peak_mb"),
        "generation_s": tim.get("generation"),
        "model_load_s": tim.get("model_load"),
        "total_wall_s": tim.get("total_wall"),
        "completed": r.get("success"),
        "scored_articulation_comparison": r.get("scored_articulation_comparison"),
        "deviations": r.get("deviations", []),
        "orientation_drift": review.get("orientation_drift"),
        "non_target_motion": review.get("non_target_motion"),
        "identity_notes": review.get("identity_notes"),
        "local_motion_notes": review.get("local_motion_notes"),
        "error": r.get("error"),
    }


def collect_runs(results_root: Path) -> list[dict]:
    runs = []
    for rj in sorted(results_root.rglob("run.json")):
        try:
            runs.append(json.loads(rj.read_text(encoding="utf-8")))
        except Exception:
            continue
    return runs


def write_summary(results_root: Path) -> tuple[Path, Path]:
    runs = collect_runs(results_root)
    rows = [_row(r) for r in runs]
    summary = {
        "schema_version": "mascot-i2v-summary/1",
        "results_root": str(results_root),
        "run_count": len(rows),
        "runs": rows,
        "note": "Measured facts only; one mascot prompt does not establish a universal model ranking.",
    }
    js = results_root / "summary.json"
    write_json(js, summary)

    lines = ["# Local I2V benchmark v0.1 — summary", "",
             "Measured facts; not a universal model ranking.", ""]
    header = ["profile", "mode", "resolution", "frames/fps", "steps",
              "peak VRAM MB", "gen s", "wall s", "completed",
              "orientation drift", "non-target motion", "identity", "local motion"]
    lines.append("| " + " | ".join(header) + " |")
    lines.append("|" + "---|" * len(header))
    for r in rows:
        lines.append("| " + " | ".join(str(x) for x in [
            r["profile"], r["mode"], r["resolution"], r["frames_fps"], r["steps"],
            r["peak_vram_mb"], r["generation_s"], r["total_wall_s"], r["completed"],
            r["orientation_drift"], r["non_target_motion"],
            r["identity_notes"], r["local_motion_notes"],
        ]) + " |")
    lines.append("")
    for r in rows:
        if r["deviations"]:
            lines.append(f"- `{r['run_id']}` deviations: {'; '.join(r['deviations'])}")
        if r["error"]:
            lines.append(f"- `{r['run_id']}` FAILED: {r['error']}")
    md = results_root / "summary.md"
    md.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return js, md
