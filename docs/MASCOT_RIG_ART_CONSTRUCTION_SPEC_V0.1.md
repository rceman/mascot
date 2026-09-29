# Mascot Rig Art Construction Specification v0.1

**Status:** authoritative Planner contract for articulated-art construction before further rig repair  
**Applies to:** MASCOT-RIG-WIN-002 and future mascot rig/art revisions  
**Primary principle:** correct articulated art first; skeleton/joints second; verified safe ranges third; animation clips and artifact QA only after art readiness

This document defines what "correctly prepared 2D articulated art" means for Mascot.

The purpose is to prevent the rigging workflow from compensating for incomplete art with increasingly complex global morphology, junction heuristics, detector thresholds, or one-off repair scripts.

---

## 1. Required construction sequence

The canonical sequence is:

    canonical flattened art
        -> semantic part inventory
        -> complete each movable part including hidden geometry
        -> reconstruct parent surfaces hidden by movable children
        -> assign line/stroke ownership
        -> define skeleton hierarchy
        -> define explicit joint contracts
        -> define draw order / occlusion independently of hierarchy
        -> choose deformation class per joint
        -> run joint-local static sweeps
        -> repair art / pivot / deformation model
        -> derive verified safe ranges
        -> ART_READY
        -> author animation clips
        -> run frame-by-frame animation QA

Do not invert this sequence by authoring broad motion first and then trying to make morphology/QA compensate for invalid art.

---

## 2. External rigging principles adopted by Mascot

Mascot intentionally adopts several established 2D rigging principles.

### 2.1 Hidden material must be drawn before it is exposed

When a source illustration is divided into movable parts, regions hidden in the rest image must be reconstructed when motion can reveal them.

This applies to both sides of an overlap:

- the movable child needs a complete hidden root;
- the parent/background surface underneath the child also needs to remain visually valid when the child moves away.

Do not treat missing hidden material as a detector problem.

### 2.2 Rotation centers are authored semantics

A pivot/joint is not merely the nearest pixel intersection or center of a detected junction.

It is an authored semantic rotation center chosen for the intended motion.

A pivot should normally lie inside the concealed joint/overlap region, not directly on a raw crop edge.

Contact-specific pivots are allowed only when maintaining that contact is part of the intended motion.

### 2.3 Bone hierarchy and draw order are separate contracts

Parent/child transform hierarchy does not determine rendering order.

Every attachment has explicit draw-order/occlusion semantics.

A part may be parented to one bone while being drawn above or below attachments owned by other bones.

### 2.4 Rigid rotation is not mandatory for every joint

Use the simplest deformation class that remains visually valid across the required motion.

If a joint cannot reach the desired range cleanly as a rigid attachment without increasingly artificial hidden geometry, escalate the deformation model instead of stacking repair heuristics.

---

## 3. Semantic part construction

A movable part MUST be authored as a complete articulated asset, not as only the pixels visible in the rest pose.

For every movable part record:

- semantic part name;
- parent bone;
- attachment owner;
- visible rest region;
- hidden continuation/root;
- intentional internal line art;
- external contour ownership;
- draw layer/slot;
- occlusion relationships;
- pivot/joint;
- deformation class;
- desired range;
- verified safe range.

### 3.1 No raw crop edges inside a declared safe range

A source cut edge may exist in authoring intermediates, but it MUST NOT become visible anywhere inside the final verified safe range.

A visible:

- straight crop line;
- rectangular edge;
- abrupt clipped highlight;
- truncated shading band;
- black ledge;
- isolated stroke fragment

is an art-construction failure.

### 3.2 Shared source strokes must be reassigned intentionally

A single black stroke in the flattened source may visually belong to two objects while they are touching, for example:

- chin / laptop lid;
- paw / laptop lid;
- foot / laptop;
- knee / laptop;
- ear rim / back.

If those objects can move apart, the shared flattened stroke cannot simply be cut and left on both resulting parts.

For every shared stroke that crosses a future articulation boundary:

1. identify which visible contour each separated object should own;
2. reconstruct missing contour/fill where separation exposes it;
3. remove orphan stroke fragments from the wrong part;
4. keep intentional internal line art separately classified;
5. verify the separated parts at motion extremes.

Do not accept "detector clean" when the separated contour looks visibly cut.

---

## 4. Hidden geometry contract

Hidden geometry is required wherever motion can reveal pixels not visible in the canonical rest image.

### 4.1 Motion-envelope rule

