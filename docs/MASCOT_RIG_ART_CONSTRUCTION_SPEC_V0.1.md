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

## 17. Dynamic line model and continuous-anatomy deformation

The current brainstorm identified a second architectural boundary that MUST be researched before more art surgery: **not every black line in the flattened source should remain baked into a movable part**.

Mascot must classify line art by semantic ownership rather than by source pixels alone.

### 17.1 Three line classes

Every black/dark stroke MUST be classified as one of:

#### A. External silhouette outline

The outer mascot contour.

Current repository behavior already follows the intended model:

    composed fill silhouette
        -> runtime outline mask
        -> outline rendered under the color composite

This is preferred over baking a complete outer black contour into each moving part.

The runtime outline must remain composition-derived.

#### B. Contact / separation boundary between independently moving parts or objects

Examples:

- chin / laptop lid;
- paw / laptop lid;
- foot / laptop;
- another pair that touches in rest but may separate in motion.

These lines are candidates for **runtime-generated internal boundary strokes** instead of duplicated baked line fragments.

The purpose is to make the boundary follow the actual posed geometry rather than leaving a cut-off source stroke on one or both parts.

This is an architectural hypothesis requiring research and prototype validation before becoming the final rendering contract.

A likely model is:

    final posed fill layers
        + part/attachment identity
        + draw-order / occlusion
        + pair-specific boundary policy
        + optional local edge mask
        -> internal contact boundary

Do NOT blindly outline every part boundary.

Many anatomical overlaps must have no visible internal line.

Per-pair policy and/or local edge masks are required so a shoulder, neck or hip does not receive an artificial seam merely because two attachments meet there.

#### C. Intrinsic line art that belongs to one deforming surface

Examples:

- eyes;
- nose;
- mouth;
- whiskers;
- laptop logo/details;
- intentional ear crease;
- intentional folds/form lines that remain part of one body surface.

These remain authored art and move/deform with their owning attachment.

They are not reconstructed from part boundaries.

### 17.2 Runtime-generated internal contact line prototype

Before adopting runtime-generated contact lines globally, prototype at minimum:

- chin / laptop lid during head motion;
- near paw / laptop lid during arm motion.

Compare:

1. current baked-line rendering;
2. generated contact-boundary rendering;
3. rest-pose fidelity against `assets/mascot.png`;
4. positive/negative motion extremes;
5. line continuity and endpoints;
6. effect on current artifact detector findings.

The prototype must answer:

- how the front/visible boundary is selected;
- how line thickness is defined;
- how anti-aliasing behaves;
- how stylized tapered/end-point strokes are preserved where necessary;
- how local edge masks are represented;
- whether a part-ID buffer, pairwise masks, or another representation is simpler;
- whether the line is drawn under or over relevant color layers;
- performance cost.

Do not convert all internal lines until the prototype proves a useful visual improvement.

### 17.3 Joint underpaint / pivot seal

A joint may use a simple unoutlined base-color underpaint beneath the rotating connection.

Conceptually:

    parent surface
        + concealed underpaint around pivot
        + moving child root

The underpaint may be circular, capsule-like or another conservative shape.

Its purpose is only to prevent transient transparency or tiny uncovered gaps.

It is a **safety layer**, not the primary solution for anatomical continuity.

Requirements:

- no independent outer black outline;
- stays concealed in the normal safe range where possible;
- does not introduce an obvious flat-color patch;
- must not replace correct hidden shading or mesh deformation;
- its extent must be joint-specific, not one global radius.

If the underpaint becomes visibly flat or changes the intended silhouette, the joint construction is still wrong.

### 17.4 Continuous anatomy requires continuous deformation

When two regions are visually one continuous body surface, a rigid cut between them may be structurally incapable of producing the required motion.

Examples:

- hip / upper leg where the rump contour should flow into the thigh;
- shoulder / upper arm;
- tail root / torso;
- neck / torso or neck / head for larger ranges;
- possibly ear base where rigid motion produces a visible cut.

In these cases, a generated boundary line cannot solve the core problem.

The surface itself must deform continuously.

Weighted mesh / skinning is the preferred escalation candidate.

