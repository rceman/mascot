# Agent handoff — Local I2V benchmark harness on RTX 4070 12 GB (SWE-2)

Task ID: MASCOT-I2V-BENCH-001

## Goal

Build a reproducible, agent-operable local **image-to-video benchmark harness** on the user's native Windows machine with:

- NVIDIA RTX 4070 12 GB;
- 32 GB system RAM;
- local-only generation after model download;
- short 480p-class diagnostic videos;
- exact run provenance;
- automatic timing/VRAM evidence;
- automatic contact-sheet generation.

This task is independent from the paused mascot native animation-framework work.

Do **not** resume rig-art repair, mesh rendering, clip authoring, or animation-runtime implementation in this branch.

## Branch

Work only on:

    agent/local-i2v-benchmark-rtx4070-swe2

This branch was forked from the paused rig/planner state:

    34454374954ee29a1c12149ba75639250515d23e

Do not write to:

    agent/windows-rig-animation-lab-v0.2-opus55

## Read first

Read in full before editing:

1. `AGENTS.md`
2. `docs/QUALITY_GATES.md`
3. `docs/MASCOT_LOCAL_I2V_BENCHMARK_SPEC_V0.1.md`
4. `benchmark/i2v/prompts/near-arm-paper-puppet-v0.1.txt`
5. `benchmark/i2v/prompts/near-arm-paper-puppet-v0.1-negative.txt`

The benchmark spec is authoritative for this task.

## Repository and native Windows workspace

Use exactly this repository:

    git@github.com:rceman/mascot.git

GitHub repository:

    rceman/mascot

Use this dedicated native Windows working directory:

    W:\\devin_folder\\mascot-i2v-benchmark

Do not reuse the paused rig working copy or another Mascot agent's checkout.

Target branch:

    agent/local-i2v-benchmark-rtx4070-swe2

Expected Planner handoff HEAD at task start:

    263210eea53e806d2f97793af6b2672913ead939

Bootstrap from native Windows PowerShell/Git Bash as appropriate:

    cd W:\\devin_folder
    git clone git@github.com:rceman/mascot.git mascot-i2v-benchmark
    cd mascot-i2v-benchmark
    git fetch origin
    git checkout agent/local-i2v-benchmark-rtx4070-swe2
    git pull --ff-only origin agent/local-i2v-benchmark-rtx4070-swe2

If `W:\\devin_folder\\mascot-i2v-benchmark` already exists, do not blindly delete or overwrite it. Inspect the existing checkout first. Reuse it only if it is the same repository and its worktree state is clean/understood.

## Task-specific Git workflow override

The repository's older WSL-authority wording does **not** apply to this task.

The user has approved native Windows Git + SSH as authoritative.

Required workflow:

1. Use only `rceman/mascot` in `W:\\devin_folder\\mascot-i2v-benchmark`.
2. Checkout `agent/local-i2v-benchmark-rtx4070-swe2`.
3. Verify the fetched branch contains the Planner handoff commit above or a later Planner-approved commit.
4. Verify branch/HEAD and inspect the worktree before editing.
5. Build/install/run everything natively on Windows.
6. Commit and push directly from native Windows Git over SSH.
7. Do not create or sync through a WSL authority copy.
8. Do not write into `W:\\devin_folder\\mascot-rig-v02` or any other agent workspace.
9. Leave the Windows worktree clean and pushed.

## No CI

Do not add GitHub Actions.

Do not use CI for CUDA/GPU/model validation.

All meaningful validation is local on the RTX 4070.

## Primary implementation outcome

Build a small reusable CLI/script harness, not a GUI workflow.

The canonical path MUST NOT require ComfyUI.

A reasonable implementation may look like:

    tools/i2v-bench/
      pyproject.toml
      README.md
      src/...
    benchmark/i2v/
      configs/
      prompts/
      results/

but prefer the smallest structure that remains reusable.

Expected operator experience should be close to:

    <command> run       --model <profile>       --input <image>       --prompt benchmark/i2v/prompts/near-arm-paper-puppet-v0.1.txt       --seed 42       --frames 33

and:

    <command> compare <run-or-profile...>

Exact CLI syntax is implementation-owned.

## 480p-only rule

Keep v0.1 benchmark generation at 480p-class or lower.

Preferred comparable input/output canvas:

    480x480

If a model's official/native contract requires another 480p geometry such as:

    832x480
    480x832

use that native profile and record the run as non-resolution-comparable.

Do not run 720p merely because the checkpoint advertises it.

Do not add upscaling or frame interpolation to the scored benchmark.

## Fast benchmark profile

Prefer:

    frames = 33
    seed = 42
    target fps = 16

when the model/backend supports them.

Implement:

- a fast smoke profile;
- a representative compare profile.

The compare profile should use the model's intended distilled/quality settings, not an arbitrarily tiny step count that destroys quality.

## Model research comes before large downloads

Before downloading heavyweight checkpoints, write:

    docs/MASCOT_LOCAL_I2V_CANDIDATE_RESEARCH_V0.1.md

Use primary/official sources where possible.

Research current candidates available at execution time.

Seed candidates to verify:

- HunyuanVideo-1.5 480p I2V step-distilled;
- Wan2.1 VACE-1.3B;
- Wan2.2 TI2V-5B if a practical 480p-class local route exists;
- Wan2.2 I2V A14B quantized/offloaded only as an optional quality reference;
- LTX-2 or newer open/open-weight I2V alternatives only when the 12 GB path is credible.

Do not assume that "newer" means better for this diagnostic.

Do not rely on community VRAM claims without validating the actual installed path.

## Minimum live model requirement

Successfully run at least **two materially different local I2V candidates** on the RTX 4070 12 GB.