Hidden geometry size MUST be justified by the intended transform envelope, not by a global magic number.

Conceptually:

    required_hidden_region =
        union(newly_exposed_region(transform(part, pose))
              for pose in required_joint_range)

Then require:

    actual_hidden_art
        covers required_hidden_region
        + anti-alias / raster safety margin

The implementation may approximate this envelope conservatively, but a single project-wide value such as:

    JUNCTION_REGION = 60

is not authoritative joint semantics.

Global morphology values may be used only as candidate-discovery/build heuristics.

### 4.2 Prefer generous hidden art over brittle exact cuts

Where cost is small, hidden fill should extend beyond the minimum currently visible requirement so small later tuning does not immediately expose raw edges.

This does not justify hidden shapes extending into unrelated visible areas.

### 4.3 Hidden geometry must preserve local art quality

Newly exposed regions must match surrounding:

- palette;
- shading direction;
- highlight continuity;
- line-art style;
- silhouette quality.

A hidden patch that prevents transparency but looks like flat cut paper is not acceptable.

---

## 5. Rigid-joint concealed-root pattern

For rigid or near-rigid rotation, the child root should normally extend behind its parent around the pivot.

Preferred conceptual pattern:

    parent occluder
    ################
    ####   (hidden child root)
    ####      *--------- visible child
    ################

    * = pivot

The hidden root should remain concealed over the verified range and should not carry an outer black contour where it is meant to stay internal.

For small rigid joints, a rounded/capsule/circular hidden root is often preferable to a straight cut because rotation around the pivot does not expose a directional crop edge.

This is a design pattern, not a requirement that every joint be literally circular.

---

## 6. Pivot / joint placement rules

Every joint MUST have an explicit authored pivot.

The pivot MUST be chosen using semantic motion intent and visual contact, not solely pixel morphology.

Review the following:

- anatomical/mechanical center of rotation;
- concealed overlap region;
- intended contact point;
- leverage radius to visually sensitive contacts;
- motion envelope;
- whether the parent should visually remain stationary;
- whether rigid rotation is even the correct deformation model.

### 6.1 Avoid pivots on raw boundaries

A pivot lying directly on an exposed cut boundary creates triangular gaps/steps immediately under rotation.

Prefer placing the pivot inside the concealed overlap zone when the motion permits it.

### 6.2 Contact-constrained pivots

A pivot may intentionally be placed at a contact such as chin/laptop only if:

- that contact should remain visually fixed or nearly fixed for the motion;
- the resulting head/neck deformation remains plausible;
- the rest of the joint has sufficient hidden geometry.

Do not move pivots solely to hide one artifact if it creates worse motion elsewhere.

---

## 7. Joint contract schema

Every articulated connection MUST have a machine-readable joint contract.

Recommended fields:

    id
    parent_part
    child_part
    parent_bone
    child_bone
    pivot
    deformation_class
    desired_range_deg
    verified_safe_range_deg
    child_hidden_root
    parent_hidden_surface
    draw_order
    occlusion_rule
    shared_stroke_policy
    contact_constraints
    qa_rois
    allowed_internal_lines
    notes

The exact serialization may integrate with existing rig/QA data rather than creating duplicate sources of truth.

Do not create a second canonical rig schema.

---

## 8. Deformation classes

Each joint MUST declare the simplest valid deformation class.

Suggested vocabulary:

### 8.1 rigid-hinge

Good for:

- elbows;
- knees;
- small ear articulation;

when the visible style supports a hinge-like split.

### 8.2 rigid-socket-small-range

Good for:

- shoulders;
- hips;
- neck/head;

only when hidden overlap and limited range keep the result visually clean.

### 8.3 multi-bone rigid chain

Good for:

- tail;
- longer flexible appendages;
- ears when one rigid pivot is insufficient.

### 8.4 weighted-mesh / deformable attachment

Use when required motion cannot remain clean with rigid parts without implausible art reconstruction.

Candidates include:

- shoulder;
- hip;
- neck;
- tail/body transition;

when the desired range is materially larger than a clean rigid range.

Escalation to a deformable model is preferable to adding layers of ad-hoc geometry patches around a structurally unsuitable rigid joint.

---

## 9. Draw order and occlusion

Draw order MUST be explicit and independent from transform hierarchy.

For every moving part specify:

- what it is normally behind;
- what it is normally in front of;
- whether the order changes during motion;
- what clips/occludes its hidden root.

