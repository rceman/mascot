# Mascot Local I2V Benchmark Harness v0.1

**Task family:** local development / model evaluation  
**Target machine:** Windows 11, NVIDIA RTX 4070 12 GB, 32 GB system RAM  
**Primary goal:** build a reproducible, agent-operable local image-to-video benchmark harness for mascot motion experiments.

## 1. Scope

This task is intentionally separate from the native mascot animation/rig implementation.

The paused rig branch remains the source baseline, but this branch owns only:

- local I2V model discovery and qualification;
- repeatable model installation/runtime setup;
- deterministic benchmark orchestration;
- 480p-class input preparation;
- short diagnostic I2V runs;
- resource/timing capture;
- output inspection artifacts;
- concise model comparison evidence.

Do not resume or modify the rig-art reconstruction, mesh renderer, animation clips, or native animation framework in this task unless a tiny read-only integration is required to reuse an existing asset.

## 2. Canonical workflow: CLI first

The canonical benchmark MUST be script/CLI driven.

ComfyUI is **not** the canonical runtime and MUST NOT be required for normal benchmark execution.

Preferred shape:

    tools/i2v-bench/
      README.md
      pyproject.toml
      ...
    benchmark/i2v/
      configs/
      prompts/
      results/

The exact layout may change if the repository strongly suggests a simpler owner.

The harness should provide one stable command surface that can:

1. select a backend/model profile;
2. select an input image;
3. set seed;
4. set frame count / FPS / step profile where supported;
5. run generation;
6. capture timings and peak resource use;
7. save a normalized run receipt;
8. extract representative frames;
9. build a contact sheet;
10. report the output path and concise metrics.

Do not build a large abstraction framework before at least two real backends prove what must be abstracted.

If model dependencies conflict materially, isolate backends rather than forcing one giant Python environment. A thin subprocess adapter is acceptable.

## 3. No CI

Do not add or use GitHub Actions/CI for GPU validation.

All model installation, CUDA execution, video generation, evidence inspection, and performance measurement are native local Windows work on the RTX 4070.

## 4. Windows Git authority — task-specific override

For this task, native Windows Git + SSH is authoritative.

This is an explicit task-specific override of the WSL-authority wording currently present in `AGENTS.md` section 9.

Requirements:

- work from a native Windows clone/check-out of this branch;
- fetch/pull/push directly from Windows using Git over SSH;
- do not create a WSL mirror for this task;
- do not copy `.git` between environments;
- final branch must be clean and pushed from Windows.

## 5. Resolution / aspect-ratio / speed policy

Mascot character assets and benchmark inputs are **1:1 square by product contract**.

For v0.1, use exactly:

    480 x 480

for all scored character I2V runs and derived character inputs.

Do not create or maintain alternate 16:9 / landscape / portrait character assets such as:

    832x480
    480x832

That is unnecessary work for this product and makes model comparison less relevant.

If a backend/checkpoint cannot run a 1:1 480x480 character input/output path without violating its actual model contract, record it as unsupported for this benchmark and move to another candidate. Do not create a second aspect-ratio benchmark lane merely to accommodate that model.

Forbidden in v0.1 unless the Planner explicitly approves it:

- non-1:1 character benchmark assets;
- 720p benchmark runs;
- 1024p/1080p runs;
- upscaling as part of the scored benchmark;
- frame interpolation as part of the scored benchmark.

The purpose of v0.1 is fast qualification for the actual Mascot product format, not general video-model coverage.

## 6. Temporal benchmark profile

Use a short canonical diagnostic profile where technically supported:

    frames: 33
    target fps: 16
    fixed seed: 42

Why 33 frames:

- short enough for fast iteration;
- long enough to expose orientation/identity drift;
- compatible with common 4n+1 / 8n+1 temporal constraints.

A backend may require native FPS or another frame constraint. Do not violate its model contract. Record the deviation explicitly in the run receipt.

Provide two execution modes:

### smoke

Minimum practical step count / distilled path sufficient to prove:

- load;
- inference;
- decode;
- encode;
- receipt generation.

### compare

The fastest model-supported profile that is still intended to preserve representative visual quality. Do not equate a 4-step smoke path with a model's quality profile unless its official/distilled checkpoint is designed for it.

## 7. Input asset policy

Do not destructively modify `assets/mascot.png`.

Derived inference assets belong under a dedicated development-only path, for example:

    assets/i2v/

The harness MUST provide deterministic preprocessing that:

- converts the chosen reference to sRGB RGBA/RGB as required;
- letterboxes/pads rather than stretches;
- uses a plain white background for the initial benchmark;
- centers the character;
- preserves aspect ratio;
- records source SHA-256 and derived SHA-256;
- records the exact preprocessing parameters.

For a 480x480 benchmark canvas, keep enough margin for limb/tail motion.

