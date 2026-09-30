# Mascot Universal Character System — Planner Synthesis Before Final Plan v0.4

**Status:** Planner synthesis for Opus review only  
**Purpose:** collect the current architecture ideas into one coherent proposal, explicitly supersede discarded directions, and ask Opus 5.5 to review the whole system before Planner/user freeze the final implementation plan.  
**Implementation authorization:** NONE. Do not implement from this document.

---

## 1. Core product goal

Mascot is not an otter/beaver-specific animation system.

The target is a reusable native Rust/Windows character framework where a new character — for example:

- current animal mascot;
- friendly service robot;
- anthropomorphic turtle;
- fox-like biped;
- another compatible biped character;

can be onboarded mostly as **art/data**, without redesigning the renderer, skeleton evaluator, animation runtime, constraints, physics architecture, or QA pipeline.

The first reusable profile remains:

    biped-3q-v1

Future profiles may exist, but do not implement them now.

---

## 2. Major simplification: one CharacterSource is the visual source of truth

The most important current direction is:

> The character-rigging pipeline begins from ONE approved assembled 1:1 CharacterSource image in a clean neutral articulated pose.

Do NOT require the user to provide:

- a rig board;
- a bones image;
- an exploded image;
- a layered PSD;
- separate limb PNGs;
- a sprite sheet;
- multiple independently generated character views;
- animation frames.

Those may be derived later by tooling/agent workflow.

The canonical visual input is:

    CharacterSource.png

plus:

    RigProfile

Everything else is derived/proposed/approved from those inputs.

### Why

Multiple independently generated representations create contradictory visual truths:

- different arm lengths;
- different hidden anatomy;
- changed line thickness;
- shape drift;
- pose drift;
- mismatched shoulders/hips;
- inconsistent tail or ears.

The framework should solve rigging from one visual source rather than asking image generation to solve rigging.

---

## 3. Image generation is for character creation, not rigging

Image generation may be used to create the assembled neutral CharacterSource.

It is NOT the canonical mechanism for:

- bone placement;
- part cutting;
- exploded sheets;
- hidden-art completion;
- mesh topology;
- weights;
- pivots;
- rig.json / CharacterPack;
- animation frames.

The framework should not depend on a complex generation board.

A generation board may remain an **optional authoring aid** if useful, but it is not a mandatory CharacterSource contract and not part of runtime authority.

### Current preferred authoring interaction

Conceptually:

    user/reference/style brief
        -> image generation / artist
        -> ONE approved 1:1 neutral CharacterSource
        -> framework/agent rigging pipeline

For the current mascot:

    assets/mascot.png

remains the identity/style reference, but a new neutral production CharacterSource must be created/approved.

---

## 4. Neutral source pose

The first profile still uses a relaxed right-facing 3/4 neutral bind/source pose.

Preferred properties:

- 1:1 canvas;
- right-facing 3/4;
- head and torso share the same view;
- arms separated from torso;
- elbows relaxed;
- legs separated;
- knees relaxed;
- hands/paws visible;
- feet visible;
- no laptop/prop occlusion;
- complete silhouette;
- enough margin for limbs/tail;
- no baked directional lighting that becomes incorrect under rotation.

Do not require a strict front-facing T-pose.

The existing generated neutral mascot reference remains research material until explicitly promoted.

---

## 5. RigProfile owns topology, not exact character appearance

The profile should declare generic articulation semantics such as:

    root
    pelvis
    spine
    chest
    neck
    head

    shoulder.left/right
    upper_arm.left/right
    lower_arm.left/right
    hand.left/right

    upper_leg.left/right
    lower_leg.left/right
    foot.left/right

with optional appendages/sockets such as:

    tail
    ears
    antennae
    wings
    accessories
    props

The profile provides:

- hierarchy;
- semantic roles;
- canonical orientation;
- expected pose family;
- normalized reference landmarks/envelopes;
- sockets;
- constraints;
- QA expectations;
- default deformation suggestions;
- clip semantic roles.

