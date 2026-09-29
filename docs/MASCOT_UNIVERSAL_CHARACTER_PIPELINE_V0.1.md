# Universal Character Art + Rig Pipeline v0.1

**Status:** authoritative Planner direction after MASCOT-RIG-RESEARCH-003  
**Scope:** architecture / authoring contract; implementation remains paused until a follow-up task authorizes a bounded slice  
**Primary goal:** Mascot must become a reusable character-animation framework, not an otter/beaver-specific rig.

## 1. Product direction

The current mascot is the first character, not the architecture.

The framework MUST make it practical to replace the current animal character later with another character — for example a robot — without redesigning the renderer, animation runtime, authoring pipeline, or clip model.

Species-specific or character-specific assumptions MUST live in character data/assets, not in generic runtime code.

The generic system should support:

    reusable rig profile
        +
    character source art
        +
    generated/verified character pack
        +
    reusable animation clips / constraints
        =
    animated character

The current `assets/mascot.png` remains the canonical identity/style reference for the current mascot only.

## 2. Planner decisions after research

The following resolve the open decisions from `MASCOT_RIG_ART_CONSTRUCTION_RESEARCH_V0.1.md`.

### D1 — bind-pose family

Do **not** use:

- a strict front-facing T-pose;
- the canonical seated/laptop pose as the universal bind pose.

For the first reusable biped profile, use a:

    relaxed right-facing 3/4 neutral bind pose

Requirements:

- square 1:1 canvas;
- characteristic 3/4 view;
- head/body view consistent;
- arms separated from torso;
- elbows relaxed rather than locked straight;
- legs separated;
- knees slightly relaxed;
- hands/paws clear of torso;
- feet fully visible;
- no laptop or prop occlusion;
- complete neck/shoulders/hips/tail-root/socket regions;
- enough negative space around limbs for generated character variation.

The exact dummy geometry/landmarks are an architecture deliverable of the next task.

### D2 — shading

Directional lighting/shading belongs in runtime for deforming/moving regions.

Authored source art owns:

- base albedo/local colour;
- identity markings;
- intrinsic line art;
- non-directional material identity.

Runtime owns, where applicable:

- stylized directional highlight/shadow;
- final external silhouette outline;
- contact/separation lines;
- dynamic contact/AO where needed.

A small amount of non-directional local AO or material texture may remain authored if it does not become visually wrong when the part rotates.

### D3 — hidden anatomy

Do not invent mascot-specific hidden anatomy as a generic rule.

The current generated neutral reference may suggest anatomy but is not authority.

For every character, hidden/complete anatomy is part of that character's source/pack and must be validated against the character design.

For the current mascot, do not introduce an unapproved cream belly or other new identity feature merely because a generated reference contains it.

### D4 — fidelity metrics

Use the multi-metric verification approach from the research:

- silhouette;
- named landmarks;
- colour/material regions;
- intrinsic line art;
- perceptual similarity;
- visual review.

Do not freeze universal numeric thresholds until the first purpose-built character-source prototype exists.

### D5 — art ownership

The production character source is a distinct art-authoring output.

The current generated T-pose is research material, not production art.

A future ChatGPT/image-generation step may create candidate character source art from the reusable dummy pose, but the result must pass the character-source contract before it becomes a CharacterPack input.

### D6 — shader build

If custom HLSL is adopted:

- HLSL source is canonical;
- shader build is deterministic;
- compiled shader bytecode may be checked in as a generated artifact when that materially simplifies native deployment;
- runtime shader compilation is not required for normal product startup.

### D7 — prototype tooling

One-off research code may remain isolated/throwaway.

Any capability needed by repeated character onboarding, art preparation, rig generation, mesh generation, inspection, or QA must become durable project-owned tooling.

## 3. Universal architecture

Use four conceptual layers.

### 3.1 RigProfile

A reusable articulation contract independent of character identity.

Example first profile:

    biped-3q-v1

A RigProfile defines:

- semantic bone roles;
- default hierarchy;
- normalized bind-pose landmarks;
- attachment/slot roles;
- view/orientation contract;
- optional socket roles;
- constraint roles;
- default mesh/deformation policy;
- retargeting semantics;
- QA landmarks;
- dummy/guide assets.

It MUST NOT contain otter/beaver-specific palette, fur, muzzle, tail shape, laptop geometry, etc.

### 3.2 CharacterSource

The approved art source for one character identity.

The first intended authoring workflow is:

    user chooses a RigProfile dummy pose
        ->
    user supplies the dummy pose + character description/reference to ChatGPT
        ->
    image generation draws the new character over the exact dummy pose
        ->
    source candidate is validated against the RigProfile
        ->
    approved CharacterSource

