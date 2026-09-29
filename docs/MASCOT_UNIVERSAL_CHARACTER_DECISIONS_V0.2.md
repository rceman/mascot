# Mascot Universal Character Pipeline — Planner Decisions v0.2

**Status:** authoritative Planner decisions after MASCOT-CHAR-ARCH-004  
**Applies to:** first implementation slice of the universal character pipeline  
**Supersedes:** unresolved U1–U9 choices in `docs/MASCOT_UNIVERSAL_CHARACTER_ARCHITECTURE_V0.1.md` where this document decides them

## 1. User-facing generation workflow

The normal character-generation workflow has exactly **one image input plus one text prompt**:

    combined reference board
        +
    desired character prompt
        ->
    generated 1:1 CharacterSource

The user should not need to provide separate pose/style images to image generation.

The combined reference board contains two logical zones in one physical image:

    LEFT  = dummy / pose / rig guide
    RIGHT = style / identity reference

Logical authority is strict:

    pose, orientation, joint layout, limb visibility, composition
        <- LEFT dummy-guide zone

    visual style, line thickness, outline language, palette,
    rendering grammar, facial/material language
        <- RIGHT style-reference zone

The generated character MUST use the dummy pose even if the style reference depicts a different pose.

## 2. Combined reference board contract

The board is a **derived authoring artifact**, not canonical character data.

Recommended deterministic layout for v0.1:

    2048 x 1024
    +----------------------+----------------------+
    |                      |                      |
    |   1024x1024          |   1024x1024          |
    |   DUMMY GUIDE        |   STYLE REFERENCE    |
    |                      |                      |
    +----------------------+----------------------+

Rules:

- no cropping or stretching;
- each panel is fit/padded deterministically;
- neutral plain background outside source content;
- fixed left/right meaning;
- a subtle divider is allowed;
- avoid embedded explanatory text inside the image-generation content area;
- source hashes and profile id are recorded beside the derived board;
- board rendering is deterministic for identical inputs.

The board itself does not have to be 1:1. The **generated CharacterSource output is 1:1**.

## 3. Style reference

The right panel may be:

- the current canonical mascot identity image;
- a user-provided style/reference image;
- a previous approved character image;
- another approved visual reference.

The style reference may contain a different pose or a prop.

Those must not override the dummy pose.

The style reference defines, where applicable:

- line thickness;
- contour treatment;
- palette;
- shading grammar;
- face/eye language;
- material language;
- broad proportion/style feel.

It does not define:

- bind pose;
- joint coordinates;
- limb visibility;
- canvas composition.

## 4. Prompt contract

The board is always paired with a textual character brief.

Examples:

    "Create a compact friendly white service robot."

    "Create an anthropomorphic cartoon turtle compatible with this biped pose."

The generation instruction must explicitly state:

1. left panel is the exact pose/orientation/layout authority;
2. right panel is style/identity/rendering authority;
3. output exactly one clean 1:1 character;
4. preserve required limb separation/visibility;
5. no prop occlusion unless a later profile explicitly permits it;
6. no cinematic scene/background;
7. do not copy the style-reference pose.

For a requested identity that does not naturally fit `biped-3q-v1`, generation should produce a biped interpretation if reasonable (for example, an anthropomorphic turtle). A fundamentally incompatible anatomy requires a different RigProfile rather than profile-specific hacks.

## 5. CharacterSource output

The primary generation result is **one assembled 1:1 CharacterSource image**.

The user is NOT required to generate:

- a second exploded image;
- a layered PSD;
- a sprite sheet;
- separate limb images;
- animation frames.

The onboarding pipeline derives/proposes the articulated representation from the single assembled source.

## 6. Exploded parts sheet decision (U3)

The exploded parts sheet is **not a required user input and not a second independently generated canonical image**.

It may exist later as:

- a derived authoring view;
- an agent/tool-generated completion workspace;
- a QA artifact;
- a human correction surface.

Canonical identity must not depend on consistency between two independently generated images.

The pipeline should prefer:

    assembled CharacterSource
        -> segmentation / semantic classification
        -> complete hidden-part reconstruction
        -> layered prepared art
        -> optional exploded authoring/QA view

## 7. U1–U9 decisions

### U1 — first current-mascot source

The first current-mascot CharacterSource will be generated/approved by the user + ChatGPT/image generation using the combined reference board.

### U2 — dummy geometry