The profile does NOT require every character source to match one pixel-perfect mannequin.

A character source is fitted to the profile.

---

## 6. Character analysis begins with skeleton/landmark fitting

Given:

    CharacterSource
    +
    RigProfile

the tooling/Opus should:

1. inspect the character;
2. propose landmarks;
3. propose semantic bone placement;
4. validate topology/orientation;
5. surface uncertainty;
6. ask for user/Planner input only where visual semantics are ambiguous;
7. commit approved annotations.

Landmarks/annotations must be:

- explicit;
- inspectable;
- hash-bound to the exact source image;
- reviewable;
- not hidden heuristic authority.

Automatic detection can propose, but must not silently become canonical.

---

## 7. Critical rule: bone does NOT imply a cut

This is a primary architectural rule.

A skeleton may contain:

    shoulder -> upper_arm -> lower_arm -> hand

while the art representation may still be:

    ONE continuous arm mesh

weighted across all four bones.

Therefore:

> Do not cut artwork merely because a bone boundary exists.

The representation is chosen per visual region.

---

## 8. Representation-selection principle

For every body region, use:

> **the least fragmented representation that passes the required articulation range and visual QA.**

Preferred escalation order:

    1. continuous weighted mesh
    2. continuous mesh + rigid terminal attachment(s)
    3. multiple overlapping meshes/attachments
    4. explicit rigid pieces with hidden overlap
    5. pose/state substitution only when genuinely required

Do NOT start from:

    one bone = one PNG

That approach caused the original cut-paper failures.

### Example: organic arm

A current mascot arm might remain one texture/mesh:

    shoulder
      |
    upper_arm
      |
    lower_arm
      |
    hand

Weights smoothly transition through the mesh.

If this passes:

- shoulder sweep;
- elbow sweep;
- combined shoulder+elbow poses;
- silhouette QA;
- line QA;
- stretch/foldover QA;

then no elbow cut is necessary.

### Example: robot

A robot may instead naturally use:

    rigid upper-arm plate
    hinge
    rigid forearm plate
    hinge
    rigid hand

The same RigProfile works.

The deformation class is character data, not engine branching.

---

## 9. Torso follows the same rule

Bones:

    pelvis
    spine
    chest

do not imply:

    pelvis.png
    spine.png
    chest.png

An organic character may use one torso mesh weighted across multiple bones.

This enables:

- lean;
- breathing;
- body sway;
- drag response;
- squash/compression;

without visible seams.

A robot may override this with rigid torso plates.

---

## 10. Segmentation/cutting is a derived proposal

After skeleton fit, tooling should propose semantic art regions.

It should not blindly segment every bone.

For each candidate region it must consider:

- is this one continuous visual surface?
- is there a true material/object boundary?
- does a joint need independent rigid motion?
- can a weighted mesh handle the required range?
- would splitting create a seam?
- does the line art imply separation?
- does the character design imply a hinge/socket?

The agent should generate a proposed decomposition map, render it, and review it.

---

## 11. Hidden geometry completion is downstream of representation choice

Once a region really requires separation or overlap, tooling/agent reconstructs hidden content.

Possible methods, in order of increasing ambiguity:

- extend nearby flat albedo/marking;
- deterministic geometric continuation;
- local clone/interpolation;
- agent-authored repaint;
- bounded local image-generation repair only when necessary.

Image generation may assist with local repair, but should not become the source of an independently generated whole-body exploded sheet.

Every hidden-art repair must remain tied to the original CharacterSource and reviewed in posed sweeps.

---

## 12. Exploded views are derived QA/authoring artifacts

An exploded view can be extremely useful, but it is not a user input.

It is derived from the approved character representation after:

- semantic regions;
- complete parts;
- meshes;
- hidden roots;
- line ownership;

have been decided.

It can show:

