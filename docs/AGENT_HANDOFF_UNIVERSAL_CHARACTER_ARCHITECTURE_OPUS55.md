# Agent handoff — Universal character pipeline architecture (Opus 5.5)

Task ID: MASCOT-CHAR-ARCH-004

## Goal

Design the reusable character-authoring + rig-profile architecture so Mascot is not tied to the current animal character.

The user's explicit product requirement is:

> We must be able to replace the current ferret/beaver-like mascot with a robot or another character without rebuilding the animation framework.

The intended authoring experience is:

    reusable dummy pose
        ->
    user gives dummy pose + new character brief/reference to ChatGPT/image generation
        ->
    new character is drawn over the exact pose
        ->
    project tooling/agent prepares it into a rig-compatible CharacterPack
        ->
    existing animation framework cuts/meshes/weights/validates it
        ->
    reusable animations work with minimal character-specific adjustment

This task defines that architecture.

It does NOT implement the production pipeline yet.

## Repository / workspace

Repository:

    git@github.com:rceman/mascot.git

Native Windows workspace:

    W:\devin_folder\mascot-rig-v02

Branch:

    agent/windows-rig-animation-lab-v0.2-opus55

Current research result is committed at:

    7d635654d4710228a5915568445fdcfdf7db947e

Fetch/pull the latest remote branch before starting because Planner documents may be added after that commit.

Native Windows Git + SSH is authoritative.

Do not use CI.

Preserve the 116 paused implementation worktree entries. Do not reset/clean/delete them.

## Read first

Read in full:

1. `AGENTS.md`
2. `docs/QUALITY_GATES.md`
3. `docs/MASCOT_RIG_ART_CONSTRUCTION_RESEARCH_V0.1.md`
4. `docs/MASCOT_RIG_ART_CONSTRUCTION_SPEC_V0.1.md`
5. `docs/MASCOT_NEUTRAL_BIND_POSE_RESEARCH_SPEC_V0.1.md`
6. `docs/MASCOT_UNIVERSAL_CHARACTER_PIPELINE_V0.1.md`
7. relevant existing rig/animation/runtime schema and code, read-only.

Planner decisions in `MASCOT_UNIVERSAL_CHARACTER_PIPELINE_V0.1.md` supersede the research report's D1 recommendation where they differ.

## Planner decision to preserve

The first reusable profile will NOT bind at:

- strict front-facing T-pose;
- canonical seated/laptop pose.

Use a:

    relaxed right-facing 3/4 neutral bind pose

The exact dummy geometry is your design deliverable.

The current generated neutral T-pose remains a non-canonical anatomy/research reference only.

## Universal requirement

Do not design an "otter rig".

Design a reusable character system.

The current mascot and a future friendly robot must both be able to use the same first profile if their anatomy fits the biped profile.

No production runtime path may require names/logic such as:

    otter
    beaver
    muzzle
    fur
    mascot_tail_is_required

Character identity belongs in data/art.

## Required architecture

Design these contracts:

### 1. RigProfile

First profile:

    biped-3q-v1

Define:

- canonical semantic bone roles;
- hierarchy;
- normalized bind landmarks;
- view/orientation semantics;
- left/right vs near/far mapping;
- required/optional slots;
- optional appendage/socket model;
- constraints;
- profile-level QA landmarks;
- retargeting semantics;
- dummy guide ownership/versioning.

### 2. Dummy pose / generation guide

Define the exact reusable dummy pose that the user can give to ChatGPT/image generation for a new character.

It must be:

- 1:1;
- non-species-specific;
- right-facing 3/4;
- relaxed/A-pose-like;
- arms clear of torso;
- legs clear of each other;
- all core joints visible;
- prop-free;
- shading-free;
- safe-margin aware;
- landmarked in normalized coordinates;
- suitable for animals and robots with the same biped topology.

Design the exact normalized landmark set and recommended body envelope.

Do not create the final dummy image in this task unless explicitly instructed. The Planner/user will use the resulting spec to generate/draw it.

### 3. CharacterSource

Define the contract for one generated/artist-created character over the dummy.

The source must remain easy to prepare into articulated art.

Specify:

- required canvas/orientation;
- visibility rules;
- occlusion rules;
- shading restrictions;
- outline/line assumptions;
- optional appendage rules;
- acceptable proportion deviation;
- landmark validation;
- provenance/hashes.

### 4. CharacterPack

Define runtime-ready derived data.

Specify the minimum data needed for:

- textures;
- semantic parts;
- meshes;
- weights;
- line classes;
- material/shading parameters;
- slots/z;
- optional appendages;
- contact-line policies;
- character-specific joint limits;
- QA anchors.

Avoid duplicate canonical representations.

### 5. Animation retargeting

Define how one semantic animation can operate on different CharacterPacks.

Include:

- semantic bone roles;
- normalized/relative transforms;
- proportion scaling;
- range clamping;
- optional roles;
- clip capability requirements;
- character-specific override policy;
- failure behavior.