The repository already contains:

- `Attachment::Mesh`;
- mesh vertices / UVs / triangles / per-vertex bone weights;
- linear-blend skinning in `skin_mesh`;
- bind-pose/follows-bones tests.

The current Windows renderer does **not** render mesh attachments; mesh slots are currently skipped during bitmap upload/render preparation.

Therefore, do not re-design the animation model to introduce mesh support: the data/runtime model is already present.

The missing work is primarily:

- a renderer path for textured skinned triangles;
- actual mesh art/topology;
- practical weight authoring/generation;
- mesh-specific QA.

The implementation agent MUST research the narrowest suitable renderer path. D3D11 interop is an obvious candidate because the renderer already owns a D3D11 device, but it is not pre-approved as the only implementation.

### 17.5 Mesh authoring rules

If a joint is promoted to weighted mesh:

- the mesh must reproduce the accepted rest pose closely;
- topology must be denser where deformation gradients are high;
- weights must sum to 1;
- influences must remain bounded;
- weights should vary smoothly across the transition zone unless a hard boundary is intentional;
- the transition width must be joint-specific;
- automated initial weights may use bone distance or another deterministic heuristic;
- the final weights remain explicit inspectable data, not opaque runtime magic.

Do not generate a huge uniform grid over the entire mascot.

Mesh only the region that materially benefits from deformation.

### 17.6 Mesh QA / TDD

For every weighted mesh attachment verify, where practical:

- bind/rest pose identity within tolerance;
- no triangle inversion inside the verified safe range;
- no degenerate triangles;
- bounded stretch/compression;
- stable UV coverage;
- no unexpected texture holes;
- no exposed atlas/background pixels;
- weight normalization;
- deterministic vertex output;
- deterministic draw order;
- correct external runtime outline from the final deformed silhouette;
- intrinsic line art deforms with the surface as intended;
- safe-range visual review at 4x-8x.

The exact stretch/compression thresholds are joint-specific and should be proposed in the agent research addendum.

### 17.7 Architecture decision matrix

Use this as the starting classification:

| Relationship | Preferred starting model |
|---|---|
| Separate objects that merely touch, e.g. chin/laptop or paw/laptop | complete independent fills + contact-boundary policy/runtime internal line candidate |
| Small rigid articulation with concealed root | rigid sprite + correct hidden geometry + optional joint underpaint |
| Continuous anatomical surface that must bend/flow | weighted mesh / deformable attachment |
| Long flexible appendage | multi-bone chain, optionally weighted mesh |
| Intrinsic detail inside one part | authored line art attached/deformed with that surface |
| Final external silhouette | runtime-generated from final composed/deformed fill |

This table is a research baseline, not permission to skip joint-specific analysis.

### 17.8 Important non-solution

Do not attempt to make a rump/thigh contour "flow" using only a generated black line while the underlying fill still consists of two visibly rigid cut-paper shapes.

Line generation solves line ownership.

Skinning/deformation solves continuous anatomy.

These concerns must remain separate.

---

## 18. Mandatory agent research pass before structural repair resumes

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

## 19. Completion effect on the current task

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

---

## Agent Research Addendum

**Source:** `docs/MASCOT_RIG_ART_CONSTRUCTION_RESEARCH_V0.1.md` (MASCOT-RIG-RESEARCH-003). Section references `R§n` point there.

**Status:** agent-proposed. These rules strengthen or specialize the Planner rules above. None of them weakens a Planner MUST/FAIL rule. Numeric thresholds are **proposed starting values** to be confirmed by the prototypes in R§17. They are not verified results.

### A. Conflicts and divergences reported for Planner decision

No existing MUST/FAIL rule in this document is contradicted. One divergence is reported explicitly rather than applied:

- **§1 construction sequence / neutral-master hypothesis.**
  - §1 starts from "canonical flattened art", and the research-only handoff frames the target as a "purpose-built *neutral* articulated art master".
  - The research recommends a purpose-built layered master whose **bind pose is the canonical seated 3/4 pose**, not a neutral T/A-pose (R§4–R§5, decision D1).
  - If D1 is accepted, §1's first step would read `canonical visual identity reference -> purpose-built layered complete-part master (bind = canonical seated pose) -> semantic part inventory -> …`.
  - This text is **not** changed here; it awaits Planner decision.