- isolated parts;
- hidden regions;
- bone ownership;
- mesh regions;
- overlap/root geometry;
- line classes.

It is evidence and an authoring surface, not a second source of identity.

---

## 13. Line ownership remains three-class

### A. External silhouette

Generated from the final composed/deformed fill silhouette.

Do not bake a complete external outline around every part.

### B. Contact/separation boundaries

For independently moving touching objects, for example:

- paw/laptop;
- chin/laptop;
- foot/laptop;
- mechanical plate contacts where stylistically required.

These need semantic pair policy and local mask/ownership.

Do not generate internal lines at every mesh/bone boundary.

### C. Intrinsic line art

Authored lines belonging to a surface:

- eyes;
- nose;
- mouth;
- whiskers;
- ear crease;
- robot panel details;
- logo/details.

They move/deform with the owning surface.

---

## 14. Directional shading should be runtime-owned

Source/prepared art should primarily own:

- albedo/base colour;
- identity markings;
- material texture that remains rotation-safe;
- intrinsic lines;
- local non-directional AO if safe.

Runtime should own pose-dependent directional shading for moving/deforming regions.

This avoids a baked highlight rotating to the physically/visually wrong side with a limb.

The engine should provide a small versioned set of generic stylized shading models.

Character data selects model + parameters.

Do not create per-character shader logic for normal onboarding.

---

## 15. CharacterPack remains runtime-ready derived data

Conceptually:

    CharacterSource
      + approved annotations
      + prepared art
      + approved meshes/weights
      + character recipe
      + RigProfile
        ->
    CharacterPack

CharacterPack includes/resolves:

- skeleton rest transforms;
- slots/z;
- textures;
- meshes;
- weights;
- limits;
- material parameters;
- line/contact policies;
- sockets;
- optional appendages;
- QA metadata.

Avoid duplicate canonical values.

Generated pack should carry provenance/input hashes.

---

## 16. Runtime animation is skeletal/deformable, not baked sprite-sheet animation

Normal character animation must remain runtime-evaluated:

    layered textures
      + skeleton
      + weighted meshes / rigid attachments
      + semantic clips
      + constraints
      + physics
        ->
    final posed frame

Do not bake normal animations into frame-by-frame PNG sprite sheets.

Sprite sheets may exist for bounded FX/fallback cases only.

---

## 17. Animation clips should be semantic/profile-based

Clips target semantic roles, not character-specific part names.

Retarget behavior must be explicit.

Do NOT silently scale an entire animation because one joint has a smaller range.

Candidate policies:

    preserve
    clamp
    scale_group
    solve_target
    unavailable

Examples:

- head tilt: clamp may be acceptable;
- coordinated tail motion: scale_group may be acceptable;
- reach/grab: solve_target through IK;
- impossible pose: unavailable.

Opus should review whether these are sufficient and how they belong in the final contract.

---

## 18. Constraints / IK / physics architecture

Future physics must not be modeled only as a cosmetic additive offset.

Preferred general evaluation concept:

    semantic animation intent
        ->
    constraint / IK targets
        ->
    physical pose solver
        <-> environment contacts / joint constraints
        ->
    final bone pose
        ->
    skinning
        ->
    rendering

A cheap tail/ear spring may still behave like a modifier.

A dragged/dangling mascot may instead require a stronger physical solve.

The architecture should support later:

- dragging root/body;
- inertia;
- gravity;
- springs/damping;
- foot/window contact;
- active-ragdoll-like blending;
- IK;
- prop contacts.

No full physics implementation is required in the immediate plan unless needed for an architecture seam.

---

## 19. Props are separate

Laptop and future held/contacted objects are separate props/rigs.

They attach through semantic sockets/constraints.

The body art under props must remain complete.

Do not bake laptop geometry into universal body anatomy.

---

## 20. Self-analysis / iterative rig-authoring loop

The agent should be expected to inspect what it produced and iterate.

