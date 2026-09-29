# Mascot Local I2V Candidate Research v0.1

**Date:** 2026-09-29  
**Machine:** Windows 11, NVIDIA RTX 4070 12 GB (driver 596.36, CUDA 13.2), 32 GB RAM  
**Constraint:** all scored generation at 480p-class or lower; ~7.5-8 GB usable VRAM after Windows desktop baseline (~4.5 GB resident).

Primary sources were used wherever possible (vendor GitHub/Hugging Face model cards). Community VRAM claims are marked as such.

## Candidate matrix

### 1. LTX-Video 0.9.7/0.9.8 2B distilled (Lightricks)

- **Checkpoints:** `Lightricks/LTX-Video-0.9.7-distilled`, `Lightricks/LTX-Video-0.9.7-dev`, `Lightricks/LTX-Video-2B-0.9.6-Distilled-04-25`, `Lightricks/LTX-Video` (0.9.x line, 2B). `Lightricks/LTX-Video-0.9.8-13B-distilled` exists but is the 13B tier (92.8 GB repo) — rejected.
- **Owner:** Lightricks. **Release:** 0.9.7 line Jul 2025. **License:** LTXV Open Weights license (`license:other`, per model card).
- **Parameters:** 2B (transformer) + T5-XXL text encoder.
- **I2V capability:** yes, first-frame conditioning (`LTXImageToVideoPipeline` / `LTXConditionPipeline` in diffusers). Multi-keyframe conditioning also supported.
- **480p support:** native operating point is 768x512; arbitrary multiples of the 32x spatial VAE compression are accepted, so 480x480 is legal. Frames must be 8n+1 → 33 OK. Native FPS 24-25 (record as deviation vs target 16).
- **Steps:** distilled variant is guidance+timestep distilled; official examples run ~4-8 steps at CFG 1.0.
- **Quantization/offload:** bf16; ~10 GB VRAM per diffusers docs; fits 12 GB with model-level CPU offload headroom.
- **Disk footprint:** ~4 GB transformer + ~9-11 GB T5-XXL bf16 + VAE ≈ 13-15 GB.
- **Direct Python/CLI:** diffusers pipeline; ComfyUI unnecessary.
- **Relevance:** fastest credible candidate; ideal for smoke/iteration profile and a real compare data point.

### 2. Wan2.1-VACE-1.3B (Alibaba Wan-AI / ali-vilab VACE)

- **Checkpoint:** `Wan-AI/Wan2.1-VACE-1.3B-diffusers` (also official `Wan-AI/Wan2.1-VACE-1.3B` + VACE repo `generate.py`).
- **Owner:** Wan-AI / Alibaba. **Release:** Apr 2025 (VACE). **License:** Apache-2.0.
- **Parameters:** 1.3B transformer + UMT5-XXL text encoder (~11 GB bf16 — the heavy component).
- **I2V capability:** VACE is a unified video-conditioned model; first-frame I2V via video+mask conditioning (keep frame 0, inpaint remainder). `WanVACEPipeline` in diffusers >= 0.34.
- **480p support:** native 480P profile (~81x480x832). Supports 832x480 / 480x832. Frames 4n+1 → 33 OK. Native 16 FPS — matches benchmark target.
- **Steps:** UniPCMultistepScheduler, flow_shift=3.0 for 480P; ~30-50 quality steps; smoke can use fewer.
- **Quantization/offload:** 1.3B transformer bf16 ~2.7 GB; T5-XXL must be int8-quantized (bitsandbytes ~5.5 GB) or sequential-offloaded to fit the ~8 GB free-VRAM envelope.
- **Disk footprint:** ~14 GB.
- **Direct Python/CLI:** diffusers `WanVACEPipeline` and official VACE CLI; ComfyUI unnecessary.
- **Relevance:** only seed-listed model with a *native* 480p I2V contract at consumer scale; materially different architecture (Wan/VACE vs LTX DiT).

### 3. Wan2.2-TI2V-5B (Wan-AI)

- **Checkpoint:** `Wan-AI/Wan2.2-TI2V-5B-Diffusers` (34.2 GB repo incl. text encoder).
- **Owner:** Wan-AI / Alibaba. **Release:** Jul 2025. **License:** Apache-2.0.
- **Parameters:** 5B transformer + UMT5-XXL + high-compression Wan2.2-VAE (16x16x4).
- **I2V capability:** unified T2V+I2V; diffusers `WanPipeline`/auto-pipeline. Official contract advertises 720P@24fps only; 480p-class (e.g. 832x480) is off-label but pipeline-legal → must be recorded `resolution_mode=native_480p`/`cross_model_resolution_comparable=false`.
- **Steps:** ~50 quality steps (UniPC); distilled/CFG-distill variants not part of this checkpoint.
- **Quantization/offload:** official CLI says >= 24 GB; community reports ~8 GB with aggressive offloading (ComfyUI, unverified). On this machine bf16 transformer (~10 GB) exceeds free VRAM → requires sequential CPU offload or fp8/int8 weight quantization; expect materially slower generation.
- **Disk footprint:** ~17-21 GB needed components.
- **Direct Python/CLI:** diffusers auto-pipeline; ComfyUI optional, not required.
- **Relevance:** newest Wan architecture; worth a bounded attempt after the two cheap candidates land. Drop if offload becomes unusably slow.