### B. Art source

- **AR-1.** The flattened `assets/mascot.png` MUST NOT be the direct source of production rig parts for the next rig revision. It remains the identity and rest-pose reference, and the reconstruction metrics of AR-4 are measured against it.
- **AR-2.** Production parts MUST come from a layered complete-part master. The master keeps, per part:
  - a `fill` layer (albedo), with no outer contour;
  - an optional intrinsic `line` layer (class C only);
  - optional `mark` layers.

  Runtime part images are derived build output of the master (single canonical source; Universal Gates 7/9).
- **AR-3.** Non-canonical generated references (e.g. the neutral T-pose image) MAY inform the shape of hidden anatomy. They MUST NOT supply line weight, palette, shading grammar, proportions or new identity features.
  - **Measured divergences of the current reference:** stroke median ≈15 px vs ≈29 px; inverted shading grammar; an invented cream belly; a mixed 3/4 head on a frontal torso.
  - Any hidden-anatomy identity choice (belly colour/marking, far-arm/paw shape) is an owner decision.
- **AR-4.** Rest reconstruction is verified against `assets/mascot.png` with these metrics (targets per R§14, pending D4):
  - silhouette IoU;
  - named-landmark error;
  - per-region albedo ΔE2000;
  - per-class line-skeleton chamfer distance;
  - SSIM, full figure and per ROI;
  - mandatory 1×/4× visual review.

  A single scalar similarity score is not sufficient.

### C. Line ownership (specializes §10 and §17.1–17.2)

- **AR-5.** Every dark component in the master MUST carry a machine-readable class (A external / B contact / C intrinsic) and an owner part. Class B also needs a partner part. An unclassified dark component is a construction FAIL.
- **AR-6.** Class B composition order is:

      colour(back) → [parts between back and front in z] → line(front,back) → colour(front)

  The line MUST be clipped to the back object's posed coverage, so it can never be drawn over the background, where class A applies. It MUST be restricted by a front-owned local edge mask that moves or deforms with the front object.
- **AR-7.** The class B edge mask defines both the extent and the thickness profile of the line. Stroke-end taper MUST be geometric (the mask band narrows from full width to 0), not an opacity fade.
- **AR-8.** A part-ID buffer MUST NOT be used as the class B source: it is aliased and loses the back object's coverage under the front. Pairwise coverage masks are the representation.
- **AR-9.** Pairs without a class B policy MUST produce no internal line. In particular: shoulder, neck, hip, tail root, torso segments.

### D. Deformation (specializes §8 and §17.4–17.7)

- **AR-10.** Starting deformation classes for the next revision (R§7), subject to joint sweeps:

  | Region | Class |
  |---|---|
  | torso (hips/body/chest) | weighted mesh |
  | near shoulder / upper arm / forearm | weighted mesh chain |
  | near hip / thigh / knee | weighted mesh chain |
  | tail | multi-bone chain + weighted mesh |
  | head | rigid socket over a complete neck continuation reaching about mouth level |
  | ears | rigid socket with concealed root |
  | paws, feet, laptop lid | rigid hinge |
  | far limbs | rigid concealed socket until a sweep proves a visible seam |

- **AR-11.** Large pose changes (standing, walking, dangling while dragged) MUST use attachment substitution (a separate posture family with its own bind meshes, on the same skeleton), not deformation of the seated meshes beyond their verified ranges.

### E. Mesh data and QA (specializes §17.5–17.6)

- **AR-12.** Mesh validation MUST reject rather than silently repair each of these:
  - any vertex with more than 4 influences;
  - a negative weight;
  - weights not summing to 1 (±1e-4);
  - an out-of-range triangle index;
  - a bind triangle with minimum interior angle below 15°;
  - inconsistent winding.

  Normalization and pruning (influences < 0.02) happen in the authoring tool, never at load time.
  - *Code finding:* `Slot::from_def` currently renormalizes silently. This must change when meshes are first used; that is an implementation-phase change, not made here.