For each region/joint:

    propose representation
        ->
    build candidate mesh/attachment
        ->
    assign weights/pivots
        ->
    render bind pose
        ->
    static joint sweeps
        ->
    combined multi-joint stress poses
        ->
    automatic metrics
        +
    visual inspection
        ->
    PASS or repair/escalate representation

The agent must not stop at:

    detector ERROR count = 0

Visual review remains mandatory.

### Inspect at minimum

- rest fidelity;
- silhouette continuity;
- raw cut exposure;
- line continuity;
- identity drift;
- mesh inversion/foldover;
- area stretch;
- anisotropic stretch;
- local thickness changes;
- paw/hand/foot distortion;
- shading correctness;
- hidden-art exposure;
- z/occlusion;
- contact-line behavior.

---

## 21. Representation escalation must be evidence-driven

Example arm decision loop:

    continuous arm mesh
        -> sweep
        -> PASS: keep it

or:

    continuous arm mesh
        -> excessive hand distortion
        -> keep upper/lower arm continuous mesh
           + rigid hand attachment
        -> sweep
        -> PASS

or:

    shoulder region still fails
        -> topology/weight refinement
        -> sweep

Only split further when evidence shows the less fragmented representation cannot satisfy motion/visual requirements.

This principle should be generic tooling behavior, not an otter-specific workflow.

---

## 22. Current reusable implementation/research

Opus should review actual repository state rather than assuming these are all correct.

Expected useful existing work includes:

- generic skeleton evaluation;
- clip evaluation;
- existing `Attachment::Mesh`;
- `skin_mesh`;
- runtime external outline approach;
- animation/QA tooling concepts;
- `biped-3q-v1` profile work;
- profile/source validation from MASCOT-CHAR-PIPE-005.

Known missing/unfinished areas include:

- production D3D11 textured-mesh render path;
- real CharacterSource -> semantic-region analysis;
- real mesh/weight authoring;
- hidden-art completion workflow;
- character-source-driven rig fitting;
- final CharacterPack pipeline;
- runtime shading;
- contact-line runtime;
- IK/physics.

The old seated decomposition/junction-repair work is evidence, not architecture authority.

---

## 23. Current Slice-1 board/dummy work must be reassessed

MASCOT-CHAR-PIPE-005 implemented:

- RigProfile;
- deterministic dummy guide;
- combined board;
- CharacterSource schema;
- source validation;
- synthetic mascot/robot fixtures.

The latest product direction no longer requires a complex generation board as the character-rigging input.

Opus must classify the Slice-1 board/dummy work:

    KEEP
    ADAPT
    OPTIONAL TOOLING
    RETIRE

Likely candidates:

- RigProfile/schema: KEEP/ADAPT.
- semantic role/profile validation: KEEP.
- source manifest/provenance: KEEP/ADAPT.
- deterministic dummy renderer: possibly OPTIONAL authoring/debug tooling.
- combined reference-board composer: possibly OPTIONAL user/imagegen convenience, not architecture dependency.
- synthetic fixtures: KEEP as generic tests if still useful.

Do not delete useful tooling merely because it is no longer mandatory.

Do not preserve it as mandatory merely because it exists.

---

## 24. Current mascot onboarding

For the current mascot, distinguish two references:

### Identity reference

    assets/mascot.png

This remains authoritative for:

- character identity;
- face;
- palette;
- line language;
- characteristic proportions/style;
- seated/laptop identity pose.

### Production neutral CharacterSource

A new approved 1:1 right-facing 3/4 neutral source suitable for rigging.

It should preserve identity from the canonical reference while satisfying the rig/source contract.

Once approved, rig construction derives from the neutral source, while identity QA still includes reconstruction of the canonical seated/laptop pose.

---

## 25. New-character onboarding target