Three is preferred only if setup remains bounded.

A candidate that is obviously impractical may be dropped after recording:

- exact attempted configuration;
- failure mode;
- observed VRAM/RAM/time behavior;
- why further effort is not justified.

Do not spend hours forcing one heavyweight reference model.

## Input image handling

Do not replace or overwrite:

    assets/mascot.png

Implement deterministic derived-input preparation.

For the initial local infrastructure smoke test, `assets/mascot.png` may be used.

For the actual near-arm articulation comparison, use only a Planner/user-approved neutral/T-pose reference.

If no approved neutral reference is available locally:

- finish the harness;
- run infrastructure smoke tests;
- prepare all benchmark plumbing;
- mark the scored articulation comparison as pending;
- do not generate a new "canonical" T-pose yourself.

## Model cache

Weights/caches must live outside Git.

Use a configurable cache location such as:

    MASCOT_I2V_MODEL_CACHE

Never commit:

- model weights;
- Hugging Face cache;
- Python virtual environments;
- CUDA caches;
- large temporary frame sets.

Pin/record exact model revisions/checkpoint filenames in receipts.

## Offline / prompt integrity

No remote inference API is allowed for benchmark generation.

Disable remote prompt rewriting/enhancement by default.

The model must receive the recorded benchmark prompt, not a silently rewritten cloud prompt.

Network use for downloading dependencies/checkpoints and reading official documentation is allowed.

## ComfyUI policy

Do not install ComfyUI merely because examples use it.

Prefer:

1. official model-owned Python/CLI;
2. Diffusers or another direct maintained inference API;
3. a small project-owned adapter.

ComfyUI is optional exploratory tooling only.

If a candidate genuinely cannot be qualified without ComfyUI and would otherwise be valuable, document the reason before adding it. It must not become the benchmark's required canonical execution path without Planner approval.

## Required automation

The harness should automate:

- deterministic input preparation;
- model/backend selection;
- generation;
- bounded resource monitoring;
- run receipt generation;
- video hash;
- representative frame extraction;
- contact sheet generation;
- concise summary aggregation.

Do not make the agent manually copy values from terminal output into JSON.

## Required run receipt

Every run must record at least:

- git commit;
- model/checkpoint/revision;
- backend version;
- quantization/dtype;
- input hashes;
- resolution;
- frame count/FPS;
- seed;
- steps;
- guidance/scheduler where applicable;
- offload strategy;
- first/last-frame conditioning capability/use;
- prompt hashes;
- load/generation/total times;
- peak VRAM;
- output hash;
- deviations;
- success/failure.

Detailed data belongs in files, not giant stdout.

## Resource measurement

Measure actual local behavior.

At minimum:

- GPU model;
- driver/CUDA/PyTorch identity;
- peak observed VRAM;
- generation wall time.

A bounded `nvidia-smi` sampler is acceptable for subprocess backends.

Do not benchmark multiple generation jobs concurrently.

## Evidence

For each successful compare run produce:

- `run.json`;
- short output MP4;
- representative frames;
- contact sheet;
- concise visual review.

Review explicitly:

- character identity;
- fixed camera/orientation;
- torso/global drift;
- anatomy regeneration;
- unwanted motion outside target;
- outline/style drift;
- requested local motion adherence.

A syntactically valid MP4 is not sufficient proof.

## Results summary

Produce:

    benchmark/i2v/results/windows/local-i2v-v0.1/summary.json
    benchmark/i2v/results/windows/local-i2v-v0.1/summary.md

Summary should compare measured facts without pretending one mascot prompt establishes a universal model ranking.

At minimum include:

- model/checkpoint;
- quantization;
- resolution;
- frames/FPS;
- steps;
- peak VRAM;
- load time;
- generation time;
- completed yes/no;
- orientation drift;
- non-target motion;
- identity-preservation notes;
- local-motion-adherence notes.

## Dependency hygiene

Keep video-model Python/CUDA dependencies entirely out of the production Rust runtime.

Do not modify production crates merely to host benchmark tooling.

Development-only ffmpeg is acceptable if needed and documented.

If backend dependency versions conflict, isolate them cleanly rather than mutating global Python repeatedly.

## Keep scope bounded

Do not:

- resume mascot rig implementation;
- redesign native runtime architecture;
- modify product UI;
- add CI;
- add 720p quality experiments;
- download every model found on Hugging Face;
- build a general generative-media framework;
- create an opaque aggregate "quality score";
- turn ComfyUI into project infrastructure.

## Universal gates

Use the canonical Mascot Universal Gates 1-20 in `docs/QUALITY_GATES.md`.

Do not create another numbered gate taxonomy.

Final report must list Gates 1-20 as PASS / FAIL / N/A with concise evidence. N/A needs a changed-cone rationale.

For this task, visual/model quality findings do not need to prove the generative model is good; they need to prove the benchmark measured and documented its behavior honestly.

## Final report

Return:

1. starting branch + HEAD;
2. final branch + HEAD;
3. Windows GPU/driver/CUDA/Python/PyTorch environment;
4. benchmark CLI architecture;
5. exact candidate research summary;
6. models/checkpoints actually installed;
7. model cache location/policy;
8. benchmark profiles;
9. input preprocessing contract;
10. successful and failed model runs;
11. VRAM/time table;
12. visual findings per successful model;
13. generated evidence paths;
14. dependency changes;
15. known limitations;
16. Gates 1-20 PASS/FAIL/N/A;
17. native Windows Git clean/push confirmation.

End with exactly one of:

    MASCOT_I2V_BENCH_001_COMPLETE

or

    MASCOT_I2V_BENCH_001_BLOCKED: <reason>