### Neutral/T-pose reference

A neutral/T-pose mascot reference is preferred for the articulation benchmark, but no generated neutral image is canonical merely because it exists locally.

If no Planner/user-approved neutral reference is committed/provided:

1. use `assets/mascot.png` only for infrastructure smoke runs;
2. do not fabricate or regenerate a replacement inside this task;
3. mark the articulation-quality comparison as pending the approved neutral reference.

Do not promote an arbitrary AI-generated T-pose to canonical art.

## 8. Initial benchmark motion

The first scored visual test is intentionally narrow:

    one moving part only

Preferred prompt: near-arm planar motion on an approved neutral reference.

The test is not a cinematic animation. It is a diagnostic identity/orientation preservation test.

Canonical positive prompt lives in:

    benchmark/i2v/prompts/near-arm-paper-puppet-v0.1.txt

Canonical negative prompt lives in:

    benchmark/i2v/prompts/near-arm-paper-puppet-v0.1-negative.txt

Backends without a separate negative-prompt surface should record that fact; do not invent an equivalent hidden behavior.

## 9. Candidate research requirement

Before downloading large checkpoints, research current open/open-weight I2V options from primary sources and create:

    docs/MASCOT_LOCAL_I2V_CANDIDATE_RESEARCH_V0.1.md

For each candidate record:

- model/checkpoint exact name;
- upstream owner;
- release/update date;
- license identifier/link;
- parameter count where published;
- I2V capability;
- native/recommended **1:1 480x480** support;
- first-frame vs first+last-frame support;
- recommended inference steps;
- quantization/offload options;
- expected disk footprint;
- expected VRAM strategy for 12 GB;
- direct Python/CLI support;
- whether ComfyUI is optional, required, or unnecessary;
- why the candidate is relevant to this mascot diagnostic.

Initial candidates to verify, not blindly assume:

1. **HunyuanVideo-1.5 480p I2V step-distilled** — official Tencent material describes an 8.3B model and a 480p I2V step-distilled checkpoint intended for 8/12-step generation.
2. **Wan2.1 VACE-1.3B** — official VACE/Wan material exposes a 480p 1.3B control-capable model and native CLI inference.
3. **Wan2.2 TI2V-5B** — official Wan2.2 unified T2V/I2V model; verify whether a practical 480p-class local path exists before including it in the comparable benchmark because the official Wan config primarily advertises its own larger native profile.
4. **Wan2.2 I2V A14B quantized/offloaded** — optional quality-reference candidate only if a 12 GB path is genuinely practical.
5. **LTX-2 / newer alternatives available at execution time** — research only; do not download heavyweight candidates merely because they are newer.

The agent MUST search for newer practical candidates available at execution time. The list above is a seed list, not a frozen ranking.

Primary sources should be preferred over community claims.

Useful starting points:

- https://github.com/Tencent-Hunyuan/HunyuanVideo-1.5
- https://huggingface.co/tencent/HunyuanVideo-1.5
- https://github.com/ali-vilab/VACE
- https://huggingface.co/Wan-AI/Wan2.1-VACE-1.3B
- https://github.com/Wan-Video/Wan2.2
- https://huggingface.co/Wan-AI/Wan2.2-TI2V-5B
- https://huggingface.co/Lightricks/LTX-2

## 10. Candidate count

v0.1 should successfully run **at least two materially different local I2V backends/models** on the RTX 4070 12 GB.

Prefer three if setup cost remains reasonable.

Do not burn time forcing a clearly impractical model to work. If a candidate exceeds the machine's practical VRAM/RAM/time envelope, record the measured blocker and move to the next candidate.

## 11. Download/cache discipline

Model weights and caches MUST NOT be committed.

Use an external configurable model cache, for example:

    MASCOT_I2V_MODEL_CACHE=<path>

Record exact upstream model revision / checkpoint filename / quantization in the run receipt.

Do not redownload a model on every run.

No remote inference APIs are part of this benchmark.

After required model downloads, benchmark generation should run locally.

Disable remote prompt rewriting/enhancement by default. The exact prompt supplied to the model must be preserved in evidence.

## 12. Normalized run receipt

Every completed generation MUST emit machine-readable metadata, for example:

    benchmark/i2v/results/<model>/<run-id>/run.json

Minimum fields:

- schema_version;
- run_id;
- UTC timestamp;
- git commit;
- hostname or stable machine label;
- GPU name;
- driver version;
- CUDA / PyTorch versions;
- model id;
- model revision/checkpoint;
- quantization/dtype;
- model/backend version;
- source image path + SHA-256;
- derived input path + SHA-256;
- width/height;
- aspect_ratio (must be 1:1 for scored runs);
- resolution_mode;
- frame count;
- output FPS;
- seed;
- steps;
- guidance/CFG values when applicable;
- scheduler/sampler when applicable;
- offload strategy;
- first/last-frame conditioning mode;
- prompt SHA-256;
- negative-prompt SHA-256 or null;
- model load time;
- generation time;
- decode/encode time if measurable;
- total wall time;
- peak VRAM MB;
- peak system RAM MB when practical;
- output file SHA-256;
- success/failure status;
- explicit deviations from canonical benchmark settings.

