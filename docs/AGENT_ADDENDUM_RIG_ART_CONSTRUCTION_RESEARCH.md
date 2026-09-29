# Agent Addendum — Rig Art Construction Research Before Repair


> **Current Planner boundary: RESEARCH ONLY**
>
> The broader implementation sequence described below is not authorized yet.
> Read `docs/AGENT_HANDOFF_RIG_RESEARCH_ONLY_OPUS55.md` and
> `docs/MASCOT_NEUTRAL_BIND_POSE_RESEARCH_SPEC_V0.1.md` first.
>
> For the current phase:
>
> - research and documentation are allowed;
> - read-only code/art inspection is allowed;
> - prototype **designs/specifications** are required;
> - production code/art/runtime changes are not allowed;
> - the dynamic-contact-line and weighted-mesh prototypes are to be specified, not implemented;
> - after research + Agent Research Addendum, STOP and report to Planner.
>
> Do not resume the numbered implementation sequence at the end of this file until a new explicit Planner instruction authorizes it.

The current structural/art repair is paused by Planner direction.

Before further structural changes, read:

- `docs/MASCOT_RIG_ART_CONSTRUCTION_SPEC_V0.1.md`
- `docs/MASCOT_AGENT_AUTHORING_VISUAL_QA.md`
- `docs/MASCOT_RIG_RUNTIME_SPEC_V0.2.md`
- `docs/QUALITY_GATES.md`

## Required next action

Do NOT immediately resume tuning junction morphology, global junction regions, hidden-extension constants, pivots, or safe ranges.

First perform the mandatory research pass defined in Section 17 of:

    docs/MASCOT_RIG_ART_CONSTRUCTION_SPEC_V0.1.md

Create:

    docs/MASCOT_RIG_ART_CONSTRUCTION_RESEARCH_V0.1.md

Use primary/official sources where practical.

The research must be your own technical review, not merely a summary of the Planner spec.

Bring in relevant knowledge from established 2D skeletal/mesh rigging systems and art-preparation workflows.

Specifically investigate:

- how cel/toon and 2D skeletal systems distinguish external silhouette, contact/separation lines, and intrinsic internal line art;
- whether runtime-generated internal boundaries from part IDs/masks are appropriate for independently moving touching objects;
- how pair-specific edge masks or line-ownership metadata should prevent unwanted seams;
- how artists prepare parts that will rotate away from one another;
- how much hidden geometry is normally drawn and how to derive it from motion;
- how pivots/rotation centers are selected;
- how shoulders/hips/neck are handled when rigid rotation fails;
- weighted-mesh/skinning topology and weight-authoring strategies for small 2D character joints;
- mesh quality invariants such as foldover/inversion, stretch and UV continuity;
- how shared outlines/strokes are split when touching parts separate;
- when a simple unoutlined pivot underpaint/cap is useful and when it only hides a deeper construction fault;
- how draw order/slots/occlusion are modeled independently from bone hierarchy;
- when weighted mesh/deformation is preferable to rigid sprites;
- how safe ranges are validated;
- how tooling can prove art readiness before animation authoring;
- the narrowest Windows renderer approach for the repository's already-existing `Attachment::Mesh` + linear-blend skinning model, noting that the current renderer skips mesh attachments;
- whether D3D11 textured-triangle interop is preferable to alternatives in this existing renderer architecture.

Also evaluate two explicit prototypes before recommending a final architecture:

1. **dynamic contact-line prototype**
   - chin/laptop;
   - paw/laptop;
   - current baked line vs generated posed boundary;
   - rest fidelity + motion-extreme comparison.

2. **continuous-anatomy mesh prototype**
   - choose one failing anatomical joint, preferably hip/upper-leg or shoulder/upper-arm;
   - small local mesh;
   - deterministic initial weights;
   - compare rigid vs skinned motion at the desired range;
   - measure rest fidelity, foldover/stretch, visual continuity and runtime cost.

Keep prototype artifacts isolated until the research conclusion chooses a direction.

## Required agent contribution

After writing the research note, add a section named:

    ## Agent Research Addendum

to `docs/MASCOT_RIG_ART_CONSTRUCTION_SPEC_V0.1.md`.

Add concrete evidence-based rules that improve the Planner contract.

You are explicitly expected to contribute your own technical conclusions.

Do not merely repeat the existing document.

You may strengthen or specialize it.

If you believe any existing Planner rule should be weakened/removed, STOP and report the conflict instead of silently editing it.

## Preserve current work

Preserve the paused Windows working-copy findings.

Current provisional art, clip values and safe ranges are diagnostic state only.

Do not promote them to final authority before the research/construction pass.

After research + addendum:

1. inventory every moving joint;
2. write explicit joint contracts;
3. classify deformation model per joint;
4. repair semantic art construction;
5. run art-readiness sweeps;
6. derive final safe ranges;
7. only then regenerate clips and animation QA.

Do not add CI.