A CharacterSource is still authoring material, not runtime-ready data.

### 3.3 CharacterPack

Derived runtime-ready character data.

Conceptually contains:

- profile id/version;
- source provenance/hash;
- character metadata;
- semantic parts;
- textures/albedo;
- intrinsic-line layers;
- masks;
- meshes;
- weights;
- slots/z-order;
- optional appendages;
- material/shading parameters;
- contact-line policies;
- character-specific constraints/limits;
- QA landmarks.

Generation SHOULD be deterministic wherever possible.

### 3.4 Animation/runtime

Animation clips operate on semantic rig roles, not on "otter" part names.

The runtime consumes:

    RigProfile-compatible CharacterPack
        +
    semantic animation clip
        +
    optional prop/contact state

The animation and renderer code MUST NOT need a new implementation merely because the character art changes from animal to robot.

## 4. First reusable profile: biped-3q-v1

The first universal profile should cover the current mascot and a humanoid/robot-like replacement.

### 4.1 Core bone roles

Use generic semantic roles similar to:

    root
    pelvis
    spine
    chest
    neck
    head

    shoulder.left
    upper_arm.left
    lower_arm.left
    hand.left

    shoulder.right
    upper_arm.right
    lower_arm.right
    hand.right

    upper_leg.left
    lower_leg.left
    foot.left

    upper_leg.right
    lower_leg.right
    foot.right

The exact naming is a task deliverable, but code MUST NOT bake in animal-specific names.

### 4.2 View roles

Anatomical left/right and visual near/far are different concepts.

The profile should preserve anatomical/logical side identity and separately map each side to a bind-view role:

    near
    far

This prevents animation semantics from depending on one drawing orientation.

### 4.3 Optional appendages

Do not put tail/ears/antennae/wings into the mandatory core skeleton.

Use optional typed appendage chains / sockets, for example:

    appendage.tail
    appendage.ear.left
    appendage.ear.right
    appendage.antenna.*
    appendage.wing.*

A robot may omit tail and ears while using antennae.

A future character may add other appendages without changing generic animation/rendering code.

### 4.4 Props

Props are separate rigs/entities attached through semantic mount/contact sockets.

Examples:

    prop.hand.left
    prop.hand.right
    prop.chin
    prop.foot.left
    prop.foot.right

The laptop is a prop for the current mascot, not part of the universal body skeleton.

## 5. Universal dummy pose

The project needs a reusable **dummy pose / character-generation guide**.

This dummy is not the mascot and should not carry a species identity.

Suggested asset family:

    assets/character-templates/biped-3q-v1/
      dummy-guide.png
      dummy-landmarks.json
      dummy-mask.png          # only if useful
      profile.json
      README.md

### 5.1 Purpose

The user should be able to take:

    dummy-guide.png

and give it to ChatGPT/image generation with a character brief such as:

    "Draw a compact orange beaver in exactly this pose"

or:

    "Draw a small friendly white service robot in exactly this pose"

The generated character should then be much easier for the art pipeline/agent to prepare because:

- pose is known;
- joints are known;
- landmarks are known;
- limbs are separated;
- body is not hidden by props;
- orientation is fixed;
- canvas is standardized;
- semantic regions can be inferred from the template.

### 5.2 Dummy design requirements

The dummy MUST:

- be 1:1;
- use normalized landmark coordinates independent of raster size;
- be right-facing 3/4;
- be visually neutral/non-species-specific;
- show all core limbs clearly;
- keep limbs separated enough for reliable segmentation;
- avoid self-occlusion where possible;
- keep joints anatomically readable;
- leave safe margin around the character;
- contain no prop;
- contain no directional shading;
- expose optional socket locations;
- use labels/shapes/landmarks that do not rely on colour alone.

The guide can be rendered at multiple resolutions, but geometry/landmarks are defined in normalized coordinates.

### 5.3 Dummy is not a character texture

The runtime never ships the dummy.

It is an authoring/conditioning guide.

Generated characters may have substantially different:

- silhouette;
- proportions within allowed envelopes;
- materials;
- face;
- ears;
- tail;
- mechanical vs organic construction.

But they must preserve the profile's required semantic anchors and articulation viability.

## 6. Character-generation source contract

A generated character candidate intended for automatic/semi-automatic rig preparation should follow these rules.

### Required

- 1:1 square canvas;
- exact profile orientation;
- one full character;
- no crop;
- no foreground prop;
- all required core limbs visible;
- arms separated from torso;
- legs separated;
- hands/feet readable;
- no limb crossing another required limb;
- no cast ground shadow baked into the character;
- no cinematic background;
- no perspective change from the dummy;
- profile landmarks remain within allowed tolerance;
- no directional baked shading that will become invalid under rotation;
- consistent material/albedo regions.

