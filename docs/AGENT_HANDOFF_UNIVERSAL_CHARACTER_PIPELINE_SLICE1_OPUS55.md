# Agent handoff — Universal Character Pipeline v0.1, Slice 1 (Opus 5.5)

Task ID: MASCOT-CHAR-PIPE-005

## Goal

Implement the first bounded, reusable universal-character pipeline slice.

This is the first code implementation after the research/architecture phases.

The slice must prove:

    one RigProfile
        ->
    deterministic dummy guide
        ->
    one combined reference board (dummy + style ref)
        ->
    one 1:1 CharacterSource contract
        ->
    profile-driven validation

for both:

- a mascot-like character fixture;
- a simple robot fixture;

with no character-specific branch.

## Repository / branch / workspace

Repository:

    git@github.com:rceman/mascot.git

Work only on:

    agent/universal-character-pipeline-v0.1-opus55

This branch was created from:

    731749c4f06a45aacd54cc1a085e292b5428cd6e

Use a NEW isolated native Windows workspace:

    W:\devin_folder\mascot-character-pipeline-v01

Do NOT implement this slice inside:

    W:\devin_folder\mascot-rig-v02

That older workspace contains 116 intentionally preserved paused implementation entries.

Native Windows Git + SSH is authoritative.

Do not use CI.

## Bootstrap

From native Windows PowerShell/Git Bash as appropriate:

    cd W:\devin_folder
    git clone git@github.com:rceman/mascot.git mascot-character-pipeline-v01
    cd mascot-character-pipeline-v01
    git fetch origin
    git checkout agent/universal-character-pipeline-v0.1-opus55
    git pull --ff-only origin agent/universal-character-pipeline-v0.1-opus55

If the directory already exists, inspect it first. Do not delete/reset an unknown worktree.

## Read first

Read in full:

1. `AGENTS.md`
2. `docs/QUALITY_GATES.md`
3. `docs/MASCOT_UNIVERSAL_CHARACTER_PIPELINE_V0.1.md`
4. `docs/MASCOT_UNIVERSAL_CHARACTER_ARCHITECTURE_V0.1.md`
5. `docs/MASCOT_UNIVERSAL_CHARACTER_DECISIONS_V0.2.md`
6. relevant existing `mascotctl`, animation schema, image/asset tooling, tests, and architecture docs.

Planner decisions in `MASCOT_UNIVERSAL_CHARACTER_DECISIONS_V0.2.md` override unresolved U1–U9 architecture choices and any conflicting agent recommendation.

## Critical generation-input contract

User-facing image-generation input is exactly:

    ONE combined reference board image
        +
    ONE desired-character text prompt

The board contains:

    LEFT  = dummy pose/rig guide
    RIGHT = style/identity reference

The generated output is:

    ONE 1:1 assembled CharacterSource image

Do not require the user to produce a second exploded sheet, layers, sprites, or frames.

The exploded view, if useful, is a later derived authoring/QA artifact.

## Implementation scope

Implement the smallest coherent version of:

### A. RigProfile schema

Implement a versioned profile schema sufficient to represent `biped-3q-v1`:

- profile id/version;
- semantic roles/hierarchy;
- normalized dummy landmarks;
- canvas/view/orientation contract;
- required vs optional roles;
- sockets;
- envelope/clearance constraints;
- pose-validation constraints;
- enough data to deterministically render the dummy guide.

Avoid copying derived bone transforms into multiple canonical files.

### B. `biped-3q-v1`

Encode the architecture-approved v0 dummy geometry from:

    docs/MASCOT_UNIVERSAL_CHARACTER_ARCHITECTURE_V0.1.md

Treat it as a prototype profile whose visual result is subject to Planner/user review.

Do not silently change the normalized geometry merely because another pose "looks better".

If implementation reveals a contradiction, stop and report it.

### C. Deterministic dummy-guide renderer

Produce at minimum:

    dummy-generation.png
    dummy-annotated.png

A mask/semantic guide may also be generated if it has a clear validator/tooling use.

Requirements:

- geometry comes from RigProfile data;
- output is deterministic;
- 1:1;
- no character identity;
- no directional shading;
- all core limbs/joints readable;
- generation guide contains no unnecessary text labels that image generation may copy;
- annotated view may contain landmarks/ids for review.

Record hashes in evidence/tests.

### D. Combined reference board

Add a deterministic command/capability that takes:

    RigProfile dummy-generation guide
    +
    style reference image

and produces one combined board.

v0 layout:

    2048x1024
    LEFT 1024x1024 = pose guide
    RIGHT 1024x1024 = style reference

Rules:

- no stretch;
- deterministic fit/pad;
- fixed left/right meaning;
- neutral background;
- source/profile hashes in a sidecar receipt/manifest rather than visible text when practical;
- board is derived, not canonical.