- **AR-13.** A part's fill and intrinsic line layers MUST share one mesh geometry (same vertices, UVs and weights). Duplicating geometry per layer is prohibited.
- **AR-14.** Meshes MUST NOT self-overlap. Overlap is expressed by separate slots and explicit z.
- **AR-15.** Topology and weights (proposed defaults):
  - hull 2–4 px outside the alpha edge at 1×, in transparent padding;
  - transition-band vertex spacing 1–1.5× stroke width;
  - at most 2 influences per single joint, 3 at triple junctions;
  - deterministic smoothstep initial weights;
  - band width as a fraction of limb width at the joint: hip 0.4–0.5, shoulder 0.3–0.4, tail root 0.5, torso 0.6.

  The generated mesh data is the single canonical data. The generator's parameters are not a second source.
- **AR-16.** Stretch thresholds per triangle (σ1 ≥ σ2 = singular values of the bind→posed 2×2 map), over the verified safe range:
  - det > 0 everywhere;
  - **fill-only** triangles: area ratio 0.75–1.33, anisotropy ≤ 1.35, posed area ≥ 25% of bind;
  - **line-covered** triangles: area ratio 0.85–1.18, anisotropy ≤ 1.15, posed area ≥ 50% of bind.

  A joint may tighten these values but MUST NOT loosen them without a Planner decision.
- **AR-17.** Bind identity: at bind, no pixel inside the mesh region may differ from the accepted reference render by more than 2/255 per channel away from alpha edges.

### F. Renderer (specializes §17.4)

- **AR-18.** Recommended mesh path: a D3D11 textured-triangle pass on the renderer's existing device, drawn into the D2D target bitmaps (via `ID2D1Bitmap1::GetSurface`), interleaved with sprites by z. Requirements:
  - skinning stays in `mascot-animation` (`skin_mesh`);
  - premultiplied-alpha blending;
  - edge anti-aliasing from texture alpha;
  - meshes contribute to the fill pass, so the external outline and shadow follow deformation.

  Direct2D `FillMesh` is not a valid path (aliased only, single brush transform).
- **AR-19.** A CPU reference rasterizer is permitted as a QA oracle and prototype harness only. It MUST NOT become a second production render path.

### G. Shading (extends the construction contract; pending D2)

- **AR-20.** Observation: canonical limb highlights are light insets whose lower edges are screen-horizontal.
  - If runtime shading is adopted, the stylized model is: `highlight = inset(group fill) ∩ screen half-plane anchored to a bone point`.
  - Albedo, markings and intrinsic lines stay authored.
  - Until Prototype C and decision D2, small-range rigid parts may keep baked highlights. Meshed regions MUST NOT be approved with baked highlights that visibly tilt or stretch in the 4×–8× review.

### H. Props

- **AR-21.** The laptop MUST become a separate prop rig with a switchable mount relation to the character, not a child bone of `hips`. Body art beneath it MUST be complete. Contacts (chin/lid, paw/lid, foot/base) are declared class B pairs and future IK targets.

---

## Architecture Compatibility Addendum

**Source:** `docs/MASCOT_UNIVERSAL_CHARACTER_ARCHITECTURE_V0.1.md` (MASCOT-CHAR-ARCH-004). Section references `UA§n` point there.

**Authority:** `docs/MASCOT_UNIVERSAL_CHARACTER_PIPELINE_V0.1.md` (UCP) decisions D1–D7.

**Status:** agent-proposed, for Planner review. The Agent Research Addendum above is **not rewritten**. This section records, for each agent rule, whether it is compatible with the universal direction, and proposes corrected text where it is not. Where the two differ, the corrected rule applies once Planner approves.

### U-A. Status of the Agent Research Addendum rules