### Strongly preferred

- transparent or simple uniform background;
- simple, uniform external outline if an outline is present;
- intrinsic details clearly separated from the outer silhouette;
- optional appendages positioned so their roots are visible;
- high enough resolution that part extraction and local meshes are stable.

### Character identity may vary freely

- animal vs robot;
- fur vs metal;
- muzzle vs faceplate;
- ears vs antennae;
- tail present/absent;
- colour palette;
- markings;
- eye design;
- limb proportions within profile constraints.

## 7. Character onboarding pipeline

Target onboarding flow:

    1. choose RigProfile
    2. take dummy-guide.png
    3. create candidate art with image generation / artist
    4. validate pose + landmarks + visibility
    5. classify semantic regions
    6. derive albedo/intrinsic-line masks
    7. generate/author local meshes + weights
    8. bind attachments to semantic roles
    9. generate contact/outline/material metadata
    10. run static articulation sweeps
    11. repair only character-specific failures
    12. generate CharacterPack
    13. run standard reusable animation/QA suite
    14. approve character

The framework should make steps 4–13 increasingly automated.

## 8. Do not repeat the flattened-seated-art problem

The dummy/source contract is intentionally designed so a new character does not begin as a heavily occluded pose.

Do not interpret "easy to cut" as permission to return to arbitrary flat-image chopping.

The neutral source reduces ambiguity, but continuous anatomy still requires the correct representation:

- rigid part where appropriate;
- weighted local mesh for shoulder/hip/torso/tail-root flows;
- explicit line ownership;
- runtime outline;
- runtime directional shading.

Automatic segmentation is an onboarding aid, not the animation model.

## 9. Animation retargeting

Reusable clips should target semantic roles in a RigProfile.

Examples:

    idle
    blink
    look
    head_tilt
    wave
    reach
    sit
    stand
    walk
    drag_react

A character may define:

- role availability;
- range limits;
- retarget scale;
- optional clip overrides;
- optional substitute art for special states.

A missing optional appendage should not make a generic clip invalid unless that clip explicitly requires it.

The goal is that a robot CharacterPack can consume the same core idle/head/body/arm clips as the current mascot after retargeting.

## 10. Universal authoring invariants

The eventual framework SHOULD validate at minimum:

- required semantic bones exist;
- normalized bind landmarks are present;
- character source matches the selected profile/orientation;
- required parts are visible and separable;
- optional appendages declare sockets;
- weights normalize and remain bounded;
- no mesh foldover in verified range;
- no raw crop edge becomes visible;
- no unowned dark/contact line exists;
- runtime silhouette remains continuous;
- source contains no unsupported baked directional lighting;
- standard profile stress poses render;
- standard reusable clips retarget successfully.

## 11. What is character-specific vs framework-generic

### Framework-generic

- RigProfile schema;
- skeleton evaluation;
- clip evaluation;
- mesh skinning;
- rendering;
- runtime outline;
- runtime directional shading model;
- contact-line engine;
- prop/contact system;
- character-pack loader;
- retargeting;
- QA commands;
- dummy/profile tooling.

### Character-specific

- art;
- proportions;
- optional appendages;
- meshes/weights where topology differs;
- material parameters;
- exact safe ranges;
- intrinsic lines;
- contact policies;
- special pose/attachment substitutions when genuinely needed.

A new character SHOULD require data/art work, not new renderer/animation architecture.

## 12. Extensibility beyond the first biped profile

Do not over-generalize the first implementation into an abstract creature engine.

Build `biped-3q-v1` cleanly first.

The generic runtime/schema must not prevent later profiles such as:

    quadruped-3q-v1
    floating-character-v1
    multi-arm-v1

but those profiles do not need implementation now.

Universal means reusable contracts and data-driven semantics, not speculative implementation of every possible anatomy.

## 13. Next Opus task

The next Opus task is architecture/specification work, not production implementation.

Opus must:

1. review this Planner direction against the completed research;
2. design `biped-3q-v1`;
3. define the exact dummy pose geometry and normalized landmarks;
4. define the minimum RigProfile schema;
5. define CharacterSource and CharacterPack contracts;
6. define semantic retargeting rules;
7. define optional appendage/socket semantics;
8. define the deterministic onboarding/build pipeline;
9. identify which current runtime types can be reused vs generalized;
10. define one bounded first implementation slice.

After producing the design, STOP for Planner/user review before implementation.