Use `assets/mascot.png` as a style-reference test input where appropriate, but do not interpret its seated pose as the generated pose authority.

### E. CharacterSource schema/manifest

Define the minimum manifest/provenance for:

- character id;
- profile id/version;
- source image path/hash;
- combined-board provenance/hash if generated through this workflow;
- textual brief/prompt hash or recorded text;
- landmark annotation ownership/status;
- accepted deviations.

Do not require a second exploded source image.

### F. Character validation

Implement a narrow command such as:

    mascotctl character validate ...

or another existing-command-compatible equivalent.

Validate at minimum:

- 1:1 source;
- profile id/version;
- required landmark annotation presence;
- role topology;
- pose-angle constraints;
- facing direction;
- side ordering;
- limb visibility/clearance where data permits;
- envelope/safe-margin constraints;
- required source provenance fields.

Separate:

- FAIL = structurally incompatible;
- WARN = reviewable deviation.

Do not make opaque heuristic inference canonical.

### G. Universality fixtures/tests

Create:

1. mascot-proportioned positive fixture;
2. simple robot positive fixture;
3. negative fixtures for at least:
   - wrong aspect ratio;
   - wrong facing/pose;
   - missing required role/landmark;
   - crossed/insufficiently separated limbs or another clear topology/clearance failure.

Both positive fixtures MUST pass exactly the same `biped-3q-v1` profile without special-casing by character id/species/material.

Add a test/audit that rejects animal-specific branching in the newly introduced generic pipeline code where practical.

## Do NOT implement in this slice

Do not implement:

- actual image-generation API calls;
- automatic character drawing;
- production segmentation;
- automatic hidden-art synthesis;
- production exploded-sheet reconstruction;
- production mesh generation;
- D3D11 textured-mesh rendering;
- runtime outline changes;
- runtime shading;
- contact-line rendering;
- clip 0.4 runtime migration unless absolutely necessary;
- IK;
- physics;
- active ragdoll;
- CharacterPack runtime integration;
- PNG sprite-sheet animation.

This slice is profile + guide + board + source contract + validation.

## Important animation/physics direction

Do not introduce any assumption that character animation will be baked to frames.

The future runtime remains layered skeletal/deformable animation.

Also preserve the future evaluation shape:

    animation intent
        -> constraints / IK targets
        -> physical pose solver
        <-> contacts / joint constraints
        -> final bone pose
        -> skinning
        -> rendering

No physics implementation now.

## Retarget-policy correction

Do not encode "scale every track down until it fits" as the universal default.

The planned closed policy list is:

    preserve
    clamp
    scale_group
    solve_target
    unavailable

You only need to encode this in a schema now if it naturally belongs in the slice. Do not implement clip retargeting merely to support the enum.

## Dependencies / implementation style

Prefer existing Rust/project libraries where they already solve:

- PNG decode/encode;
- hashing;
- JSON/schema parsing;
- CLI wiring.

Do not add a heavyweight GUI/image framework for this task.

No browser/runtime/webview dependency.

Keep the implementation native, deterministic and agent-testable.

## Evidence

Commit bounded evidence proving:

- profile parses/validates;
- dummy-generation render hash is deterministic;
- annotated guide generated;
- combined board generated using a style reference;
- board receipt records both input hashes/profile;
- mascot-like positive fixture passes;
- robot positive fixture passes;
- negative fixtures fail for expected reasons;
- no animal-specific branch is needed.

Do not commit giant temporary images or build caches.

## Universal Gates

Use Universal Gates 1–20.

Especially:

- Gate 1: every slice acceptance item satisfied;
- Gate 2: contradictions/open semantic decisions surfaced;
- Gate 4: exact-scope proof;
- Gate 6: generic/profile/character ownership remains separated;
- Gate 7: no duplicated canonical profile geometry;
- Gate 9: stable schema/role contracts;
- Gate 13: bounded dependency additions;
- Gate 16: deterministic hashes/validation evidence;
- Gate 17: no sunk-cost reuse of seated decomposition;
- Gate 20: robot and mascot-like fixtures use the exact same path.

## Final report

Return:

1. starting branch + HEAD;
2. final branch + HEAD;
3. Windows toolchain;
4. files/commits;
5. RigProfile schema summary;
6. `biped-3q-v1` profile path;
7. dummy guide outputs + hashes;
8. combined-board command + output/receipt;
9. CharacterSource schema;
10. validator command + validations;
11. mascot-like fixture result;
12. robot fixture result;
13. negative fixture results;
14. proof of no character-specific branch;
15. dependency changes;
16. known limitations/open decisions;
17. Gates 1–20 PASS/FAIL/N/A;
18. native Windows Git clean/push confirmation.

End with exactly:

    MASCOT_CHAR_PIPE_005_COMPLETE

or:

    MASCOT_CHAR_PIPE_005_BLOCKED: <reason>