### 6. Character onboarding/build pipeline

Design the deterministic/semi-automatic pipeline from a 1:1 generated source image to CharacterPack.

Target:

    validate
      -> segment/classify
      -> remove/classify external vs intrinsic lines
      -> generate parts/masks
      -> generate local meshes
      -> initialize weights
      -> bind semantic roles
      -> build material/contact metadata
      -> run standard stress poses
      -> visual QA
      -> pack

Identify what can be automatic now and what needs an agent/user decision.

Do not hide artistic uncertainty behind heuristics.

## Reuse current research

Preserve conclusions that remain useful:

- external contour is runtime/composition-derived;
- continuous anatomy uses weighted local mesh where rigid pieces fail;
- contact/separation lines are semantically owned;
- intrinsic line art follows its owning surface;
- directional shading is a runtime concern for moving/deforming regions;
- laptop is a separate prop;
- existing `Attachment::Mesh` / `skin_mesh` are valuable;
- renderer mesh support is still missing;
- current seated decomposition/junction heuristics are not the future source pipeline.

## Generalization rule

Use this test continuously:

> If the current animal were replaced today by a friendly robot drawn over the same dummy pose, would this contract/code concept still make sense unchanged?

If not, move that assumption into CharacterPack/profile data or report why it genuinely belongs in the generic runtime.

## Do not over-generalize

The first implemented profile is still:

    biped-3q-v1

Do not build quadruped/multi-arm/floating-character implementations now.

The schema/runtime design should be extensible enough for future profiles without hardcoding biped assumptions into rendering/animation primitives.

## Required output

Create:

    docs/MASCOT_UNIVERSAL_CHARACTER_ARCHITECTURE_V0.1.md

It must contain at minimum:

1. executive architecture;
2. terminology and authority;
3. `biped-3q-v1` semantic skeleton;
4. exact dummy pose specification;
5. normalized landmark table;
6. required/optional slots and sockets;
7. CharacterSource contract;
8. CharacterPack contract;
9. animation retargeting contract;
10. optional appendage model;
11. prop/contact model;
12. line/shading/material ownership;
13. source-to-pack build pipeline;
14. automatic vs agent/user-owned steps;
15. validation/QA invariants;
16. versioning/hard-cut policy;
17. mapping from current mascot runtime/types to generic equivalents;
18. robot thought experiment showing the same pipeline works;
19. first bounded implementation slice;
20. open Planner/user decisions.

Also add an **Architecture Compatibility Addendum** to:

    docs/MASCOT_RIG_ART_CONSTRUCTION_SPEC_V0.1.md

Only add rules that are compatible with the universal character direction.

If an existing agent-authored rule conflicts — especially the seated-bind / mandatory attachment-substitution recommendation — report the conflict and propose the corrected rule, but do not silently rewrite Planner-owned history.

## First implementation slice proposal

Your report must propose the smallest implementation slice that proves universality.

Strong candidate:

    RigProfile schema + biped-3q-v1
        +
    dummy landmark validation
        +
    one current-mascot CharacterSource
        +
    one synthetic/simple robot CharacterSource
        ->
    both validate against the same profile

No full animation implementation is required in this architecture task.

Do not implement the slice until Planner approval.

## No production implementation

Research/design/read-only code inspection only.

Do not modify:

- Rust runtime behavior;
- renderer;
- rig assets;
- production art;
- meshes;
- clips;
- pivots;
- safe ranges;
- shading runtime;
- physics.

After documents are complete, commit/push and STOP.

## Universal Gates

Use Mascot Universal Gates 1–20.

For this architecture-only task, implementation gates may be N/A with changed-cone rationale.

Gate 1 must prove all required architecture outputs exist.

Gate 2 must surface unresolved semantic/art decisions.

Gate 4 must prove no implementation leaked.

Gate 6 must prove generic vs profile vs character ownership is coherent.

Gate 7 must prove no duplicate canonical data is introduced.

Gate 9 must prove stable semantic contracts.

Gate 17 must challenge animal-specific assumptions and sunk-cost reuse.

Gate 20 must include a whole-design "robot substitution" audit.

## Final report

Return:

1. starting HEAD;
2. final HEAD;
3. generic architecture summary;
4. `biped-3q-v1` skeleton summary;
5. dummy pose/landmark summary;
6. CharacterSource contract;
7. CharacterPack contract;
8. retargeting model;
9. onboarding automation boundary;
10. robot substitution audit;
11. current-code reuse/generalization map;
12. proposed first implementation slice;
13. open decisions;
14. files changed;
15. confirmation no production implementation occurred;
16. Gates 1–20 PASS/FAIL/N/A;
17. native Windows Git push/clean-status report.

End with exactly:

    MASCOT_CHAR_ARCH_004_COMPLETE

or:

    MASCOT_CHAR_ARCH_004_BLOCKED: <reason>
