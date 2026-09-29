# Agent Addendum — Rig Art Construction Research Before Repair

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

- how artists prepare parts that will rotate away from one another;
- how much hidden geometry is normally drawn and how to derive it from motion;
- how pivots/rotation centers are selected;
- how shoulders/hips/neck are handled when rigid rotation fails;
- how shared outlines/strokes are split when touching parts separate;
- how draw order/slots/occlusion are modeled independently from bone hierarchy;
- when weighted mesh/deformation is preferable to rigid sprites;
- how safe ranges are validated;
- how tooling can prove art readiness before animation authoring.

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
