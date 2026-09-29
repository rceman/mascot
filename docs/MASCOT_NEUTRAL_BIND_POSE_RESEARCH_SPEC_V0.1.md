# Mascot Neutral Bind-Pose / Art Architecture Research Specification v0.1

**Status:** authoritative Planner research contract  
**Applies to:** current research-only phase of MASCOT-RIG-WIN-002  
**Implementation status:** PAUSED — research/documentation only  
**Primary question:** should Mascot move from a seated flattened illustration as the literal rig source to a purpose-built neutral articulated art master?

## 1. Why this research exists

The current seated source image is excellent as a visual identity/reference pose, but it is a poor literal source for broad articulation because:

- limbs and torso are heavily occluded by the laptop and seated pose;
- hidden anatomy must be invented from missing pixels;
- source strokes are shared across parts that later need to separate;
- directional shading is baked into regions that may rotate;
- rigid cutout decomposition produces visible cut-paper joints;
- detector-clean poses can still look visually wrong.

The research phase must determine the correct future art/rig architecture before more production repair is attempted.

Do not assume that the current seated decomposition should be repaired further.

## 2. Current reference assets and authority

### 2.1 Canonical visual identity reference

    assets/mascot.png

This remains the canonical visual identity/style reference.

It defines, as applicable:

- mascot identity;
- seated/laptop pose identity;
- characteristic silhouette;
- face;
- proportions;
- palette;
- markings;
- line-art language;
- overall rendering style.

It is **not automatically the canonical future bind-pose art source**.

### 2.2 Non-canonical neutral research reference

Expected path:

    assets/research/neutral-bind-pose/mascot-neutral-tpose-1x1-generated-v0.1.png

Expected dimensions:

    1254 x 1254 px

This image is a generated research reference only.

It may be used to reason about:

- complete unoccluded anatomy;
- near/far limb separation;
- neutral pose construction;
- possible bone placement;
- mesh topology;
- deformation zones;
- line ownership;
- shading behavior;
- future runtime pose reconstruction.

It MUST NOT be treated as approved production art.

Do not:

- replace `assets/mascot.png`;
- call the generated reference canonical;
- trace it blindly into production assets;
- lock production proportions to it without comparison;
- use its baked shading as final shading authority;
- commit structural art derived from it during this research-only phase.

## 3. Research-only boundary

This phase is documentation/research only.

Allowed:

- inspect current Rust/runtime/art/tooling code;
- inspect current generated/rig assets;
- inspect the neutral research reference;
- inspect external primary/official rigging/rendering sources;
- write research documents;
- write architecture decision proposals;
- write implementation plans/pseudocode/data-shape proposals;
- identify tooling gaps;
- identify experiments/prototypes to perform later.

Not allowed without a new Planner instruction:

- production source-code changes;
- renderer implementation;
- mesh rendering implementation;
- art reconstruction;
- hidden-geometry painting;
- pivot changes;
- safe-range changes;
- rig.json changes;
- clip changes;
- new animation clips;
- structural raster repair;
- global junction tuning;
- production shader implementation;
- production physics implementation;
- promoting the neutral generated image to canonical art.

If a tiny local experiment is indispensable to answer a research question, keep it outside the committed product tree and report exactly what it proved. It must not become an implementation side door.

## 4. Research hypothesis: separate visual identity from rig source

Evaluate the following proposed architecture:

    canonical visual identity / canonical seated pose
        assets/mascot.png
                |
                v
    purpose-built neutral articulated art master
        complete anatomy + intrinsic markings
                |
                v
    rig / skeleton / weights / constraints
                |
                v
    posed/deformed fills
                |
                +--> semantic intrinsic lines
                +--> contact/separation boundary lines
                |
                v
    runtime external silhouette outline
                |
                v
    runtime stylized directional shading
                |
                v
    final frame

The research must decide whether this separation is appropriate for Mascot.

The seated laptop image should be evaluated as a target pose/style reference such as:

    Pose::CanonicalSeatedLaptop

rather than necessarily being the literal material source for every animated pose.

## 5. Bind-pose question

Do not assume that a front-facing T-pose is ideal.

Mascot has a characteristic right-facing side/3/4 identity.

Research and compare at least:

- strict T-pose;
- relaxed A-pose;
- side/3/4 neutral stance with arms separated from torso;
- another rigging-friendly neutral pose if evidence supports it.

Evaluate each against:

- identity preservation;
- near/far limb readability;
- shoulder/hip topology;
- neck/head articulation;
- tail-root topology;
- paw/foot readability;
- mesh weighting;
- future walking/sitting/typing poses;
- laptop interaction;
- future physical response / active-ragdoll requirements;
- amount of pose-specific art needed.