A future character such as a robot should ideally require:

    1. create/approve neutral assembled CharacterSource
    2. choose compatible RigProfile
    3. analyze/fit landmarks
    4. approve skeleton
    5. choose representation by region
    6. derive prepared art / hidden completion
    7. generate/author meshes + weights
    8. run sweeps + QA
    9. build CharacterPack
    10. retarget standard semantic clips
    11. character-specific fixes only when evidence requires them

No renderer rewrite.

No robot-specific animation engine branch.

---

## 26. Desired autonomy for Opus/tooling

The goal is not merely to automate file conversion.

The authoring agent should be able to:

- inspect source art;
- reason about anatomy/material boundaries;
- propose bones;
- propose meshes;
- choose between rigid and weighted representations;
- run tests/render sweeps;
- inspect evidence;
- identify failures;
- revise topology/weights/cuts;
- repeat until it reaches the acceptance contract or hits an explicit ambiguity/blocker.

The user/Planner should be needed mainly for:

- identity/art-direction ambiguity;
- semantic ambiguity;
- accepting substantial visual tradeoffs;
- profile-level changes.

---

## 27. Questions Opus must review before final plan

Opus must review and answer at least:

1. Is ONE assembled CharacterSource + RigProfile sufficient as the primary onboarding input?
2. Which parts of current Slice-1 board/dummy work remain valuable?
3. Is `biped-3q-v1` currently too strict/pixel-pose-driven for source fitting?
4. What should be canonical vs derived in landmarks, prepared art, meshes, weights and CharacterPack?
5. How should the tool choose a representation per region?
6. Should a continuous mesh be the preferred organic default, or only one candidate?
7. What objective metrics are sufficient to decide when a mesh must be split?
8. How should line art be represented on deforming meshes?
9. How should hidden geometry be reconstructed without creating a second visual authority?
10. What authoring data format best supports agent iteration and human inspection?
11. What renderer work is minimally required before real mesh authoring is useful?
12. Should mesh skinning initially stay CPU-side or move GPU-side?
13. How should runtime outline operate over mixed sprite/mesh attachments?
14. What is the smallest runtime shading model that preserves current style?
15. How should contact-line generation interact with mesh deformation?
16. What needs to change in current clip schema/retargeting?
17. How should IK and future physics be staged without overbuilding now?
18. What existing paused WIN-002 work should be salvaged?
19. What should be retired explicitly?
20. What is the smallest implementation sequence that proves the full pipeline with the current mascot and a robot fixture/character?
21. At what point should a real generated robot CharacterSource enter the plan?
22. What tests/evidence prove that a new character does not require code changes?
23. What parts of this Planner synthesis are wrong, overcomplicated or underspecified?

Opus is expected to challenge the proposal.

Do not rubber-stamp it.

---

## 28. Requested final review output

Create:

    docs/MASCOT_UNIVERSAL_CHARACTER_SYSTEM_REVIEW_V0.1.md

It must include:

1. executive verdict;
2. architecture diagram;
3. accepted Planner ideas;
4. rejected/modified Planner ideas;
5. contradictions found in current docs/code;
6. canonical-vs-derived ownership table;
7. CharacterSource contract recommendation;
8. source-analysis / landmark-fitting plan;
9. representation-selection algorithm/decision tree;
10. mesh/rigid/substitution policy;
11. hidden-art completion policy;
12. line/shading/contact policy;
13. renderer prerequisites;
14. animation/retargeting implications;
15. IK/physics staging;
16. current-code KEEP/ADAPT/RETIRE table;
17. Slice-1 board/dummy KEEP/OPTIONAL/RETIRE decision;
18. current mascot onboarding plan;
19. robot onboarding thought experiment;
20. evidence/QA model;
21. proposed implementation phases with dependencies;
22. smallest useful first implementation slice;
23. open decisions requiring Planner/user approval;
24. risks/sunk-cost traps;
25. recommended final-plan structure.

After review, STOP.

Do not implement the final architecture yet.