Typical Mascot examples include:

    far arm
      < body
      < near arm
      < laptop / foreground paw details

but actual ordering must be derived from the canonical artwork.

Do not let bone parenting implicitly decide rendering order.

---

## 10. Outline and line-art ownership

Mascot's external outline is a composition concern.

### 10.1 External silhouette

The final external mascot outline should be generated from the composed silhouette.

Do not bake a complete outer outline around every moving fill attachment when that produces internal doubled contours.

### 10.2 Internal line art

Intentional internal lines remain separate classified artwork.

Examples:

- facial lines;
- laptop details;
- intentional folds/creases;
- explicitly approved ear crease.

### 10.3 Hidden joint roots

Hidden roots MUST NOT introduce black perimeter fragments that become internal wedges/slivers when parts rotate.

A newly visible black line inside a joint is presumed suspicious unless it belongs to an explicit allowed internal-line feature.

---

## 11. Art-readiness sweep before clip authoring

Every joint MUST pass a static sweep before animation clips are considered authoritative.

For each joint:

1. render rest;
2. render desired negative extreme;
3. render intermediate negative poses;
4. render zero;
5. render intermediate positive poses;
6. render desired positive extreme;
7. inspect full character;
8. inspect joint crop at 4x-8x;
9. run automatic artifact analysis;
10. classify every ERROR/WARN;
11. repair art/pivot/deformation or reduce the range;
12. repeat.

Do not use animation playback as the first place joint construction is validated.

---

## 12. Desired range vs verified safe range

These are different concepts.

    desired_range = product/animation target
    verified_safe_range = range proven visually acceptable

Never set:

    safe_range = desired_range

without a successful sweep.

The verified safe range is the largest continuous interval around the intended rest region that passes:

- automatic artifact checks;
- joint-local zoom review;
- actual human/agent visual review.

Apply a small safety margin when a boundary pose is only marginally clean.

If the desired range materially exceeds the verified range:

1. improve hidden art;
2. reconsider pivot;
3. reconsider occlusion/draw order;
4. escalate deformation class;
5. only then reduce the product range if necessary.

---

## 13. Art Readiness evidence

Before a joint is marked `ART_READY`, generate a compact review artifact containing:

- isolated parent;
- isolated child;
- hidden regions tinted;
- pivot marker;
- desired arc;
- verified safe arc;
- rest composition;
- min safe pose;
- representative mid pose(s);
- max safe pose;
- 4x-8x joint crops;
- detector summary;
- shared-stroke / line-ownership notes.

Preferred tooling direction:

    mascotctl art joint <joint>
    mascotctl art joint <joint> --sheet
    mascotctl art joint <joint> --sweep
    mascotctl art readiness
    mascotctl art readiness --json

Exact syntax may adapt to existing CLI ownership.

Do not create ad-hoc scripts when an equivalent durable capability already exists.

---

## 14. TDD / machine-verifiable invariants

Where practical, every joint should have automated invariants for:

- pivot exists and lies within declared joint area;
- desired/verified ranges are explicit;
- raw source cut edge never appears in a safe pose;
- expected parent/child coverage remains continuous;
- hidden child root remains plausibly covered;
- parent hidden surface has no transparency holes;
- no unexpected internal dark component appears;
- no orphan stroke remains when the child moves;
- draw order is deterministic;
- rest pose remains within accepted fidelity tolerance;
- safe sweep has zero unresolved ERROR findings;
- WARN findings are explicitly reviewed.

A detector PASS is necessary but not sufficient.

Actual rendered visual review remains mandatory.

---

## 15. Global heuristic limits

The following are NOT allowed as final joint authority:

- one global junction radius for every joint;
- one global hidden-extension distance;
- nearest-fill intersection as the sole pivot definition;
- morphology-derived joint candidates treated as final semantic joints;
- increasing global regions until gaps disappear;
- accepting a range only because detector ERROR count is zero.

Heuristics may propose candidates or accelerate build steps.

Semantic joint contracts and rendered evidence decide correctness.

---

## 16. Current MASCOT-RIG-WIN-002 construction findings

The paused agent's findings are incorporated as requirements for the next pass.

### 16.1 Eyes

The head fill must remain continuous beneath eye/highlight/blink artwork.

Blink/double-blink must preserve that continuity with zero visual black-dot regression.

### 16.2 Head / chin / laptop

The previous neck-junction pivot caused excessive chin/laptop displacement.

The current contact-oriented pivot candidate at canvas approximately `(800,610)` is a useful hypothesis, not automatically final authority.

