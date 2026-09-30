# Mascot Universal Character Architecture v0.1

**Task:** MASCOT-CHAR-ARCH-004
**Status:** agent-authored architecture proposal for Planner review. It authorizes no implementation.
**Planner authority:** `docs/MASCOT_UNIVERSAL_CHARACTER_PIPELINE_V0.1.md` (UCP). Its decisions D1–D7 supersede the research report's recommendations where they differ.
**Inputs:**
- `docs/MASCOT_RIG_ART_CONSTRUCTION_RESEARCH_V0.1.md` (research)
- `docs/MASCOT_RIG_ART_CONSTRUCTION_SPEC_V0.1.md` (construction spec)
- `docs/MASCOT_NEUTRAL_BIND_POSE_RESEARCH_SPEC_V0.1.md`
- read-only inspection of `crates/`, `apps/` and `assets/mascot/rig-v0.2/`: committed HEAD `f3ed251` plus the paused, uncommitted WIN-002 worktree

Normalized coordinates in this document: origin at the canvas top-left, `x` to the right, `y` down, both in `[0,1]`, on a square canvas. "Near" and "far" are relative to the viewer.

---

## 1. Executive architecture

Four layers. Each owns only its own concerns:

    Engine (generic code)            skeleton eval, clip eval, skinning, renderer,
                                     runtime outline/shading/contact lines, QA engine,
                                     pack loader, retargeter, tooling
        consumes
    RigProfile (data, versioned)     roles, hierarchy, bind landmarks, sockets,
                                     slots/z bands, constraints, default deformation
                                     policy, QA anchors, stress poses, dummy guide
        instantiated by
    Character (data + art)           CharacterSource (provenance) -> prepared art +
                                     recipe (canonical) -> CharacterPack (derived)
        animated by
    Clip library (data, per profile) semantic clips on roles, normalized units,
                                     capability requirements
        optionally with
    Prop (data + art)                separate prop rig + mount/contact declarations

**Core rules**

1. The engine contains **no character names**. It knows only profile-declared roles, sockets and capabilities, read from data.
2. A profile contains **no identity**: no palette, species, materials, face design or appendage shapes.
3. A character contains **no engine logic**. Everything that varies per character is data: parts, meshes, weights, limits, materials, contact policies, overrides.
4. Clips address **roles, never parts**. They are written in profile-normalized units, so the same clip file drives any pack of that profile.
5. Every datum has **exactly one canonical owner**. Everything else is derived by deterministic build steps (§8.1).

**The robot test (§18) is the acceptance criterion.** A friendly robot drawn over the same dummy must go through the same tools, schema, clips and renderer. Its differences must be expressible purely as data.

---

## 2. Terminology and authority

| Term | Definition | Canonical owner | Location (proposed) |
|---|---|---|---|
| **RigProfile** | Articulation contract for a family of characters (`biped-3q-v1`) | Planner-approved data | `assets/character-templates/biped-3q-v1/profile.json` |
| **Dummy geometry** | Exact bind landmarks, envelope capsules, sockets and zones of the profile | Part of `profile.json` | same file (single source) |
| **Dummy guide** | Raster renders of the dummy geometry for image generation and annotation | **Derived**, by `mascotctl profile guide` | `assets/character-templates/biped-3q-v1/guide/*.png` |
| **CharacterSource** | Candidate or approved art of one identity drawn over the dummy, plus its manifest | User/artist/generator output; approval by user | `assets/characters/<id>/source/` |
| **Prepared art** | Layered, completed per-part art (albedo / intrinsic line / mark) prepared from the source | Canonical character art after onboarding | `assets/characters/<id>/art/` |
| **Character recipe** | The character's decisions: landmark annotation, accepted deviations, deformation overrides, limits, materials, contacts, appendages, clip overrides | Canonical character data | `assets/characters/<id>/character.json` |
| **Character meshes** | Mesh topology + weights for meshed parts (generator-initialized, then owned) | Canonical character data | `assets/characters/<id>/meshes/*.json` |
| **CharacterPack** | Runtime-ready bundle built from the profile, art, recipe and meshes | **Derived** (`mascotctl character build`) | `assets/characters/<id>/pack/` (committed generated artifact, carries its input hashes) |
| **Semantic clip** | Clip on profile roles in normalized units | Clip-library data | `assets/character-templates/biped-3q-v1/clips.json` |
| **Retarget report** | Per pack × clip: resolved tracks, dropped optional tracks, clamps/scales, availability | Derived | build output / evidence |
| **Prop** | Separate rig (skeleton + parts) with mount and contact sockets | Prop data + art | `assets/props/<id>/` |

**Authority order when documents conflict:**
1. Planner documents.
2. This architecture, once Planner-approved.
3. The construction spec addenda.
4. The research report.

---

## 3. `biped-3q-v1` semantic skeleton

### 3.1 Role naming