The architecture's current dummy geometry is accepted as a **v0 prototype**, not frozen forever.

The implementation must render it deterministically so Planner/user can visually inspect it before profile v1 is frozen.

### U3 — exploded sheet

Resolved by §6: derived/optional authoring artifact, not required generation input.

### U4 — CharacterPack persistence

Yes.

CharacterPack is a generated/committed artifact with an input-hash lock/provenance record.

Canonical source data remains profile + approved prepared art + character recipe + canonical meshes/weights.

### U5 — retarget limit behavior

There is **no universal implicit "scale the whole track down" default**.

Each semantic clip or track/group must declare an explicit retarget policy from a closed list, initially:

    preserve
    clamp
    scale_group
    solve_target
    unavailable

Meaning:

- `preserve`: values are semantic; exceeding verified range makes the clip unavailable/fail validation.
- `clamp`: per-track clamping is intended by the clip author.
- `scale_group`: a declared related track group may be scaled together.
- `solve_target`: preserve an end-effector/pose intent using constraints/IK rather than scaling raw tracks.
- `unavailable`: explicitly unsupported for a capability/character.

Do not silently change the meaning of a motion because one joint has a smaller range.

### U6 — direction/look semantics

Approved: semantic/facing-based clip names and directions, not screen-left/right names.

### U7 — current mascot identity QA pose

Approved.

The current mascot must include the canonical seated/laptop pose as an identity QA pose, even though the universal bind pose is neutral.

Character-specific substitution may be used only when verified deformation cannot reach the pose cleanly.

### U8 — shading models

Approved: engine owns a closed, versioned list of generic stylized shading models.

Character data selects a model + parameters.

No per-character shader code in normal onboarding.

### U9 — landmark authority

First implementation: manual/agent-reviewed landmark annotations are authoritative.

Automation may propose landmarks later, but hidden heuristic output is never silently canonical.

## 8. Physics/constraint architecture clarification

Do not model future physics as only a final additive "physics offset".

The generic evaluation contract should allow:

    semantic animation intent
        ->
    constraint / IK targets
        ->
    physical pose solver
        <-> contacts / joint constraints / environment
        ->
    final bone pose
        ->
    mesh skinning
        ->
    rendering

A simple ear/tail spring may still be implemented as a cheap modifier.

A future dragged/dangling character may instead use a stronger physical pose solve.

The CharacterPack and RigProfile must not need replacement to support that evolution.

Physics implementation is NOT part of the first slice.

## 9. Runtime animation remains layered/bone-based

The character architecture is runtime skeletal/deformable animation.

Do not bake normal character animations into PNG sprite sheets.

The intended runtime remains:

    layered character textures
        + skeleton
        + local weighted meshes where needed
        + semantic clips
        + constraints / IK
        + optional physics
        -> runtime frame

Sprite sheets may exist only for bounded special FX/fallback cases, not as the core character animation format.

## 10. First implementation slice

The first implementation slice should prove the authoring/profile contract without touching the production mesh renderer.

Implement:

1. versioned RigProfile schema;
2. `biped-3q-v1` profile data;
3. deterministic dummy-guide rendering from profile geometry;
4. deterministic combined-reference-board composer;
5. CharacterSource manifest/schema;
6. `mascotctl character validate` (or the narrowest coherent equivalent);
7. profile/landmark/source validation;
8. at least:
   - one mascot-proportioned fixture;
   - one synthetic/simple robot fixture;
   - negative fixtures that fail for clear reasons;
9. evidence proving both positive fixtures pass the same profile with no character-specific branch.

Do NOT implement yet:

- production image generation;
- automatic hidden-art generation;
- production segmentation;
- D3D11 mesh rendering;
- runtime shading;
- contact-line rendering;
- physics;
- IK;
- final current-mascot production art;
- full CharacterPack runtime loader;
- clip 0.4 runtime migration unless strictly required by schema work.

## 11. First-slice acceptance

The slice is successful when:

- one profile owns the dummy geometry;
- guide images are derived deterministically;
- one style image can be composed with the dummy into one combined board;
- the combined board has stable provenance/hashes;
- CharacterSource validation is profile-driven;
- robot and mascot-like fixtures pass through identical code/data paths;
- invalid pose/topology fixtures fail with actionable reasons;
- no animal-specific production logic is introduced;
- no sprite-sheet animation architecture is introduced.