Recommend the bind-pose family and explain why.

Do not choose merely because one generated image already exists.

## 6. Complete-art requirements for a future neutral master

Research what the future production neutral art should contain.

At minimum evaluate whether it needs:

- complete torso under arms;
- complete shoulder and hip surfaces;
- complete near/far upper and lower limbs;
- complete paws/feet;
- complete neck beneath head;
- complete tail root and body continuation;
- complete ear roots;
- complete muzzle/head surfaces beneath movable facial/ear details;
- independent laptop/prop art rather than laptop-baked body geometry;
- intrinsic markings separated from directional lighting;
- semantic line ownership metadata;
- mesh-ready source topology or high-resolution raster/vector source.

The laptop should be researched as a separate prop/contact object, not body skin.

## 7. Deformation architecture

Research how to classify each region into the simplest valid deformation model.

Candidate classes:

- rigid hinge;
- concealed rigid socket / small-range rigid rotation;
- multi-bone chain;
- weighted local mesh;
- another bounded 2D deformable method if justified.

Pay special attention to:

- shoulder / upper arm;
- hip / upper leg;
- neck / head;
- tail root / torso;
- ear root;
- elbow/knee;
- paw/prop contact.

The research must explicitly answer:

1. which regions should remain rigid;
2. which regions should use local weighted mesh;
3. whether the existing `Attachment::Mesh` + linear-blend skinning model is sufficient;
4. what renderer capability is missing;
5. what authoring representation is needed;
6. how rest-pose identity is verified.

## 8. Line model

Research the three-class line model already proposed by Planner.

### A. External silhouette

Derived at runtime from the final composed/deformed fill silhouette.

This should remain independent of individual attachment baked outlines.

### B. Contact/separation boundaries

Potential runtime-generated or semantically authored boundaries between independently moving touching objects.

Examples:

- chin / laptop;
- paw / laptop;
- foot / laptop;
- other prop/body contacts.

Research:

- part-ID buffer;
- pair-specific boundary policies;
- local edge masks;
- endpoint/taper control;
- draw order;
- anti-aliasing;
- when no line should be generated.

### C. Intrinsic line art

Lines that belong to one deforming surface:

- eyes;
- nose;
- mouth;
- whiskers;
- ear crease;
- intentional form/fold marks;
- laptop logo/details.

These should move/deform with their owning surface.

The research must define how intrinsic line art follows weighted mesh deformation when needed.

## 9. Shading architecture

The current generated neutral reference and the seated reference contain directional darker-orange shading.

This research must explicitly evaluate whether directional shading should remain baked into moving limb textures.

Planner hypothesis:

    base albedo / local markings = authored
    directional form shading = runtime
    external silhouette outline = runtime
    local contact/AO = authored or runtime depending on semantics

The concern is simple:

If a limb rotates while a fixed directional shadow is baked into the limb texture, the shadow rotates with the limb and can end up on the physically/visually wrong side.

Research a lightweight stylized solution appropriate for Mascot, not physically based rendering.

Evaluate at least:

- two-tone/cel/toon shading;
- fixed screen/world-space light direction;
- per-part or per-vertex form cues;
- mesh-deformation-compatible shading;
- local AO/contact darkening;
- whether a tiny normal/orientation representation is needed;
- whether shading can remain deterministic and cheap in the native Windows renderer.

The final recommendation must state exactly what remains baked and what becomes runtime-generated.

## 10. Renderer implications

Inspect the existing Windows renderer and current animation crate.

Known current state to verify:

- renderer already owns D3D11 + Direct2D;
- external outline is currently composition-derived;
- animation code already has `Attachment::Mesh`;
- mesh data includes vertices/UVs/triangles/weights;
- `skin_mesh` already performs linear-blend skinning;
- current Windows renderer skips mesh attachments.

Research the narrowest renderer evolution that would support the recommended architecture.

Evaluate:

- D3D11 textured triangles;
- D2D custom geometry / bitmap mapping if viable;
- another minimal native path already compatible with the current renderer.

Do not implement it yet.

Return:

- ownership boundary;
- minimum data path;
- minimum GPU resources;
- batching implications;
- alpha/composition implications;
- outline-generation compatibility;
- intrinsic-line deformation implications;
- expected complexity/risk.

## 11. Future physical interaction compatibility

Physics is not part of this implementation phase.

However, the future rig architecture should not prevent later behavior such as:

- dragging the mascot;
- gravity after release;
- limb inertia;
- springs/damping;
- contact with window edges/borders;
- legs bending near/against a support;
- active-ragdoll-style blending between animation intent and physical response;
- laptop as a separate contacted/held prop;
- IK/contact constraints.

Research only the architectural consequences.

