# Mascot Universal Character Guide Refinement — Planner Decisions v0.3

**Status:** authoritative Planner refinement after MASCOT-CHAR-PIPE-005  
**Applies to:** dummy/profile visual refinement before the first real generated CharacterSource  
**Implementation scope:** bounded profile/guide/validator refinement only

## 1. Slice-1 result accepted provisionally

MASCOT-CHAR-PIPE-005 proved the intended generic path:

    RigProfile
        -> deterministic dummy guide
        -> deterministic combined reference board
        -> CharacterSource contract
        -> profile-driven validation

The same path accepts both mascot-like and robot fixtures without character-specific branching.

The current `biped-3q-v1` profile remains `status = prototype`.

Do not freeze it to revision 1 yet.

## 2. Combined-board workflow remains canonical

User-facing generation input remains exactly:

    ONE combined reference board image
        +
    ONE desired-character text prompt

The board remains:

    LEFT  = dummy / pose / rig authority
    RIGHT = style / identity / line/rendering authority

The generated output remains:

    ONE assembled 1:1 CharacterSource

Do not reintroduce exploded-sheet or multi-image requirements for the user.

## 3. Dummy visual refinement required before real generation

The current deterministic dummy is technically valid, but it is not yet visually neutral enough to freeze as the reusable generation guide.

### 3.1 Remove species-like face geometry

The current generation guide uses a protruding triangular face/nose wedge.

That shape can bias image generation toward:

- a beak;
- a snout;
- a specific head construction.

For a universal biped guide, the **generation guide MUST NOT contain a protruding nose/beak/snout shape**.

Use a neutral head silhouette.

Facing direction should come from:

- the declared prompt contract;
- the 3/4 body/depth construction;
- the feet/limb orientation;
- annotated/profile landmarks outside the generation image where needed.

A face-direction marker may exist in the annotated guide, but it should not become character geometry in the generation guide.

### 3.2 Strengthen 3/4 readability without species identity

The generation dummy must clearly read as right-facing 3/4 rather than:

    3/4 head + near-frontal torso

Use generic depth cues only.

Preferred cues:

- near-side limb slightly visually stronger/thicker than far-side limb;
- near/far overlap and z-order;
- small shoulder/hip depth offsets;
- foot direction;
- asymmetry that follows the profile view contract.

Do not add facial identity, fur, clothing, a muzzle, ears, tail or props to achieve the view.

The semantic landmark geometry remains the authority. Do not silently move landmarks unless the existing coordinates prevent the required view.

If landmark changes are necessary, report them explicitly and keep profile status `prototype`.

### 3.3 Neutral proportions

The dummy should remain a **compact stylized biped**, not a realistic human mannequin and not the current mascot.

It may be cartoon-proportioned.

However, avoid guide-specific features that strongly dictate:

- one species;
- one material;
- one head type;
- one hand/paw shape.

The guide should work for examples such as:

- the current animal mascot;
- a service robot;
- an anthropomorphic turtle;
- a fox-like biped.

## 4. Slice-1 open-decision resolutions

### 4.1 Clearance thresholds

Accept the Slice-1 warning thresholds as **prototype values**:

    arm warning = 0.03
    leg warning = 0.06

Keep the existing failure thresholds unchanged.

Reason: the deterministic dummy itself measured below the earlier architecture warning values once outlines/joint discs were included.

These warning values are not frozen profile-v1 compatibility guarantees yet.

Re-measure after the visual dummy refinement.

### 4.2 Elbow bend semantics

Remove ambiguous natural-language authority such as:

    elbow bends backward

for projected 3/4 validation.

Bend direction must be defined by deterministic profile geometry / signed orientation rules derived from the canonical dummy.

Human-readable docs may describe the visual intention, but validator authority is the signed geometric rule.

Near/far projected bends may have opposite apparent screen signs.

### 4.3 Landmark ownership

For the current onboarding architecture, approved source landmark annotations MAY remain canonically inside `source.json` when:

- they are hash-bound to the exact CharacterSource image;
- owner/status are explicit;
- accepted deviations are explicit.

A later `character.json` recipe may reference/hash the approved source annotation rather than duplicate all landmark coordinates.

Do not store the same canonical landmark set independently in two files.

### 4.4 Appendage zones

Profile appendage zones are **authoring guidance**, not structural FAIL authority in this refinement.

The current tail zone overlaps the near-arm area.

Do not enable hard appendage-zone validation until the zone geometry is reviewed.

If a zone is later used for structural validation, it must be compatible with core-limb clearance and must not require a character-specific special case.

### 4.5 Prompt shading

Keep the generation prompt rule:

    flat/local colours; no baked directional lighting

The style-reference panel may show directional shading.

Image generation should extract:

- rendering language;
- palette;
- line character;

without baking pose-dependent lighting into the neutral CharacterSource.

Runtime directional shading remains the target architecture.

## 5. Generation guide vs annotated guide

Maintain two derived guide products.

### `dummy-generation.png`

For image generation.

It MUST be:

- 1:1;
- visually neutral;
- no labels;
- no landmark dots;
- no appendage zones;
- no text;
- no face/snout/beak geometry;
- no directional shading;
- no props;
- no character identity.

### `dummy-annotated.png`

For humans/agents/tooling.

It MAY contain:

- landmarks;
- ids;
- sockets;
- appendage zones;
- ground lines;
- profile envelopes;
- facing annotations.

The annotated guide is not used as the style/pose image-generation panel.

## 6. Combined reference board

Continue to generate one deterministic board.

Current v0 layout is accepted:

    2048 x 1024
    left panel  = 1024x1024 dummy-generation
    right panel = 1024x1024 style reference

The board is not itself a character asset, so it does not need to be 1:1.

CharacterSource output remains 1:1.

The board receipt must continue to record:

- profile identity/hash;
- dummy guide hash;
- style reference hash;
- board hash;
- prompt hash/brief provenance;
- composer/tool version.

## 7. Visual review gate before profile freeze

Before `biped-3q-v1` is promoted beyond prototype:

Planner/user must visually review at minimum:

1. generation dummy alone;
2. annotated guide;
3. combined board with current mascot style reference.

The review asks:

- does the dummy look species-neutral?
- is right-facing 3/4 unambiguous?
- are all core limbs readable/separated?
- is the pose suitable for both organic and rigid/mechanical bipeds?
- does the combined board clearly separate pose authority from style authority?

Do not declare profile revision 1/frozen before this review.

## 8. Scope of the refinement task

Allowed:

- guide-rendering changes;
- profile prototype metadata/value corrections when needed;
- docs;
- tests;
- validator wording/geometry fixes;
- board regeneration;
- evidence regeneration.

Not allowed:

- real image-generation API integration;
- production hidden-art generation;
- segmentation;
- CharacterPack runtime integration;
- mesh renderer;
- animation runtime migration;
- runtime shading;
- contact-line runtime;
- IK;
- physics;
- active ragdoll.

End after producing the revised visual artifacts and validation evidence.