| Rule | Status | Note / corrected rule |
|---|---|---|
| §A divergence (seated bind) | **Superseded by UCP D1** | The bind pose is the profile's relaxed right-facing 3/4 neutral pose (`biped-3q-v1`, UA§4–5). The seated/laptop pose is a pose state (UA§11). |
| AR-1 | Compatible, generalized | "The flattened identity reference of any character (for the current mascot `assets/mascot.png`) MUST NOT be a direct source of production rig parts. Production parts come from that character's CharacterSource over the profile dummy." |
| AR-2 | Compatible, clarified | The "layered complete-part master" is the character's prepared art (`assets/characters/<id>/art/`), derived from the assembled source plus the exploded parts sheet (UA§7.1). |
| AR-3 | Compatible, extended | Also: the dummy guide is never identity, style or proportion authority for any character. |
| AR-4 | **Corrected** | "Identity fidelity of an identity-bound character is verified in its declared identity pose state (for the current mascot: `sit_laptop`) with the multi-metric set. Numeric targets are frozen only after the first real CharacterSource prototype (UCP D4)." The research's default values remain proposals. |
| AR-5 … AR-9 | Compatible | Generic. In AR-6 the "back object" may be a prop slot. |
| AR-10 | **Corrected** (it assumed a seated bind and mascot anatomy) | "The RigProfile declares a default deformation class per joint (UA§3.8). A character MAY override any joint's class in its recipe; an override takes effect only after that joint's art-readiness sweep passes. Deformation classes are character data, not engine assumptions." |
| AR-11 | **Corrected** (mandatory substitution families conflicted with neutral bind) | "No pose may be reached by deformation beyond the verified range of any joint. A pose state MAY declare substitution attachments for specific slots (e.g. a seated thigh), in the character recipe, per slot and per state. Substitution is a data feature of the pack, not a required posture family, and is added only when a sweep shows the verified range cannot reach the state." |
| AR-12 … AR-17 | Compatible | Apply to every CharacterPack mesh. AR-16 thresholds remain proposed defaults; they may be tightened per joint and loosened only by Planner decision. |
| AR-18, AR-19 | Compatible | Renderer is character-agnostic. |
| AR-20 | **Updated by UCP D2** | "Directional shading for moving/deforming regions is runtime-owned. The engine offers a closed, versioned list of stylized shading models (first: `top_light`). Characters select models and parameters in data; no per-character shaders. Authored art may keep only rotation-safe, non-directional AO/material texture." |
| AR-21 | Compatible, generalized | "Props are separate rigs mounted to profile sockets through pose states; z insertion is relative to profile z bands; contacts are class B pairs declared in the character recipe." |

### U-B. Proposed corrected construction sequence (§1)

This is a proposal only; §1 itself is not changed here:

    RigProfile dummy guide (assembled + exploded)
        -> CharacterSource (candidate) + provenance
        -> landmark annotation + CharacterSource validation
        -> segmentation / complete part art (exploded sheet) / line ownership
        -> bind parts to profile roles; skeleton derived from landmarks
        -> joint contracts = profile defaults + character overrides
        -> z = profile bands + character rules
        -> deformation class per joint; local meshes/weights where chosen
        -> joint-local static sweeps -> verified ranges
        -> ART_READY -> CharacterPack build
        -> retarget semantic clips -> frame-by-frame animation QA

### U-C. Additional compatible rules

- **UA-1.**
  - Engine code (animation, renderer, QA engine, pack loader, retargeter) MUST NOT contain character, species or part-identity names or logic outside test fixtures.
  - Profile-specific knowledge MUST come from RigProfile data.
  - Enforced by an audit in the project check (UA§15 E-1).
- **UA-2.** Semantic clips MUST address profile roles or profile groups, never pack part ids. Translations use profile-normalized units (UA§9.1).
- **UA-3.** Joint contracts (§7) are resolved from the profile default plus the character override. They MUST NOT be hand-duplicated per character outside the recipe.
- **UA-4.**
  - CharacterPacks are derived artifacts, carrying a lock of their input hashes.
  - Landmarks, art, recipe and meshes are the canonical inputs.
  - No datum may be stored canonically in both the recipe and the pack.
- **UA-5.**
  - QA joint ROIs are derived from profile QA anchors and character landmarks.
  - Hand-maintained per-character joint lists (such as the current `qa.json` joints) are retired when the pack pipeline lands. Hard cut.
- **UA-6.** Any change to profile role ids, hierarchy, dummy geometry or landmark semantics creates a new profile major version (hard cut; UA§16).
