import argparse
import json
import sys
from pathlib import Path

from . import config


def _run_memprobe(args: argparse.Namespace) -> int:
    """Bounded low-step run that dumps torch CUDA allocator stats per phase."""
    import tempfile
    import torch
    from PIL import Image
    from . import backends
    from .profiles import get_profile

    def snap(tag: str) -> None:
        alloc = torch.cuda.memory_allocated() / 2**30
        reserv = torch.cuda.memory_reserved() / 2**30
        peak = torch.cuda.max_memory_allocated() / 2**30
        print(f"[memprobe {tag}] alloc={alloc:.2f}G reserved={reserv:.2f}G peak={peak:.2f}G",
              flush=True)

    profile = get_profile(args.profile)
    backend = backends.get_backend(profile["backend"])
    prompt = Path(args.prompt).read_text(encoding="utf-8").strip()
    negative = Path(args.negative).read_text(encoding="utf-8").strip() if args.negative else None
    image = Image.open(args.input).convert("RGB")

    snap("start")
    with tempfile.NamedTemporaryFile(suffix=".pt", delete=False) as f:
        pt = Path(f.name)
    backend.encode(profile, prompt, negative, pt)
    embeds = torch.load(pt, weights_only=True)
    pt.unlink()
    snap("after-encode-child")
    session = backend.load_pipe(profile)
    session["embeds"] = embeds
    snap("loaded")
    torch.cuda.reset_peak_memory_stats()
    mode_cfg = dict(profile["modes"]["smoke"])
    mode_cfg["steps"] = args.steps
    frames, info = backend.generate(
        session, profile, mode_cfg, image,
        profile["seed"], int(args.frames or 9), profile["fps"])
    snap("after-generate")
    out = config.DEFAULT_RESULTS / "memprobe-first-frame.png"
    frames[0].save(out)
    print("memprobe done ->", out, flush=True)
    return 0


def _is_approved_neutral_input(input_path: Path) -> bool:
    """True when the input is (or was derived from) the committed approved
    neutral reference (filename marker: 'approved-neutral')."""
    if "approved-neutral" in input_path.name:
        return True
    prep = input_path.with_suffix(".prep.json")
    if prep.exists():
        try:
            src = Path(json.loads(prep.read_text(encoding="utf-8"))["source_path"])
            return "approved-neutral" in src.name
        except Exception:
            return False
    return False


def _add_common_run_args(p: argparse.ArgumentParser) -> None:
    p.add_argument("--profile", required=True)
    p.add_argument("--mode", choices=["smoke", "compare"], default="smoke")
    p.add_argument("--input", required=True, type=Path)
    p.add_argument("--prompt", required=True, type=Path)
    p.add_argument("--negative", type=Path, default=None)
    p.add_argument("--seed", type=int, default=None)
    p.add_argument("--frames", type=int, default=None)
    p.add_argument("--out", type=Path, default=config.DEFAULT_RESULTS)
    p.add_argument("--scored-articulation", action="store_true",
                   help="mark run as the scored articulation comparison "
                        "(requires approved neutral reference input)")


def main() -> None:
    config.setup_hf_env()
    ap = argparse.ArgumentParser(prog="i2v-bench", description="Local I2V benchmark harness")
    sub = ap.add_subparsers(dest="cmd", required=True)

    sub.add_parser("env", help="print environment identity JSON")
    sub.add_parser("profiles", help="list configured profiles")

    p = sub.add_parser("prepare", help="deterministic letterbox input preparation")
    p.add_argument("--input", required=True, type=Path)
    p.add_argument("--canvas", required=True, help="WxH, e.g. 480x480")
    p.add_argument("--margin", type=float, default=0.10)
    p.add_argument("--out", required=True, type=Path)

    p = sub.add_parser("run", help="run one benchmark generation")
    _add_common_run_args(p)

    p = sub.add_parser("download", help="download model weights into the model cache")
    p.add_argument("--profile", required=True)

    p = sub.add_parser("memprobe", help="bounded run printing CUDA allocator stats per phase")
    p.add_argument("--profile", required=True)
    p.add_argument("--input", required=True, type=Path)
    p.add_argument("--prompt", required=True, type=Path)
    p.add_argument("--negative", type=Path, default=None)
    p.add_argument("--frames", type=int, default=9)
    p.add_argument("--steps", type=int, default=2)

    p = sub.add_parser("compare", help="aggregate run receipts into summary.json/summary.md")
    p.add_argument("--results", type=Path, default=config.DEFAULT_RESULTS)

    # internal: prompt-embedding child process (isolated VRAM lifecycle)
    p = sub.add_parser("_encode-child")
    p.add_argument("--backend", required=True)
    p.add_argument("--repo", required=True)
    p.add_argument("--revision", default=None)
    p.add_argument("--prompt", required=True)
    p.add_argument("--negative", default=None)
    p.add_argument("--out", required=True, type=Path)

    args = ap.parse_args()

    if args.cmd == "env":
        from .env import env_identity
        print(json.dumps(env_identity(), indent=2))
    elif args.cmd == "profiles":
        from .profiles import load_profiles
        for name, p in load_profiles().items():
            print(f"{name:28s} {p['backend']:10s} {p['hf_repo']}  {p['width']}x{p['height']}")
    elif args.cmd == "prepare":
        from .prepare import prepare_input
        w, h = (int(x) for x in args.canvas.lower().split("x"))
        rec = prepare_input(args.input, w, h, args.margin, args.out)
        print(json.dumps(rec, indent=2))
    elif args.cmd == "run":
        from .profiles import get_profile
        from .runner import run
        profile = get_profile(args.profile)
        if args.scored_articulation and not _is_approved_neutral_input(args.input):
            print("WARNING: --scored-articulation requires input derived from a "
                  "Planner/user-approved neutral reference; marking unscored.",
                  file=sys.stderr)
            args.scored_articulation = False
        run_dir = run(profile, args.mode, args.input, args.prompt, args.negative,
                      args.out, seed=args.seed, frames=args.frames,
                      scored_articulation=args.scored_articulation)
        receipt = json.loads((run_dir / "run.json").read_text(encoding="utf-8"))
        print(json.dumps({
            "run_dir": str(run_dir),
            "success": receipt["success"],
            "peak_vram_mb": receipt["resources"]["peak_vram_mb"],
            "timing_s": receipt["timing_s"],
            "error": receipt.get("error"),
        }, indent=2))
        sys.exit(0 if receipt["success"] else 1)
    elif args.cmd == "download":
        from huggingface_hub import snapshot_download
        from .profiles import get_profile
        profile = get_profile(args.profile)
        rev = snapshot_download(profile["hf_repo"], revision=profile.get("hf_revision") or None)
        print(rev)
    elif args.cmd == "memprobe":
        sys.exit(_run_memprobe(args))
    elif args.cmd == "_encode-child":
        from .backends.common import encode_child_main
        encode_child_main(args)
    elif args.cmd == "compare":
        from .summary import write_summary
        js, md = write_summary(args.results)
        print(f"wrote {js} and {md}")