Research and verify:

- intended head motion center;
- neck deformation plausibility;
- chin/lid contact semantics;
- safe sweep;
- whether rigid head rotation is sufficient.

### 16.3 Shared strokes

Explicitly resolve:

- chin / laptop lid;
- paw / laptop lid;
- foot / laptop;
- knee / laptop;
- ear rim / back;

and any other shared source stroke discovered by inventory.

### 16.4 Cut-paper hidden fills

Reconstruct or redesign hidden regions at:

- shoulder;
- hips;
- tail root;
- far foot;
- below ear;
- other detected straight-edge/highlight-cut regions.

### 16.5 Overextended hidden shapes

Audit hidden reconstruction against occluders.

A hidden extension must not escape unrelated silhouettes, such as laptop-screen fill appearing beyond the screen.

### 16.6 Orphan strokes

Audit for line art that remains on the body when its corresponding moving child separates.

### 16.7 Current measured clean ranges are provisional

The paused agent reported detector-clean ranges, including asymmetric and very narrow intervals.

These values are diagnostics only.

They MUST NOT become final safe ranges until:

- art construction is repaired;
- research is completed;
- joint contracts are explicit;
- static sweeps are rerun;
- visual review passes.

The agent also reported that near/far arms can pass the detector at ±15° while still failing visually. This is direct evidence that automatic ERROR count alone is insufficient.

---

## 17. Mandatory agent research pass before structural repair resumes

Before making further structural art/joint changes, the implementation agent MUST perform an independent focused research pass.

Use primary/official sources where available.

At minimum research:

- 2D material/part separation for articulated characters;
- hidden geometry / redraw rules;
- pivot / rotation-center placement;
- rigid vs deformable/weighted attachments;
- parent-child hierarchy;
- draw order / occlusion independent of hierarchy;
- mesh weighting / skinning for shoulders, hips, neck, tail;
- overlap/shared-line handling;
- safe motion range validation;
- practical QA for exposed cut edges/seams.

Starting official references:

- Live2D Cubism — Illustration Processing / Material Separation:
  https://docs.live2d.com/en/cubism-editor-tutorials/psd/
- Live2D Cubism — Deformers / Rotation Deformer:
  https://docs.live2d.com/en/cubism-editor-manual/deformer/
  https://docs.live2d.com/en/cubism-editor-manual/making-and-rotation-of-rotationdeformer/
- Spine — Basic Concepts / Slots / Draw Order:
  https://esotericsoftware.com/spine-basic-concepts
  https://esotericsoftware.com/spine-slots
- Spine — Meshes / Weights:
  https://esotericsoftware.com/spine-meshes
  https://esotericsoftware.com/spine-weights

The agent should expand beyond these sources when another authoritative reference materially improves the contract.

### Required research output

Create:

    docs/MASCOT_RIG_ART_CONSTRUCTION_RESEARCH_V0.1.md

It MUST contain:

1. sources and exact relevant principles;
2. what is directly applicable to Mascot;
3. what is not applicable and why;
4. disagreements/tradeoffs between approaches;
5. concrete rules the agent proposes adding;
6. component-specific recommendations for:
   - head/neck;
   - ears;
   - shoulders/arms/paws;
   - hips/legs/feet;
   - tail;
   - laptop contact/occlusion;
7. recommended deformation class per joint;
8. proposed art-readiness checks;
9. tooling gaps discovered during research.

After the research note is complete, the agent MUST add an **Agent Research Addendum** section to this specification with evidence-based additions.

The addendum MAY strengthen or make the Planner rules more specific.

It MUST NOT silently weaken or contradict existing Planner MUST/FAIL rules.

If research indicates a Planner rule is wrong or counterproductive, stop and report the conflict explicitly before changing that rule.

Only after this research/addendum pass may structural rig-art repair resume.

---

## 18. Completion effect on the current task

MASCOT-RIG-WIN-002 cannot be complete until:

- the research pass is committed;
- this construction spec has the agent research addendum;
- every required moving joint has an explicit joint contract;
- art-readiness sweeps pass for final safe ranges;
- shared strokes and orphan strokes are resolved;
- hidden geometry is visually plausible;
- desired vs verified safe ranges are distinguished;
- animation clips are regenerated against the ART_READY rig;
- frame-by-frame animation QA is rerun;
- final evidence matches the materially final candidate;
- Universal Gates 1-20 pass.

The construction contract is upstream of animation QA.