Keep stdout concise; detailed metadata belongs in the receipt.

## 13. GPU/resource measurement

Use a reproducible Windows measurement mechanism.

At minimum capture:

- GPU identity;
- peak observed dedicated VRAM;
- wall-clock generation time.

A bounded `nvidia-smi` polling helper is acceptable for cross-process backends. Use a reasonable polling interval and stop it reliably on success/failure.

If the backend exposes a more accurate allocator peak, record both and label the source.

Do not run multiple generation jobs concurrently in v0.1.

## 14. Visual evidence

For every compare run produce:

- output MP4;
- representative extracted frames;
- one contact sheet;
- a concise visual-review JSON/Markdown record.

Inspect at minimum:

- character identity preservation;
- fixed camera/orientation;
- torso drift;
- unintended global motion;
- anatomy regeneration;
- limb growth/shrinkage;
- background drift;
- outline/style drift;
- whether requested local motion occurred;
- whether non-target regions remained reasonably stable.

Do not claim a model “passes” merely because it produced a valid MP4.

The agent may use computer vision metrics as supporting evidence, but visual review remains required for generative failure modes.

## 15. Optional simple image metrics

Keep v0.1 metrics narrow and interpretable.

Allowed examples:

- source vs first-frame perceptual/pixel diff;
- source vs final-frame diff when return-to-start is requested;
- SSIM/LPIPS if already available without disproportionate dependency cost;
- non-target-region difference if a trustworthy static mask exists.

Do not create a fake precision score that collapses all visual behavior into one opaque number.

## 16. Result storage policy

Suggested layout:

    benchmark/i2v/results/windows/local-i2v-v0.1/
      summary.json
      summary.md
      <backend-model>/
        <run-id>/
          run.json
          output.mp4
          contact-sheet.png
          review.md

Do not commit:

- model checkpoints;
- Hugging Face caches;
- Python virtualenvs;
- huge extracted frame directories;
- temporary tensors;
- duplicate videos.

For evidence committed to Git, keep artifacts bounded. Prefer one short compare MP4 + contact sheet per successful model. If an MP4 is unexpectedly large, commit its receipt/contact sheet/hash and keep the large binary local.

## 17. Model comparison

Comparison must separate measured facts from visual judgments.

Minimum table columns:

- model/checkpoint;
- backend;
- quantization;
- resolution (scored runs: 480x480 only);
- frames/FPS;
- steps;
- peak VRAM;
- generation time;
- model-load time;
- I2V completed;
- orientation drift observed;
- non-target motion observed;
- identity preservation notes;
- local-motion adherence notes.

Do not declare a universal “winner” from one mascot prompt. The task should identify which candidates are worth deeper mascot-specific testing.

## 18. Dependency policy

This is development tooling only.

Do not add Python/video-generation dependencies to the production Rust runtime.

Keep GPU/model dependencies isolated under the benchmark tooling.

A development-only ffmpeg dependency is acceptable if needed for deterministic encoding/frame extraction and is documented.

ComfyUI may be installed only as an optional exploratory comparison if direct/model-owned inference is blocked or materially deficient. It MUST NOT become required for the canonical benchmark without Planner approval.

## 19. Failure/stop conditions

Stop and report rather than hiding the problem if:

- CUDA/toolchain installation requires destructive system changes;
- a model requires more than the practical 12 GB VRAM path can provide and offload becomes unusably slow;
- official licensing/availability prevents automated use;
- model dependencies require incompatible global Python mutations;
- a backend only works through a remote service;
- benchmark outputs cannot be reproduced from recorded config;
- the approved neutral reference is missing for the scored articulation comparison.

A failed candidate does not fail the whole task if at least two other useful local I2V candidates complete and the failure is documented.

## 20. Acceptance criteria

The task is complete only when:

- native Windows CLI harness is implemented;
- no ComfyUI dependency is required for normal execution;
- all scored character runs are exactly 480x480 (1:1);
- at least two distinct local I2V candidates successfully generate a short video on RTX 4070 12 GB;
- exact model/runtime provenance is recorded;
- seed/config are reproducible where the backend supports determinism;
- resource/timing receipts are produced automatically;
- contact sheets are produced automatically;
- model weights/caches remain outside Git;
- benchmark results are summarized;
- all applicable Mascot Universal Gates 1-20 are reported PASS/FAIL/N/A with evidence;
- branch is clean and pushed from native Windows Git.

