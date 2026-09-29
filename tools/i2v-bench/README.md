# i2v-bench — local image-to-video benchmark harness (v0.1)

Development-only tooling. Keeps all video-model Python/CUDA dependencies out of the
production Rust runtime. Canonical execution is this CLI — no ComfyUI required.

## Setup (native Windows)

```powershell
cd tools/i2v-bench
python -m venv .venv
.venv\Scripts\pip install -U pip
.venv\Scripts\pip install torch --index-url https://download.pytorch.org/whl/cu128
.venv\Scripts\pip install -e .
.venv\Scripts\pip install bitsandbytes protobuf tiktoken
```

Model weights/caches live **outside Git**. Default cache:

    W:\devin_folder\mascot-i2v-model-cache

Override with `MASCOT_I2V_MODEL_CACHE=<path>` before running.

Requires `ffmpeg`/`ffprobe` on PATH (dev-only evidence dependency, documented).

## Commands

```powershell
# environment identity
.venv\Scripts\python -m i2v_bench env

# list model profiles
.venv\Scripts\python -m i2v_bench profiles

# deterministic input prep (letterbox, white bg, centered)
.venv\Scripts\python -m i2v_bench prepare `
  --input assets/mascot.png --canvas 480x480 --margin 0.10 `
  --out assets/i2v/mascot-480x480.png

# prefetch weights
.venv\Scripts\python -m i2v_bench download --profile ltxv-2b-0.9.5

# smoke run (low step count, proves load/generate/encode/receipt)
.venv\Scripts\python -m i2v_bench run `
  --profile ltxv-2b-0.9.5 --mode smoke `
  --input assets/i2v/neutral-tpose-480x480.png `
  --prompt benchmark/i2v/prompts/near-arm-paper-puppet-v0.1.txt `
  --negative benchmark/i2v/prompts/near-arm-paper-puppet-v0.1-negative.txt `
  --seed 42 --frames 33 --scored-articulation

# compare-quality run + aggregate summary
.venv\Scripts\python -m i2v_bench run --profile wan21-vace-1.3b --mode compare ...
.venv\Scripts\python -m i2v_bench compare
```

Run receipts + evidence land in
`benchmark/i2v/results/windows/local-i2v-v0.1/<profile>/<run-id>/`:
`run.json`, `output.mp4`, `contact-sheet.png`, `review.md`, `frames/`
(frames are gitignored; keep artifacts bounded for commit).

## VRAM architecture (12 GB card, WDDM)

Every candidate needs a ~10 GB T5/UMT5 encoder plus several GB of
transformer/VAE — too much for one process on a 12 GB WDDM card. The harness
therefore runs **prompt encoding in a child process** (`i2v_bench
_encode-child`): the int8 encoder loads, encodes, and the whole GPU footprint
is released on process exit. Embeds are persisted to `prompt-embeds.pt` inside
the run dir (hash recorded in `run.json`) and reused by `load_pipe`/`generate`.
This keeps the generation phase at ~6 GB allocated and avoids WDDM
shared-memory paging (~30x step-time speedup observed).

`wan22-ti2v-5b` also quantizes its transformer to int8 (bf16 ~9.3 GB exceeds the
post-desktop budget) and uses `WanImageToVideoPipeline` with
`expand_timesteps=True` — the official diffusers path for TI2V-5B.

## Scored articulation comparison

`--scored-articulation` is only valid against a Planner/user-approved neutral
T-pose reference (currently `assets/i2v/neutral-tpose-480x480.png`, prepared from
the approved source PNG — the guard resolves it via the adjacent `.prep.json`).