- Role ids are lowercase dot-paths: `<segment>[.<side>]`.
- `side ∈ {left, right}` is **anatomical** (the character's own left/right).
- Appendage and socket ids use the `appendage.` and `socket.` prefixes.
- Role ids are stable contract strings (Gate 9): renaming one requires a new profile major version.

### 3.2 Core roles and hierarchy (all required)

    root                                  ground reference, between the feet at ground contact
    └─ pelvis
       ├─ spine
       │  └─ chest
       │     ├─ neck
       │     │  └─ head
       │     ├─ shoulder.right ─ upper_arm.right ─ lower_arm.right ─ hand.right
       │     └─ shoulder.left  ─ upper_arm.left  ─ lower_arm.left  ─ hand.left
       ├─ upper_leg.right ─ lower_leg.right ─ foot.right
       └─ upper_leg.left  ─ lower_leg.left  ─ foot.left

That is 20 core roles.

- **`shoulder.*`** (clavicle) is required as a role but may be *inert*: zero-length, no art, rotation limit [0,0].
  - This keeps shrug and reach clips portable.
  - A character without clavicle motion simply reports the capability `shoulder.shrug` as unavailable.
- **`root`** is defined at the ground contact point. The renderer's contact shadow already defaults its floor to the origin of `world[0]` (`renderer.rs:572`). That default becomes a profile guarantee rather than a mascot coincidence.

### 3.3 Optional face-feature roles (children of `head`)

| Role | Purpose | Capability it provides |
|---|---|---|
| `eye.right`, `eye.left` | Independently scalable/offset eye features | `eyes.blink` (scale_y), `eyes.look` (x/y) |
| `mouth` | Mouth feature | `mouth.open` (scale_y) / `mouth.state` (substitution) |
| `brow.right`, `brow.left` | Optional expression features | `brows.raise` |

Face features are **roles, not parts**:
- a robot's two LED eyes provide `eye.right` and `eye.left`;
- a single visor provides `eye.right` only and declares itself `cyclops: true` in the pack. The eye group then resolves to one member, and `eyes.look` still works.

### 3.4 Optional appendages

These are typed chains attached to sockets (§10). They are not core roles.

    appendage.tail                 chain of 1–4 bones: appendage.tail.0 .. .3
    appendage.ear.right|left       chain of 1–2
    appendage.antenna.<n>          chain of 1–3, n = 0..3
    appendage.wing.right|left      chain of 1–2 (future)

### 3.5 View and orientation semantics

- **Canonical view:** right-facing 3/4. The character faces screen-right (+x).
- **Near/far mapping (derived from the view, not stored per character):**
  - facing +x, the anatomical **right side is near** and the **left side is far**;
  - check: with y up and the viewer at +z, `right = forward × up = (+x) × (+y) = +z`, toward the viewer.
- **Runtime mirroring** (`RootPlacement.mirror`) is a presentation transform. Under mirroring:
  - anatomical labels do not change: the near-side art stays the anatomical right art;
  - screen-direction semantics flip;
  - clips never refer to screen sides (§9.3).
- **Rotation convention:**
  - clip rotations are degrees in the **canonical view**, positive = clockwise on screen (the current convention);
  - mirroring negates the effective screen rotation;
  - no per-bone sign conventions.
- **Mapping for the current mascot**, which uses mixed conventions today:
  - `eye_left` sits at screen-left, so it is the **near eye = `eye.right`**; `eye_right` → `eye.left`;
  - `*_near` → `*.right`, `*_far` → `*.left`.

### 3.6 Slots and z bands

The profile defines default **z bands** by role for the canonical view, back to front:

    appendage.wing.left < upper_arm.left, lower_arm.left, hand.left < appendage.ear.left
    < upper_leg.left, lower_leg.left, foot.left < appendage.tail < pelvis < spine < chest
    < shoulder.* < neck < upper_leg.right, lower_leg.right, foot.right < head < eye.*, mouth, brow.*
    < appendage.ear.right < appendage.antenna.* < upper_arm.right, lower_arm.right, hand.right
    < appendage.wing.right

- **Within a band**, the pack orders its own parts.
- **Across bands**, a pack may override only by explicit, named z rules (e.g. `appendage.ear.left above head` for tall ears). Each rule is validated to be acyclic.
- **Props** insert themselves relative to bands (§11), never with absolute numbers.

### 3.7 Constraints (declared now, implemented later)

| Constraint | Roles | Semantics |
|---|---|---|
| `ik.arm.<side>` | upper_arm → lower_arm → hand | 2-bone IK; elbow bend sign = backward (the elbow points away from facing) |
| `ik.leg.<side>` | upper_leg → lower_leg → foot | 2-bone IK; knee bend sign = forward (toward facing) |
| `look.head` | neck, head, eye.* | Distribute a look target across neck/head/eyes by profile weights (e.g. 0.2 / 0.5 / 0.3) |
| `limit.<role>` | every role | Hard joint limit = the pack's verified range (§8.5) |
| `mount.<socket>` | props | Prop root follows the socket transform (§11) |

Order of evaluation:

    clips → constraints → physics offsets (future) → skinning

This is the same ordering Spine uses for physics (research S13).

### 3.8 Default deformation policy (profile defaults; the pack may override per joint)

| Joint | Default class | Why this default |
|---|---|---|
| pelvis/spine/chest | weighted mesh (torso) | Continuous surface for organic characters |
| shoulder.* / upper_arm.* | weighted mesh | Shoulder flow |
| lower_arm.* | weighted mesh (same mesh as the upper arm) | Elbow flow |
| hand.* | rigid hinge | Distinct object |
| upper_leg.* / lower_leg.* | weighted mesh | Hip/knee flow |
| foot.* | rigid hinge | Distinct object |
| neck / head | head rigid socket over a complete neck | Head is a distinct silhouette |
| appendage.tail | chain + weighted mesh | Flexible |
| appendage.ear/antenna | rigid socket, concealed root | Small ranges |

A robot pack typically overrides most joints to `rigid-hinge`, because its parts really are rigid. That is a data decision; it needs no code (§18).

---

## 4. Exact dummy pose specification

### 4.1 Canvas and rendering

- Square canvas, geometry defined only in normalized coordinates.
- Guide renders at **1024²** (generation input) and **2048²** (annotation), both derived from `profile.json`.
- Safe margin: every core landmark and every core envelope capsule lies inside `[0.06, 0.94]²`. Only appendage zones may extend to `[0.02, 0.98]`.
- Ground lines:
  - **near foot contact** `y = 0.915`;
  - **far foot contact** `y = 0.905`. The far foot is 0.01 higher, a ground-plane depth cue.

### 4.2 Pose description

The pose is a relaxed, right-facing 3/4 A-pose. All segment angles below are from vertical in the canonical view.

- **Head:** faces +x in 3/4. Head and body share the same view, with the face front at `x ≈ 0.69`.
- **Torso:** upright. The trunk (neck base → pelvis) leans about 2°.
- **Arms:** hang in an A-pose.
  - The upper arms make 45° with vertical, abducted outward (profile revision 1).
  - The elbows are relaxed with about 15° of flexion: forearms at about 30° from vertical; hands continue at about 25°.
  - Arm-to-torso clearance is at least 0.04 at elbow height (measured on the rendered revision-1 dummy: ≈0.044 near / ≈0.049 far).
- **Legs:** a slight A-stance.
  - The thigh-to-shin knee bend is about 8°, with the knee **forward** (toward +x) of the hip–ankle line.
  - Feet point toward +x and lie flat on the ground lines.
  - Leg-to-leg clearance is at least 0.08 at knee height (measured on the rendered revision-1 dummy: ≈0.092).
- **Hands:** open, neutral, clear of the torso and thighs. There is no grip pose.
- **Visibility:**
  - all 20 core joints are visible;
  - the far arm and far leg are visible outside the torso silhouette, except their roots.

### 4.3 Guide rendering style (both renders derived from the same geometry)

- **`guide/dummy-generation.png`**: the conditioning image given to image generation.
  - A featureless mannequin of flat capsules, with no text, labels or numbers, because generators copy text.
  - **Near-side** segments are flat `#C4C4C4`; **far-side** segments are flat `#9C9C9C`; the trunk and head are `#B4B4B4`.
  - Outline `#404040`, 0.006 wide.
  - Background: uniform `#FFFFFF`.
  - No shading and no gradients.
  - The facing direction is shown by shape, not colour alone: a small wedge "nose" on the head at `head.face`, and foot capsules pointing +x.
  - Joints are drawn as slightly larger circles (1.15× the capsule width) so they read as articulation points without labels.
- **`guide/dummy-annotated.png`**: the same mannequin plus landmark dots, ids, socket markers, appendage zones and clearance guides. Used for human/agent annotation and review, **never** as generation input.
- **`guide/dummy-mask.png`**: a per-role colour-index mask, for automatic segmentation priors (§13, step 5).

### 4.4 Envelope capsules (mannequin shapes; recommended body envelope)

All sizes are normalized.

| Segment | Shape | Parameters |
|---|---|---|
| head | ellipse | centre (0.555, 0.190), rx 0.135, ry 0.120 |
| neck | capsule | `neck` → `head`, width 0.070 |
| chest | ellipse | centre (0.490, 0.450), rx 0.085, ry 0.100 |
| pelvis | ellipse | centre (0.490, 0.590), rx 0.085, ry 0.060 |
| upper arm | capsule | shoulder joint → elbow, width 0.050 |
| forearm | capsule | elbow → wrist, width 0.045 |
| hand | ellipse | wrist → hand tip axis, 0.050 × 0.065 |
| thigh | capsule | hip → knee, width 0.070 |
| shin | capsule | knee → ankle, width 0.058 |
| foot | capsule | heel → toe, height 0.040 |

---

## 5. Normalized landmark table

**Classes:**
- **J:** joint and pivot. This is the bone origin; the bind rest transforms are derived from it.
- **T:** tip or end, for leaf directions and validation.
- **F:** feature (optional roles).
- **S:** socket.

**Position tolerance** `r` is used only for the WARN tier (§7.4). The pose tier uses angles.

| Id | Class | x | y | r | Notes |
|---|---|---|---|---|---|
| `root` | J | 0.490 | 0.910 | 0.04 | ground reference between the feet |
| `pelvis` | J | 0.485 | 0.600 | 0.04 | |
| `spine` | J | 0.485 | 0.515 | 0.04 | |
| `chest` | J | 0.490 | 0.430 | 0.04 | |
| `neck` | J | 0.495 | 0.345 | 0.04 | neck base |
| `head` | J | 0.510 | 0.315 | 0.04 | head pivot (top of neck) |
| `shoulder.right` | J | 0.470 | 0.370 | 0.04 | clavicle root, near |
| `upper_arm.right` | J | 0.415 | 0.385 | 0.05 | shoulder joint, near |
| `lower_arm.right` | J | 0.335 | 0.500 | 0.05 | elbow, near |
| `hand.right` | J | 0.295 | 0.610 | 0.06 | wrist, near |
| `shoulder.left` | J | 0.520 | 0.365 | 0.04 | clavicle root, far |
| `upper_arm.left` | J | 0.575 | 0.375 | 0.05 | shoulder joint, far |
| `lower_arm.left` | J | 0.650 | 0.490 | 0.05 | elbow, far |
| `hand.left` | J | 0.690 | 0.600 | 0.06 | wrist, far |
| `upper_leg.right` | J | 0.435 | 0.620 | 0.05 | hip joint, near |
| `lower_leg.right` | J | 0.420 | 0.760 | 0.05 | knee, near |
| `foot.right` | J | 0.390 | 0.880 | 0.06 | ankle, near |
| `upper_leg.left` | J | 0.545 | 0.615 | 0.05 | hip joint, far |
| `lower_leg.left` | J | 0.585 | 0.755 | 0.05 | knee, far |
| `foot.left` | J | 0.600 | 0.870 | 0.06 | ankle, far |
| `head.top` | T | 0.530 | 0.070 | 0.06 | crown |
| `head.face` | T | 0.690 | 0.215 | 0.06 | front-most face point (snout, faceplate, nose) |
| `chin` | T | 0.585 | 0.300 | 0.06 | lowest front point of the head silhouette |
| `hand.right.tip` | T | 0.280 | 0.665 | 0.06 | |
| `hand.left.tip` | T | 0.705 | 0.655 | 0.06 | |
| `foot.right.toe` | T | 0.465 | 0.915 | 0.06 | |
| `foot.right.heel` | T | 0.370 | 0.915 | 0.06 | |
| `foot.left.toe` | T | 0.675 | 0.905 | 0.06 | |
| `foot.left.heel` | T | 0.580 | 0.905 | 0.06 | |
| `eye.right` | F | 0.555 | 0.190 | 0.07 | near eye centre (optional) |
| `eye.left` | F | 0.640 | 0.185 | 0.07 | far eye centre (optional) |
| `mouth` | F | 0.635 | 0.255 | 0.07 | optional |
| `socket.head.top.near` | S | 0.470 | 0.100 | 0.07 | ear.right / antenna |
| `socket.head.top.far` | S | 0.585 | 0.085 | 0.07 | ear.left / antenna |
| `socket.head.top.center` | S | 0.530 | 0.070 | 0.07 | antenna |
| `socket.chest.back` | S | 0.420 | 0.440 | 0.06 | wings / backpack |
| `socket.pelvis.back` | S | 0.400 | 0.600 | 0.06 | tail |
| `socket.hand.right.grip` | S | 0.285 | 0.640 | 0.06 | prop |
| `socket.hand.left.grip` | S | 0.700 | 0.630 | 0.06 | prop |
| `socket.foot.right.sole` | S | 0.415 | 0.915 | 0.06 | prop / ground |
| `socket.foot.left.sole` | S | 0.625 | 0.905 | 0.06 | prop / ground |

> **Revision note (profile revision 1):** `assets/character-templates/biped-3q-v1/profile.json` is the canonical geometry. Revision 1 raises the arms (upper arm 45°, forearm 30°, hand 25° from vertical), widens the stance (far leg +0.020 x), moves the tail zone below the hands and restores the clearance warn thresholds (arms 0.04, legs 0.08). Where this table differs, the profile wins.

**Bone-local sockets** (not bind positions; defined by a parameter along the bone plus an offset):

- `socket.chin`: at the `chin` landmark, owned by `head`.
- `socket.upper_leg.<side>.lap`: `t = 0.6` along `upper_leg`, offset to the upper surface (+0.5 × local width). Meaningful when seated.

**Pose-tier constraints** (these are what validation FAILs on):

| Constraint | Dummy value | Tolerance |
|---|---|---|
| Upper arm angle from vertical, abducted outward | 45° (near and far) | ±12° |
| Forearm angle | about 30° | ±12° |
| Elbow flexion | about 15° | 0–35°, bend backward |
| Thigh angle | near 6° / far 16° (outward) | ±10° |
| Knee flexion | about 8° | 0–25°, knee forward |
| Trunk lean (neck-base → pelvis) | about 2° | ±8° |
| Head facing | `head.face.x > head.x + 0.05` | hard |
| Eye order (if both eyes exist) | `eye.right.x < eye.left.x` | hard |
| Side order | near-arm joints have x < far-arm joints at the same level; near leg x < far leg x | hard |
| Arm–torso clearance at elbow height | ≥ 0.04 | ≥ 0.02 FAIL, 0.02–0.04 WARN |
| Leg–leg clearance at knee height | ≥ 0.08 | ≥ 0.03 FAIL, 0.03–0.08 WARN |
| Foot ground contact | toe/heel y within ±0.015 of the ground lines | hard |

**Proportion envelope**, as fractions of `H = root.y − head.top.y`. This allows mascot-like, robot-like and chibi-humanoid characters:

| Measure | Range |
|---|---|
| Head height (`head.top` → `chin` extent) | 0.18–0.55 H |
| Arm length (shoulder joint → wrist) | 0.18–0.45 H |
| Leg length (hip → ankle) | 0.12–0.45 H |
| Trunk (neck base → pelvis) | 0.12–0.40 H |
| Head width | 0.8–1.6 × head height |

---

## 6. Required / optional slots and sockets

### 6.1 Core slots

Each core role owns at most one **body slot**:

- the slot has a `fill` layer (albedo), an optional `line` layer (intrinsic lines) and an optional `mark` layer;
- a meshed region may span several roles (e.g. `torso` covering pelvis, spine and chest), declared as a **region slot** with its member roles;
- required slots: `head`, trunk coverage (pelvis/spine/chest via one or more slots), each upper arm, lower arm, hand, upper leg, lower leg and foot;
- `neck` and `shoulder.*` may be covered by a region slot or by their parents (they have no own slot).

### 6.2 Sockets

A socket is a profile-defined, bone-attached frame (origin + orientation) that accepts one of these typed attachments:

| Socket | Accepts |
|---|---|
| `socket.head.top.near` / `.far` / `.center` | `appendage.ear.*`, `appendage.antenna.*`, `accessory` |
| `socket.chest.back` | `appendage.wing.*`, `prop` |
| `socket.pelvis.back` | `appendage.tail` |
| `socket.hand.<side>.grip` | `prop` |
| `socket.foot.<side>.sole` | `prop` / ground contact |
| `socket.chin` | contact only |
| `socket.upper_leg.<side>.lap` | `prop` |

A pack may use each socket at most once for appendages. Props may share sockets through the mount system.

---

## 7. CharacterSource contract

### 7.1 Files

    assets/characters/<id>/source/
      source.png          assembled candidate over the dummy pose (required)
      exploded.png        exploded parts sheet of the same character (required for production packs)
      source.json         manifest (required)

The **exploded sheet** is this architecture's answer to the flattened-image problem:

- The profile also derives `guide/dummy-exploded-generation.png`: the same mannequin with every articulated segment translated apart along its parent bone axis by a known offset (e.g. 0.04). This leaves gaps at the neck, shoulders, elbows, wrists, hips, knees, ankles and appendage sockets.
- The generator (or artist) draws the **same character** in that layout. Every joint end is then drawn complete, which is the complete-parts rule (construction spec §2.1 / research S1, S7).
- Because the offsets are known exactly, the pipeline reassembles the parts deterministically at their bind positions. It checks consistency against `source.png` (§7.4).
- This is the proposal's highest-risk element (generator consistency between two images). See decision U3.

### 7.2 `source.json`

```json
{
  "format": "mascot-character-source/1",
  "character_id": "robot-demo",
  "profile": {"id": "biped-3q-v1", "sha256": "<profile.json hash>"},
  "files": {
    "source":   {"path": "source.png",   "sha256": "...", "size": [2048, 2048]},
    "exploded": {"path": "exploded.png", "sha256": "...", "size": [2048, 2048]}
  },
  "provenance": {
    "kind": "generated|artist|synthetic",
    "tool": "...", "prompt": "...", "guide": "guide/dummy-generation.png",
    "guide_sha256": "...", "created": "YYYY-MM-DD"
  },
  "identity_reference": "assets/mascot.png",
  "approval": {"state": "candidate|approved|rejected", "by": "user", "note": "..."}
}
```

The landmark annotation and the accepted deviations live in `character.json` (the recipe), not here. That keeps each fact in one file: the source manifest records provenance only.

### 7.3 Visual rules

**Required (FAIL if violated):**

- square canvas of at least 1024²; one character; no crop; the facing and view of the profile;
- transparent or uniform background;
- no props, no cast or ground shadows, no scenery;
- all core segments visible and separable; no limb crossing another required limb;
- appendage roots visible inside their zones:
  - tail zone `x∈[0.04,0.40], y∈[0.45,0.93]`;
  - head-top zone `x∈[0.36,0.72], y∈[0.02,0.16]`;
  - back zone `x∈[0.10,0.45], y∈[0.25,0.60]`;
- no appendage overlapping a core limb, except at its own socket;
- no directional baked shading:
  - per semantic region, colours cluster into albedo + marking clusters;
  - a luminance gradient aligned with a single light direction across several regions is flagged;
  - see §15, S-7.

**Line assumptions:**

- An external outline, if present, has approximately uniform width. It is measured and **removed**; the runtime regenerates it.
- Intrinsic lines must be separable from the outer silhouette.

**Occlusion:**

- In `source.png`, only the dummy's inherent overlaps are allowed: the neck under the head, the far shoulder and hip roots behind the trunk, and socket roots.
- In `exploded.png`, no segment may occlude another.

**Identity:** free, within the proportion envelope (§5), e.g. animal vs robot, fur vs metal, ears vs antennae, tail vs none.

### 7.4 Validation tiers

| Tier | Checks | Result |
|---|---|---|
| **Format** | canvas, alpha/background, hashes, profile hash matches | FAIL |
| **Pose** | §5 pose-tier constraints on the annotated landmarks | FAIL |
| **Topology** | one connected silhouette (assembled); segment separability; zones; no crossing | FAIL |
| **Proportion** | §5 proportion envelope | FAIL outside; WARN near the edges |
| **Envelope** | landmark distance to the dummy > `r` | WARN; the user or agent must record an accepted deviation with a reason in `character.json` |
| **Exploded consistency** | reassembled exploded vs assembled source: silhouette IoU, per-region albedo ΔE, intrinsic-line chamfer (research §14 metrics) | FAIL / WARN; numbers are frozen only after the first real source (UCP D4) |
| **Style** | baked directional shading; outline uniformity; palette cluster count | WARN for review |

**Landmark annotation.**
- In the first slice, landmarks are **annotated**: by the agent, via an overlay sheet and the annotation tool, or by the user.
- Automatic landmark estimation (dummy-mask registration) is a later aid. When it is added, its output is still stored as an annotation and reviewed. No hidden heuristic becomes authoritative.

---

## 8. CharacterPack contract

### 8.1 Canonical vs derived (Gate 7)

| Datum | Canonical in | Derived into the pack as |
|---|---|---|
| Joint positions (bind pivots) | `character.json` `landmarks` (normalized) | Bone rest transforms in canvas px (computed; never stored twice) |
| Per-part art | `art/<part>.{fill,line,mark}.png` | Trimmed textures + origins |
| Mesh topology/weights | `meshes/<region>.json` | Validated copy / packed buffers |
| Z order | profile bands + `character.json` z rules | Resolved slot order |
| Limits | `character.json` `limits` (desired + verified) | Constraint table |
| Materials / outline / shading params | `character.json` `materials` | Resolved per-slot parameters |
| Contact policies + edge masks | `character.json` `contacts` + `art/<part>.edge.<pair>.png` | Resolved pair table |
| Capabilities | **not stored**: computed from present roles and features | Report only |
| QA anchors | profile anchors (role-local definitions) + the character's measured landmarks | Resolved ROIs (no per-character hand-written joint list) |

**`pack/` is a generated artifact.** It is committed alongside `pack.lock.json`, which records the sha256 of every input (profile, art, recipe, meshes) and the builder version. The loader rejects a pack whose lock does not match its inputs in the development build (analogous to UCP D6 for shaders).

### 8.2 `character.json` (the recipe, canonical)

```json
{
  "format": "mascot-character/1",
  "id": "robot-demo",
  "profile": "biped-3q-v1",
  "canvas": 2048,
  "landmarks": {"root": [0.49, 0.91], "pelvis": [0.486, 0.598], "...": []},
  "accepted_deviations": [{"landmark": "head.top", "reason": "larger head by design"}],
  "features": {"eye.right": true, "eye.left": true, "mouth": false},
  "appendages": [
    {"type": "antenna", "id": "appendage.antenna.0", "socket": "socket.head.top.center", "bones": 2}
  ],
  "parts": [
    {"id": "head", "role": "head", "layers": ["fill", "line"]},
    {"id": "torso", "region": ["pelvis", "spine", "chest"], "layers": ["fill"], "mesh": "torso"}
  ],
  "deformation": {"upper_arm.*": "rigid-hinge", "upper_leg.*": "rigid-hinge"},
  "z_rules": [],
  "limits": {"head.rotation": {"desired": [-15, 15], "verified": [-12, 12]}},
  "materials": {
    "outline": {"width_h": 0.012, "colour": "#1A1A1A"},
    "shading_groups": [{"parts": ["upper_arm.right"], "model": "top_light", "light": "#DDE6EE", "inset_h": 0.01}]
  },
  "contacts": [{"front": "hand.right", "back": "prop:laptop.lid", "edge_mask": "art/hand.right.edge.lid.png", "width_h": 0.012}],
  "clip_overrides": {}
}
```

Widths and offsets use **H-normalized units** (`_h` suffix, fraction of `H`), so values survive resolution changes.

### 8.3 Pack contents (derived)

- **`pack.json`:**
  - profile id + hash;
  - character id;
  - canvas;
  - resolved skeleton (roles present, rest transforms, limits);
  - slots (resolved z, layer textures, geometry = sprite | mesh ref);
  - sockets used;
  - materials;
  - contacts;
  - QA ROIs;
  - retarget report reference.
- **`textures/*.png`**: trimmed and padded; optionally atlased later.
- **`meshes/*.bin|json`**: validated per the construction spec AR-12..AR-17.
- **`pack.lock.json`**.

### 8.4 Minimum needed per concern

| Concern | Minimum data |
|---|---|
| Textures | Per slot layer: image, origin, trim rect |
| Semantic parts | part id, role or region roles, layers |
| Meshes | vertices (bind, canvas px), UVs, triangles, weights ≤ 4 per vertex, normalized at authoring time |
| Weights | inside the mesh file only |
| Line classes | class C = `line` layers; class B = `contacts`; class A = runtime (no data except `outline` params) |
| Material/shading | `outline`, `shading_groups`; albedo lives in textures |
| Slots/z | resolved order |
| Optional appendages | type, socket, bone count, parts |
| Contact-line policies | front, back, edge mask, width |
| Joint limits | desired + verified per role/property |
| QA anchors | resolved ROIs + the landmark set |

### 8.5 Limits

- **Desired** ranges come from the profile defaults. The character may narrow them.
- **Verified** ranges come from art-readiness sweeps, recorded by tooling (construction spec §12).
- The runtime clamps to **verified**.
- Widening a verified range requires a new sweep. The recorded range carries the sweep evidence hash.

---

## 9. Animation retargeting contract

### 9.1 Clip format (hard cut to `mascot-clips/0.4`, at slice-2 time)

`0.4` keeps the compact 0.3 grammar: groups, key tuples, `target.property`. What changes:

- **Targets are profile roles or profile groups.** Examples: `head.rotation`, `@eyes.scale_y`, `@ears.rotation`, `appendage.tail.*.rotation`.
- **Groups are profile-level:**
  - `@eyes = [eye.right, eye.left]`;
  - `@ears = appendage.ear.*`;
  - `@antennae = appendage.antenna.*`;
  - `@tail = appendage.tail.*`.

  A group resolves against the pack's present roles.
- **Normalized translation units.** `x`/`y` values are multiples of a per-role **unit** declared by the profile:

  | Roles | Unit |
  |---|---|
  | root, pelvis, spine, chest | `H` |
  | neck, head | `head_h` |
  | eye.* | `eye_w` |
  | limbs | own segment length |
  | appendages | own chain length |

  The pack measures these from its landmarks.
- **Rotation, scale and opacity** are unitless. Rotations use the canonical-view convention (§3.5).
- **Clip header:**

  ```json
  "look_forward": {"duration": 1.7, "mode": "additive",
    "requires": ["head", "neck"], "optional": ["@eyes", "@ears"], "range": "scale",
    "tracks": {"head.rotation": [[0, 0], [0.4, 2, "linear"], [1.2, 2], [1.7, 0]]}}
  ```

- **`mode`:**
  - `additive` (default): offsets from the pack bind;
  - `absolute`: profile-space target angles for pose states such as `sit`, `stand` or `hand_on_prop`.

### 9.2 Expansion (deterministic, at load; no runtime interpretation)

1. Resolve groups and wildcards against the pack's roles, in declared order.
2. **Availability:**
   - a missing `requires` role means the clip is **unavailable** for this pack. This is recorded in the retarget report and is not a runtime error; the director never schedules it;
   - a missing `optional` role means its tracks are **dropped** and the drop is recorded;
   - a track on a role in neither list is a clip-library validation error.
3. **Units:** `px = v × unit(role)`.
4. **Absolute mode:** `offset_r = target_r − bind_r(character)`. Both are world-space angles in the canonical view. The target is authored against the dummy, so each pack's bind deviation from the dummy is compensated automatically. This is why the pose tier (§5) bounds bind angles: it keeps the offsets inside verified ranges.
5. **Range policy against the verified limits:**
   - `clamp`: clamp each key;
   - `scale` (default for additive idle clips): multiply the positive lobe of the track by `min(1, hi / max_pos)` and the negative lobe by `min(1, lo / min_neg)`. This preserves timing and shape.

   Every scale and clamp is recorded.
6. **Mirroring** is applied after evaluation. No clip changes are needed.

### 9.3 Direction semantics

- Clips never say "screen-left".
- "Look toward facing" is `look_forward`; "look away" is `look_back`.
- The current `look_left`/`look_right` clips become `look_back`/`look_forward` in the canonical view. They stay correct under mirroring.

### 9.4 Capabilities

- A clip's `requires` implies capabilities: e.g. `blink` requires `@eyes` with at least 1 member.
- The pack's capability list is **computed** (§8.1).
- The **idle director** selects from `idle.json`, a profile-level default schedule; a pack may replace it entirely. Unavailable clips are filtered out. There are no hard-coded clip names in the engine.

### 9.5 Character overrides

- `clip_overrides` in `character.json` names a replacement clip file per clip id.
- A replacement is a **whole clip**. Partial track patching is not allowed: merge semantics would create a second meaning for the same clip (Gate 9).
- An override must satisfy the same `requires`.

### 9.6 Failure behavior

| Situation | Behavior |
|---|---|
| Clip-library error (unknown role, bad group) | Build FAIL |
| Unavailable clip | Report; excluded from scheduling |
| Dropped optional tracks | Report |
| Scale/clamp applied | Report, with factor |
| Absolute-mode target outside verified limits | Clamp + report; WARN in QA |

The runtime never falls back to a different clip silently.

---

## 10. Optional appendage model

- **Declaration.** An appendage is declared in `character.json` with:
  - `type` (`tail|ear|antenna|wing`);
  - `id` (a profile-pattern id);
  - `socket`;
  - `bones` (chain length, within the profile's per-type bounds);
  - `parts`.
- **Chain construction.** The chain bones are derived from annotated appendage landmarks: `appendage.<id>.<k>` joint points, annotated like the core landmarks.
- **Defaults per type** (profile data): deformation class, default desired ranges, z band, clip groups, QA anchors.
- **Clip rules:**
  - clips target appendages only through groups/wildcards (`@tail`, `@ears`, `@antennae`);
  - an absent appendage drops those tracks (optional);
  - a clip may **require** an appendage (e.g. `tail_flick` requires `@tail`), and is then unavailable for tail-less packs.
- **Extensibility.** A new appendage *type* is a profile change: a new minor version, if additive. Unknown types are rejected rather than ignored.

---

## 11. Prop / contact model

- **Separate rig.** A prop is its own rig in `assets/props/<id>/prop.json`: bones, parts, materials, **mount point** and **contact anchors**. The prop format reuses the pack slot/mesh schema; props are "characters without a profile".
- **Mount.** A **pose state** declares mount relations, e.g. `sit_laptop` mounts `prop:laptop.root` to `socket.upper_leg.right.lap`, with an offset in `H` units.
- **Z insertion** is relative to profile z bands, e.g. `laptop.base above upper_leg.right; laptop.lid below upper_arm.right`. It is resolved per pose state.
- **Contacts** are class B pairs (research §9) between a character slot and a prop slot, declared in the character recipe. The edge masks belong to the front part's art. Future IK targets reuse the same anchors, e.g. `hand.right` → `laptop.keyboard.anchor`, `chin` → `laptop.lid.top`.
- **Current mascot:** the seated laptop pose is the pose state `sit_laptop`:
  - an `absolute` clip for body angles;
  - a mount for the laptop;
  - contacts for the chin, paw and foot.

  Its fidelity to `assets/mascot.png` is verified in that pose state (§15, S-10), not at bind.

---

## 12. Line / shading / material ownership

| Concern | Owner | Mechanism |
|---|---|---|
| External silhouette outline (class A) | Engine | Composition-derived ring; `outline.width_h` and colour from the pack; width 0 allowed (outline-free styles) |
| Contact lines (class B) | Engine mechanism + character data | Front-owned, clipped to back coverage (research §9.2) |
| Intrinsic lines (class C) | Character art | `line` layer; follows the owner's sprite/mesh |
| Albedo, markings | Character art | `fill` / `mark` layers |
| Directional shading | Engine model + character params | `shading_groups` (`top_light` model in research §10). A robot may use a `band` or `rim` model variant, but only through a closed, versioned list of engine models; there are no per-character shaders |
| Non-directional local AO/material texture | Character art (allowed if rotation-safe) | UCP D2 |
| Shadows (contact/drop) | Engine | Floor = `root` (§3.2) |

**No species concepts appear in the engine.** "Fur", "muzzle", "metal" and "visor" are character-art facts only. Material models are described by rendering behavior (e.g. `top_light`, `band`, `rim`), not by substance names.

---

## 13. Source-to-pack build pipeline

Every step is a `mascotctl` command. Outputs are cached by input hash. Each step emits a report and review sheets.

| # | Step | Input → output | Automatic now | Agent/user-owned |
|---|---|---|---|---|
| 1 | `character new <id> --profile biped-3q-v1` | → skeleton `character.json`, `source/` | yes | — |
| 2 | Generate source | `guide/dummy-generation.png` (+ exploded) + brief → `source.png`, `exploded.png` | no | **user** (image generation or an artist) |
| 3 | `character annotate` | overlay sheet → landmarks in `character.json` | assisted (overlay, snapping to the silhouette) | **agent/user** place and confirm landmarks |
| 4 | `character validate` | source + landmarks → validation report (§7.4) | yes | accept WARN deviations with a reason |
| 5 | `character segment` | exploded sheet + dummy-mask prior → per-segment masks | partially (connected components in the exploded sheet are separated by construction) | **agent** reviews and corrects masks |
| 6 | `character lines` | masks → outline width measurement, outline removal, `line` layer extraction, dark-component classification (A/B/C) | partially (outline ring removal is deterministic given the silhouette) | **agent** classifies ambiguous dark components; unclassified = FAIL (construction spec AR-5) |
| 7 | `character parts` | masks + layers → `art/<part>.*.png`, reassembly consistency report | yes | **agent/user** approve hidden-region quality |
| 8 | `character mesh init <region>` | art + landmarks + profile policy → `meshes/<region>.json` (hull, loops, smoothstep weights) | yes (deterministic generator) | **agent** tunes weights; the mesh file is canonical after init |
| 9 | `character bind` | landmarks → skeleton; parts → roles/slots; z rules | yes | z-rule overrides |
| 10 | `character materials` | albedo clusters → proposed `shading_groups`, outline params | proposal only | **agent/user** approve (style decision) |
| 11 | `character contacts` | prop pose states → contact pair candidates | proposal only | **agent** authors edge masks / approves pairs |
| 12 | `character build` | → `pack/` + `pack.lock.json` | yes | — |
| 13 | `character sweep` | profile stress poses × pack → sheets, verified ranges | yes (render + detector) | **agent** visual review; records verified ranges |
| 14 | `character qa` | profile clip library retargeted → frames, detector, retarget report, evidence | yes | **agent** reviews WARNs |
| 15 | Approve | evidence → `approval` | no | **user** |

**Where artistic uncertainty lives:**
- steps 2, 3, 5 (corrections), 6 (ambiguous lines), 7 (hidden quality), 10 and 15.

Each of these produces a review sheet and records an explicit decision in `character.json`. No heuristic output becomes canonical without that record.

**Performance targets:** the routine targets from the Quality Gates apply. Per step, a warm-cache re-run is under 10 s. A full build (steps 9 and 12) that doesn't touch art is under 5 s.

---

## 14. Automatic vs agent/user-owned summary

| Category | Automatic | Agent-owned | User-owned |
|---|---|---|---|
| Deterministic transforms (guides, bind skeleton, units, retarget expansion, pack build, lock hashes) | ✓ | | |
| Measurements (pose tier, proportions, outline width, clearances, IoU/ΔE/chamfer) | ✓ | | |
| Proposals (segmentation, line classes, mesh/weights, materials, contacts) | ✓ proposal | ✓ correct / approve | |
| Landmark placement | assist | ✓ | ✓ may override |
| Accepted deviations, identity-affecting hidden anatomy | | propose | ✓ |
| Source generation, final approval | | | ✓ |

---

## 15. Validation / QA invariants

**Profile-level**
- P-1: role graph acyclic and complete.
- P-2: every landmark is inside the safe margin.
- P-3: the dummy satisfies its own pose tier.
- P-4: guides regenerate byte-identically from `profile.json`.
- P-5: clip library resolves on a synthetic full-feature pack.

**Source-level**
- S-1…S-6: §7.4 tiers.
- S-7: baked-shading detector below the review threshold, or reviewed.
- S-8: exploded consistency.
- S-9: appendage zones respected.
- S-10: for identity-bound characters (the current mascot), fidelity in the declared pose state (`sit_laptop`) against the identity reference with research §14 metrics, thresholds frozen after the first source (UCP D4).

**Pack-level**
- K-1: lock matches inputs.
- K-2: all required roles bound.
- K-3: mesh rules (construction spec AR-12..AR-17).
- K-4: no unowned dark component.
- K-5: z rules acyclic.
- K-6: verified ⊆ desired.
- K-7: bind render = reassembled source within tolerance.
- K-8: stress poses clean inside verified ranges (detector + visual review).
- K-9: retarget report has no unexplained unavailable/dropped/scaled entries.

**Engine-level**
- E-1: no character/species identifiers in engine crates outside test fixtures, enforced by a `mascotctl check` audit.
- E-2: renderer output is independent of role names; changing ids in a synthetic pack does not change pixels.

---

## 16. Versioning / hard-cut policy

- **Profile id carries the major version.**
  - Any change to role ids, hierarchy, dummy geometry or landmark semantics creates **`biped-3q-v2`**, a new directory.
  - Characters are migrated by re-annotation or rebuild, and `v1` is deleted once no character uses it. Hard cut; no dual support.
  - **Exception:** a real external boundary, such as user-distributed characters, if the Planner ever names one.
- Additive profile changes (a new appendage type, a new socket) are **minor versions** inside `profile.json`, and the hash recorded in sources changes. Sources remain valid if the validation re-run passes.
- **Formats:** `mascot-character-source/1`, `mascot-character/1`, `mascot-character-pack/1`, `mascot-clips/0.4`.
  - Each replaces its predecessor directly.
  - `mascot-rig/0.2` (`rig.json`) and `mascot-clips/0.3` are retired when the pack loader lands. No loader keeps the old formats.
- **The dummy guide is a derived artifact of the profile version.**
  - The source manifest records the guide hash used for generation.
  - A guide change without a geometry change (a render-style tweak) regenerates the guides, but does not invalidate sources.

---

## 17. Mapping from current runtime/types to generic equivalents

Based on read-only inspection (committed HEAD + paused worktree).

| Current | Generic equivalent | Change class |
|---|---|---|
| `Skeleton`, `Bone{id, parent, rest, safe_rotation_deg}` | Same evaluator. `id` = role id; `rest` derived from landmarks; `safe_rotation_deg` → verified limits from the pack | KEEP evaluator; ADAPT construction |
| `Transform`, `BoneOffset`, `Pose` | KEEP. `Pose.part_opacity` keyed by pack part id is character data; semantic clips must not address parts, so opacity tracks target roles/states | ADAPT |
| `Attachment::{Sprite, Mesh}`, `skin_mesh` | KEEP (pack slot geometry) | KEEP; validation per AR-12 |
| `AttachmentDef{part, role: fill\|line, z}` | Pack slot + layers (`fill`/`line`/`mark`); z resolved from profile bands + rules | ADAPT |
| `OutlineDef{radius}` | `materials.outline.width_h` × H | ADAPT |
| `JointGuardDef` + `qa.json` joints (`neck_seam`, `tail_root`, …) | Derived from profile QA anchors + character landmarks | RETIRE the hand list |
| `rig.json` (`mascot-rig/0.2`) | `character.json` + `pack/` | RETIRE (hard cut) |
| `clips.json` `mascot-clips/0.3` (groups, tuples, `target.property`) | `mascot-clips/0.4` (roles, profile groups, units, requires/optional, mode) | ADAPT grammar kept |
| `Clip`/`Track`/`Key`/`Property` | KEEP; `Track.target` resolves roles; add unit scale at expansion | KEEP + ADAPT expansion |
| `IdleDirector`, `IdleAction` | KEEP (already name-free); schedule from `idle.json` | KEEP |
| `Player` | KEEP | KEEP |
| `RootPlacement.mirror` | KEEP as the view mirror (§3.5) | KEEP |
| Renderer (sprites, runtime outline, shadow from `world[0]`) | KEEP; no character names found in `mascot-render-win32`. Mesh path still missing (research §11) | KEEP + future mesh pass |
| `mascot-animation-lab` `main.rs`: hard-coded bone list (lines 46–57), clip names (355, 816–822), `head`/`arm_near_upper` lookups (387, 395, 874) | Profile-driven role lists, clip list from the library, demo poses from profile stress poses | ADAPT (app is character-specific today) |
| `mascotctl` `cmds.rs:602` stress bone list | Profile stress-pose set | ADAPT |
| `mascotctl` `qa.rs` self-test fixtures using `head`/`neck`/`tail_root` | Synthetic profile pack fixtures | ADAPT |
| `tools/rig_art` seated decomposition, junction morphology | Onboarding steps 5–8 on exploded sources | RETIRE |
| Engine crate tests using `head`/`neck`/`body` names | Acceptable as fixtures; prefer profile role names once the profile exists | KEEP |

**Audit method:** grep for quoted character/segment identifiers across `crates/**/src` and `apps/**/src`.
- **69 hits.**
- **Engine crates** (`mascot-animation`, `mascot-render-win32`): hits are only in `#[cfg(test)]` fixtures, plus one doc comment in `mascot-qa`.
- **Apps** (`mascot-animation-lab`, `mascotctl`): contain real hard-coded rig names. These are the generalization work items above.

---

## 18. Robot thought experiment

**Character:** "Unit-7", a small friendly white service robot.
- Rounded head with a dark visor and two round cyan LED eyes;
- one antenna on the head top;
- no ears; no tail;
- cylindrical rigid limbs; mitten-grippers; flat boots;
- thin uniform outline.

| Step | What happens | Engine/profile change needed? |
|---|---|---|
| Generate | User gives `dummy-generation.png` + "white service robot in exactly this pose"; then the exploded sheet | No |
| Annotate/validate | The 20 core joints sit at the robot's joint discs; `head.face` = visor front; `eye.right/left` = LED centres; antenna chain landmarks. Pose tier passes (same dummy). Proportions: head 0.33 H, inside the envelope | No |
| Appendages | `appendage.antenna.0` on `socket.head.top.center`, 2 bones; no tail, no ears | No (types exist) |
| Deformation | `character.json` `deformation`: all limbs `rigid-hinge`, torso `rigid`; **no meshes** | No; data only |
| Hidden art | Joint discs drawn complete in the exploded sheet (natural for robots) | No |
| Lines | Thin outline measured (`width_h ≈ 0.006`) → runtime ring; visor edge and panel seams = class C | No |
| Materials | `shading_groups` with the `band` model (a metallic highlight band) plus `rim`; LED eyes unshaded | Only if `band` is not yet in the closed model list (a one-time engine addition, not robot-specific) |
| Clips | `blink` → `@eyes.scale_y` on the LEDs ✓. `look_forward` ✓. `small_head_tilt` ✓. `stretch` ✓ (scaled to the robot's verified ranges). `ear_twitch` → requires `@ears` → **unavailable** (reported). `tail_flick` → requires `@tail` → **unavailable**. An `antenna_wiggle` clip on `@antennae` is available to any pack with antennae | No |
| Props | Laptop mounts via `sit_laptop`; contacts `hand.right`↔`lid` use the robot's edge masks | No |
| QA | ROIs from profile anchors + robot landmarks; stress poses from the profile | No |

**Robot-substitution audit (Gate 20), bounded surface:** every contract element in §3–§16 plus the §17 code map. Each element was classified as works unchanged, works via data, or needs engine work.

**Needs engine work (neither robot- nor mascot-specific):**
1. Mesh render path (already known).
2. Contact-line mechanism (Prototype A).
3. Runtime shading models (`top_light`, and possibly `band`).
4. Pack loader, retargeter and `mascot-clips/0.4`.
5. Making the apps (lab, `mascotctl` stress/self-test) profile-driven.

**Result:** no remaining element needs a robot-specific branch. **One concept failed the test and was moved to data:** *mandatory weighted meshes at shoulders and hips* (research AR-10) became a profile *default* that the pack can override (§3.8).

**Robot-specific friction found (not a contract change):** metallic highlight shapes may need the `band` model. The model list is closed and versioned, so adding one is an engine feature available to every character.

---

## 19. First bounded implementation slice (proposal; not started)

**Goal:** prove universality at the contract level without touching animation or rendering.

1. **RigProfile schema + `biped-3q-v1` `profile.json`:**
   - roles, hierarchy, landmarks, sockets, zones, envelope, pose-tier constraints, proportion envelope;
   - z bands, deformation defaults, units, groups;
   - loader + validation (P-1…P-3) in a new small crate (`mascot-profile`), with no renderer dependency.
2. **`mascotctl profile guide`:** deterministic renders `dummy-generation.png`, `dummy-annotated.png`, `dummy-mask.png`, `dummy-exploded-generation.png`, plus a byte-identity test (P-4).
3. **CharacterSource + `character.json` schemas** (`source/1`, `character/1`, landmarks + deviations only).
4. **`mascotctl character validate`:**
   - format, pose, topology, proportion and envelope tiers;
   - a review sheet with an overlay.
5. **Fixtures:**
   - **synthetic robot source:** procedurally drawn from the dummy capsules with robot-like shapes (disc joints, visor, antenna), plus its annotation;
   - **current-mascot source:** requires a user-generated candidate over the dummy (decision U1). If unavailable, a *synthetic mascot-proportioned* fixture (large head, short legs, tail) is used instead;
   - **negative fixtures:** crossed arm, left-facing, cropped foot, arm fused to torso. Each must FAIL with the expected reason.
6. **Acceptance:** both positive sources validate against the same profile with zero engine/profile branching. The negatives fail with the correct tier. No `rig.json`, clip or renderer change.

**Explicitly excluded from the slice:** segmentation, meshes, the pack builder, the retargeter, clip format 0.4, renderer and animation changes.

---

## 20. Open Planner/user decisions

| # | Decision | Agent recommendation |
|---|---|---|
| U1 | Who produces the first current-mascot CharacterSource over the dummy (user via ChatGPT, artist, agent)? | The user generates from `dummy-generation.png` + `assets/mascot.png` as the identity reference, after the slice ships guides |
| U2 | Approve the exact dummy geometry (§4–§5), or request proportion changes, e.g. a larger head closer to the mascot | Approve as the middle of the envelope; the mascot fits via the proportion envelope |
| U3 | Adopt the exploded-sheet requirement for production sources | Adopt; the alternative is agent inpainting of joint overlaps, which reintroduces the flattened-image problem |
| U4 | Pack committed as a generated artifact with a lock file vs built at startup | Committed + lock (deterministic, reviewable, matches D6 practice) |
| U5 | Retarget range policy default: `scale` for additive idle clips | Accept |
| U6 | Clip direction renames (`look_left/right` → `look_back/forward`) at the 0.4 hard cut | Accept |
| U7 | The current mascot's canonical identity is verified in the `sit_laptop` pose state; the seated thigh may need a substitution attachment if mesh ranges can't reach it | Accept; decide substitution after the first mesh sweep |
| U8 | Closed list of shading models (`top_light` first; `band` / `rim` later) | Accept; no per-character shaders |
| U9 | Landmark annotation by agent/user first; automatic estimation later as an aid | Accept |