### 4. HunyuanVideo-1.5 480p I2V step-distilled (Tencent) — REJECTED for v0.1

- **Checkpoint:** `tencent/HunyuanVideo-1.5` (`HunyuanVideo-1.5-480P-I2V-step-distill`, 8.3B).
- **Owner:** Tencent. **Release:** Nov-Dec 2025 (step-distilled 480p I2V added Dec 5, 2025). **License:** Tencent Hunyuan community license (`license:other`).
- **Blocker:** official model card states **minimum GPU memory 14 GB with offloading enabled** — exceeds the 12 GB card. The only sub-12 GB paths are third-party FP8-scaled repacks (Civitai, ~8 GB claim, unverified, non-canonical provenance). Official diffusers-format transformer alone is a 33.3 GB safetensors blob.
- **Decision:** record as researched/impractical; do not download. Revisit only if an official sub-12 GB path appears.

### 5. Wan2.2-I2V-A14B — REJECTED (quality reference only per spec)

- 14Bx2 MoE, official 480P support, but ~28B params → impractical on 12 GB even quantized within bounded effort. Recorded, not attempted.

### 6. LTX-2 family (LTX-2 / LTX-2.3 / LTX-2.5) — RESEARCHED, DEFERRED

- Weights are now open (`Lightricks/LTX-2`, `LTX-2.3`, `LTX-2.5`, `LTX-2.3-fp8`, `LTX-2.3-nvfp4`), but all are 19-22B audio-video models; full component download ~66 GiB. The nvfp4 variant targets Blackwell-class FP4 tensor cores — RTX 4070 (Ada sm_89) cannot execute FP4 matmuls natively. No credible 12 GB path. Deferred.

## Selected candidates for v0.1

| Slot | Model | Why |
| --- | --- | --- |
| A | `Lightricks/LTX-Video-0.9.5` (2B) | cheapest credible path, diffusers-native |
| B | `Wan-AI/Wan2.1-VACE-1.3B-diffusers` | native 480P I2V contract, Apache-2.0, different architecture |
| C (bounded) | `Wan-AI/Wan2.2-TI2V-5B-Diffusers` | only if A+B land cleanly and offload path proves practical |

## Empirical outcomes (2026-09-29, RTX 4070, 480x480 only)

- **LTX-Video-0.9.5 — SUPPORTED.** `LTXConditionPipeline`, first-frame conditioning. Smoke 10-step generation 3.8 s; compare 30-step 8.0 s. Identity/camera/background stable; requested near-arm motion not executed (clip static). Note: the initially-configured `LTX-Video-0.9.7-distilled` repo turned out to contain a ~10B transformer (~20 GB bf16) — dropped in favour of 0.9.5.
- **Wan2.1-VACE-1.3B — UNSUPPORTED at 480x480.** Conditioning stream produces corrupted block-mosaic output at 1:1; the identical pipeline produced coherent output at the official 832x480 profile (diagnostic probe) and coherent T2V at 480x480. Model contract is ~480x832; marked unsupported per the 1:1-only Planner directive.
- **Wan2.2-TI2V-5B — SUPPORTED (bounded, off-label res).** Official diffusers path is `WanImageToVideoPipeline` + `expand_timesteps=True` (not the Wan22 I2V modular blocks — those target I2V-A14B). int8 transformer + int8 UMT5. Smoke 8 steps / compare 30 steps both valid 480x480; identity stable, motion not executed. ~1-1.2 s/step steady state.

## Memory architecture (why a subprocess boundary was required)

- bnb `load_in_8bit` keeps the fp32 original weights reachable after quantization, so `del` cannot release the ~13.6 GB the encoder phase touched (T5 int8 ~7.6 GB + retained fp32 ~6 GB).
- Prompt embedding therefore runs in a short-lived child process (`i2v_bench _encode-child`); all encoder memory is released by the OS on exit. Generation-phase alloc dropped from ~19.5 GB (WDDM paging, ~29 s/step) to ~6 GB (~0.4-1.0 s/step).

## Shared infrastructure notes

- All three need a T5-class text encoder (~11 GB bf16); plan: `bitsandbytes` int8 or sequential offload for the encoder when peak VRAM demands it.
- HF cache lives outside Git at `MASCOT_I2V_MODEL_CACHE` (default `W:\devin_folder\mascot-i2v-model-cache`).
- No remote inference; prompt rewriting disabled by default (none of these pipelines rewrite prompts anyway — receipt still records the exact prompt hash).