Do not design a full physics engine.

State whether the proposed:

- bind pose;
- bone hierarchy;
- local meshes;
- constraints;
- prop separation

are compatible with future active-ragdoll/IK behavior.

## 12. Canonical seated-pose reconstruction metric

If a neutral master is adopted later, it must be able to reconstruct the Mascot identity and canonical seated/laptop pose closely.

Do not rely on one opaque similarity score.

Research a future verification set including:

- silhouette IoU or equivalent silhouette comparison;
- landmark alignment:
  - eyes;
  - nose;
  - ears;
  - chin;
  - shoulders;
  - paws;
  - laptop corners;
  - hips;
  - feet;
  - tail;
- palette/color-region comparison;
- intrinsic line-art comparison;
- perceptual image similarity;
- explicit visual review.

A broad “>= 90% similarity” may be useful as a product target only if its measurement is defined. Do not invent a meaningless scalar.

## 13. Existing seated decomposition: salvage vs retire

The research must decide which current work is reusable.

Classify existing artifacts into:

- KEEP — architecture/tooling remains valid;
- ADAPT — reusable with a neutral master;
- RETIRE — tied to seated cutout decomposition and should not guide production;
- RESEARCH ONLY — useful evidence but not runtime authority.

At minimum classify:

- skeleton concepts;
- clip schema 0.3 work;
- `Attachment::Mesh`;
- `skin_mesh`;
- current external-outline pipeline;
- current rigid sprite attachments;
- hidden patch generation;
- junction morphology;
- pivot heuristics;
- detector rules;
- art-readiness tooling;
- current provisional safe ranges;
- current seated part decomposition.

## 14. Required external research

Use primary/official sources where practical.

Research established 2D rigging systems and workflows, including at least:

- Live2D material/PSD preparation;
- Live2D deformers / rotation deformers;
- Spine slots/draw order;
- Spine meshes/weights;
- one additional credible 2D skeletal/deformation system or primary technical source if useful.

Also research relevant native GPU rendering concepts for weighted textured 2D meshes.

For every external source record:

- source;
- specific principle;
- applicability to Mascot;
- what does **not** transfer cleanly;
- resulting recommendation.

Do not cite marketing copy as architectural evidence when technical documentation exists.

## 15. Research deliverables

Create/update:

### Required

    docs/MASCOT_RIG_ART_CONSTRUCTION_RESEARCH_V0.1.md

This is the agent's independent research report.

It must include:

1. executive technical conclusion;
2. source review;
3. seated-source failure analysis;
4. neutral-master recommendation;
5. recommended bind-pose family;
6. part/material separation model;
7. deformation-class map by body region;
8. mesh/weight strategy;
9. line-ownership model;
10. runtime shading model;
11. renderer implication analysis;
12. prop/contact strategy;
13. future physics/IK compatibility;
14. canonical seated-pose reconstruction verification;
15. KEEP/ADAPT/RETIRE classification;
16. tooling gaps;
17. proposed implementation phases;
18. open decisions requiring Planner/user approval.

### Required addendum

Append:

    ## Agent Research Addendum

to:

    docs/MASCOT_RIG_ART_CONSTRUCTION_SPEC_V0.1.md

The addendum must contain the agent's own evidence-based rules that strengthen/specialize the Planner contract.

Do not weaken a Planner MUST/FAIL rule silently.

If research indicates a Planner rule is wrong or materially over-constraining, record the conflict and stop for Planner decision.

### Optional architecture proposal

If useful, add:

    docs/MASCOT_NEUTRAL_ART_ARCHITECTURE_PROPOSAL_V0.1.md

Only if it reduces ambiguity beyond the main research report.

Do not create documents merely to increase output volume.

## 16. Prototype planning — not implementation

The previous research addendum requested two prototypes:

1. dynamic contact-line prototype;
2. continuous-anatomy weighted-mesh prototype.

During this research-only phase, specify these prototypes precisely but do not implement them.

For each prototype define:

- exact input assets;
- exact body/prop region;
- expected output;
- comparison baseline;
- metrics;
- visual evidence;
- failure criteria;
- estimated implementation scope;
- exact question the prototype answers.

Recommended later implementation targets remain:

- chin/laptop or paw/laptop for contact-line behavior;
- hip/upper-leg or shoulder/upper-arm for weighted mesh behavior.

## 17. Stop condition at end of research

After completing the research documents and addendum:

**STOP.**

Do not proceed automatically into:

- art reconstruction;
- renderer implementation;
- mesh implementation;
- pivot repair;
- safe-range changes;
- clip work;
- production tooling changes.

Commit/push the research-only result and report to Planner.

A new explicit Planner instruction will authorize the next implementation phase.
