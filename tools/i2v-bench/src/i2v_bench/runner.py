import time
import traceback
from pathlib import Path

from PIL import Image

from . import backends, evidence
from .backends.common import torch_allocator_peak_mb
from .config import REPO_ROOT
from .env import env_identity
from .monitor import ResourceMonitor
from .util import git_head, run_id_now, sha256_file, sha256_text, utcnow, write_json

SCHEMA = "mascot-i2v-run/1"


def _deviations(profile: dict, mode_cfg: dict, frames: int, fps: int, seed: int) -> list[str]:
    dev = []
    if [profile["width"], profile["height"]] != [480, 480]:
        dev.append(f"resolution {profile['width']}x{profile['height']} != canonical 480x480")
    if fps != 16:
        dev.append(f"fps {fps} != benchmark target 16 (model native rate)")
    if frames != 33:
        dev.append(f"frames {frames} != canonical 33")
    if seed != 42:
        dev.append(f"seed {seed} != canonical 42")
    return dev


def run(profile: dict, mode: str, input_path: Path, prompt_path: Path,
        negative_path: Path | None, out_root: Path,
        seed: int | None = None, frames: int | None = None,
        scored_articulation: bool = False) -> Path:

    mode_cfg = profile["modes"][mode]
    seed = profile.get("seed", 42) if seed is None else seed
    frames = profile.get("frames", 33) if frames is None else frames
    fps = profile.get("fps", 16)

    run_dir = out_root / profile["dir_name"] / f"{run_id_now()}-{mode}"
    run_dir.mkdir(parents=True, exist_ok=True)
    frames_dir = run_dir / "frames"

    prompt = prompt_path.read_text(encoding="utf-8").strip()
    negative = negative_path.read_text(encoding="utf-8").strip() if negative_path else None

    receipt = {
        "schema_version": SCHEMA,
        "run_id": run_dir.name,
        "profile": profile["name"],
        "mode": mode,
        "scored_articulation_comparison": scored_articulation,
        "timestamp_utc": utcnow(),
        "git_commit": git_head(REPO_ROOT),
        "env": env_identity(),
        "model": {
            "id": profile["hf_repo"],
            "revision": profile.get("hf_revision", "main"),
            "backend": profile["backend"],
            "dtype": profile.get("dtype", "bf16"),
            "quantization": profile.get("quantization", "none"),
        },
        "input": {
            "path": str(input_path),
            "sha256": sha256_file(input_path),
        },
        "resolution": {
            "width": profile["width"], "height": profile["height"],
            "mode": profile.get("resolution_mode", "comparable_480p"),
            "cross_model_resolution_comparable": profile.get("resolution_mode", "comparable_480p") == "comparable_480p",
        },
        "frames": frames,
        "output_fps": fps,
        "seed": seed,
        "steps": mode_cfg["steps"],
        "guidance": mode_cfg.get("guidance"),
        "offload_strategy": profile.get("offload", "model"),
        "prompt": {"path": str(prompt_path), "sha256": sha256_text(prompt)},
        "negative_prompt": (
            {"path": str(negative_path), "sha256": sha256_text(negative)}
            if negative is not None else None
        ),
        "deviations": _deviations(profile, mode_cfg, frames, fps, seed),
        "success": False,
    }

    monitor = ResourceMonitor().start()
    t_total_start = time.perf_counter()
    t_prompt = t_load = t_gen = t_encode = 0.0
    try:
        image = Image.open(input_path).convert("RGB")
        backend = backends.get_backend(profile["backend"])

        # prompt embedding runs in a child process so the multi-GB text
        # encoder (incl. bnb-retained fp32 copies) is fully released on exit
        import torch
        embeds_path = run_dir / "prompt-embeds.pt"
        t_prompt = backend.encode(profile, prompt, negative, embeds_path)
        embeds = {}
        if embeds_path.exists():
            receipt["prompt_embeds"] = {
                "path": str(embeds_path),
                "sha256": sha256_file(embeds_path),
            }
            embeds = torch.load(embeds_path, weights_only=True)

        t0 = time.perf_counter()
        session = backend.load_pipe(profile)
        session["embeds"] = embeds
        session["prompt"] = prompt
        session["negative_prompt"] = negative
        t_load = time.perf_counter() - t0

        torch.cuda.reset_peak_memory_stats()
        t0 = time.perf_counter()
        frames_out, info = backend.generate(
            session, profile, mode_cfg, image, seed, frames, fps)
        t_gen = time.perf_counter() - t0
        receipt["backend_info"] = info
        receipt["torch_allocator_peak_mb"] = torch_allocator_peak_mb()

        t0 = time.perf_counter()
        frame_paths = evidence.save_frames(frames_out, frames_dir)
        evidence.encode_mp4(frames_dir, fps, run_dir / "output.mp4")
        idx = evidence.representative_indices(len(frame_paths))
        evidence.contact_sheet(frame_paths, idx, run_dir / "contact-sheet.png")
        t_encode = time.perf_counter() - t0

        probe = evidence.probe_mp4(run_dir / "output.mp4")
        receipt["output"] = {
            "path": str(run_dir / "output.mp4"),
            "sha256": sha256_file(run_dir / "output.mp4"),
            "probe": probe,
        }
        receipt["contact_sheet"] = str(run_dir / "contact-sheet.png")
        receipt["representative_frames"] = idx
        receipt["success"] = True
    except Exception as e:
        receipt["error"] = f"{type(e).__name__}: {e}"
        receipt["traceback_tail"] = traceback.format_exc()[-3000:]
    finally:
        t_total = time.perf_counter() - t_total_start
        mon = monitor.stop()
        receipt["resources"] = mon
        receipt["timing_s"] = {
            "prompt_encode": round(t_prompt, 2),
            "model_load": round(t_load, 2),
            "generation": round(t_gen, 2),
            "frame_save_encode_sheet": round(t_encode, 2),
            "total_wall": round(t_total, 2),
        }
        write_json(run_dir / "run.json", receipt)
        if receipt["success"]:
            _write_review_template(run_dir / "review.md", receipt)

    return run_dir


def _write_review_template(path: Path, receipt: dict) -> None:
    path.write_text(f"""# Visual review — {receipt['run_id']}

Profile: `{receipt['profile']}` mode `{receipt['mode']}` — scored articulation comparison: {receipt['scored_articulation_comparison']}

| Check | Finding |
| --- | --- |
| character identity preservation | _pending review_ |
| fixed camera / orientation | _pending review_ |
| torso / global drift | _pending review_ |
| anatomy regeneration | _pending review_ |
| limb growth/shrinkage | _pending review_ |
| background drift | _pending review_ |
| outline/style drift | _pending review_ |
| requested local motion occurred | _pending review_ |
| non-target regions stable | _pending review_ |

Reviewer notes:

""", encoding="utf-8")
