# Agent handoff — Rig architecture research-only phase (Opus 5.5)

Task ID: MASCOT-RIG-RESEARCH-003

## Goal

Resume the paused Mascot rig task **only for research and architecture work**.

Do not resume production implementation yet.

The immediate goal is to decide the correct long-term articulated-art architecture before more seated-source repair is performed.

## Repository / workspace

Repository:

    git@github.com:rceman/mascot.git

GitHub:

    rceman/mascot

Use the existing native Windows working copy:

    W:\devin_folder\mascot-rig-v02

Branch:

    agent/windows-rig-animation-lab-v0.2-opus55

Expected Planner baseline before this research-only addendum is the branch state at or after:

    34454374954ee29a1c12149ba75639250515d23e

The Planner may add research documents on top of that baseline. Fetch before starting and use the latest remote HEAD.

Native Windows Git + SSH is authoritative for this task.

Do not use CI.

Do not create a WSL authority copy.

## Important local asset

The user placed this file in the Windows working copy:

    assets/research/neutral-bind-pose/mascot-neutral-tpose-1x1-generated-v0.1.png

Expected size:

    1254x1254

Before research:

1. verify the file exists;
2. verify it is the expected image;
3. verify Git status;
4. after pulling Planner docs, make sure the file was not overwritten;
5. include it in the research commit only under the non-canonical research contract documented by Planner.

This image is NOT approved production art.

## Read first

Read in full:

1. `AGENTS.md`
2. `docs/QUALITY_GATES.md`
3. `docs/AGENT_HANDOFF_WINDOWS_RIG_ANIMATION_OPUS55.md`
4. `docs/MASCOT_RIG_ART_CONSTRUCTION_SPEC_V0.1.md`
5. `docs/AGENT_ADDENDUM_RIG_ART_CONSTRUCTION_RESEARCH.md`
6. `docs/MASCOT_NEUTRAL_BIND_POSE_RESEARCH_SPEC_V0.1.md`
7. `assets/research/neutral-bind-pose/README.md`
8. `docs/MASCOT_RIG_RUNTIME_SPEC_V0.2.md`
9. `docs/MASCOT_AGENT_AUTHORING_VISUAL_QA.md`
10. `docs/MASCOT_AGENT_AUTHORING_FORMAT.md`

Then inspect relevant current code for `Attachment::Mesh`, `skin_mesh`, renderer mesh handling, outline composition, rig data, and current art tooling.

## Hard research-only boundary

You are NOT authorized to continue implementation.

Do not modify:

- production Rust behavior;
- renderer behavior;
- rig.json semantics/data;
- production art;
- pivots;
- hidden geometry;
- safe ranges;
- clips;
- clip timing;
- detector thresholds to make existing art pass;
- junction constants;
- mesh rendering;
- runtime shading;
- physics.

Do not resume structural seated-art repair.

Do not turn research into implementation by adding a "small helper" that materially changes production behavior.

Read-only code inspection is expected.

Documentation changes are expected.

## Main research question

Determine whether Mascot should adopt:

    canonical visual identity/reference pose
        -> purpose-built neutral articulated art master
        -> skeleton + constraints + weights
        -> posed/deformed fills
        -> semantic/contact lines
        -> composition-derived external outline
        -> runtime stylized directional shading
        -> final frame

instead of continuing to treat the seated flattened PNG as the literal source for broad articulation.

Do not assume the answer is yes.

Evaluate alternatives and make an evidence-based recommendation.

## Neutral reference

Use both:

    assets/mascot.png

and:

    assets/research/neutral-bind-pose/mascot-neutral-tpose-1x1-generated-v0.1.png

with different authority:

- `assets/mascot.png` = canonical identity/style/seated-pose reference;
- neutral generated image = non-canonical research/reference material only.

The neutral reference may help identify complete anatomy and rigging concerns, but its proportions, anatomy, shading, and pose are not authoritative.

Explicitly compare it to the canonical mascot and document where it loses identity/detail.

## Mandatory research topics

Investigate and conclude on:

1. neutral articulated master vs seated-source decomposition;
2. T-pose vs A-pose vs right-facing 3/4 neutral bind pose;
3. complete hidden/unoccluded art requirements;
4. laptop as independent prop/contact object;
5. rigid vs weighted deformation per body region;
6. existing `Attachment::Mesh` and `skin_mesh` suitability;
7. minimum renderer path for skinned textured meshes;
8. three-class line ownership:
   - external silhouette;
   - contact/separation lines;
   - intrinsic line art;
9. runtime-generated contact-line feasibility;
10. baked vs runtime stylized directional shading;
11. mesh topology and weight authoring;
12. rest-pose identity preservation;
13. canonical seated/laptop pose reconstruction from neutral art;
14. art-readiness QA before clips;
15. future IK / active-ragdoll / gravity/contact compatibility at architecture level only;
16. which current seated-rig artifacts are KEEP / ADAPT / RETIRE / RESEARCH ONLY;
17. durable tooling needed for the eventual implementation;
18. two later prototype designs:
   - dynamic contact line;
   - local weighted anatomy mesh.

## External research

Do fresh independent research.

Use primary/official sources where practical.

At minimum include:

- Live2D art/material preparation;
- Live2D deformation/rotation concepts;
- Spine slots/draw order;
- Spine meshes/weights;
- another credible relevant source if useful;
- native Windows textured-mesh rendering considerations relevant to the current D3D11/D2D renderer.

Do not merely restate Planner documents.

You are expected to contribute your own technical conclusions.

## Required outputs

Create:

    docs/MASCOT_RIG_ART_CONSTRUCTION_RESEARCH_V0.1.md

Append:

    ## Agent Research Addendum

to:

    docs/MASCOT_RIG_ART_CONSTRUCTION_SPEC_V0.1.md

Optionally create one concise architecture-proposal document only if needed.

Do not generate production evidence that pretends implementation occurred.

## Required research report structure

At minimum:

1. Executive technical conclusion
2. Sources and extracted principles
3. Failure analysis of current seated source
4. Neutral-master decision
5. Bind-pose recommendation
6. Complete-art / material separation contract
7. Body-region deformation map
8. Mesh topology / weights recommendation
9. Line ownership / contact-line recommendation
10. Runtime shading recommendation
11. Renderer implications
12. Laptop/prop/contact architecture
13. Future IK/physics compatibility
14. Canonical seated-pose reconstruction verification
15. KEEP / ADAPT / RETIRE / RESEARCH ONLY table
16. Tooling gaps
17. Later prototype specifications
18. Proposed implementation phases
19. Open Planner/user decisions

## No implementation after research

When research/addendum are complete:

STOP.

Do not continue into code/art changes even if the recommended next implementation seems obvious.

Commit and push research-only changes.

## Git / asset handling

Because the neutral PNG currently exists only in the local working copy:

- pull/fetch Planner documentation first without deleting the local asset;
- inspect status before any reset/clean command;
- never run destructive clean/reset that would remove the user's uncommitted file;
- add the neutral PNG only after verifying the README/contract describes it as non-canonical;
- commit the research asset and research docs together or in clearly related commits;
- push from native Windows Git over SSH.

Do not touch another agent workspace.

## Universal gates

Use the canonical Mascot Universal Gates 1-20.

For this research-only phase, many implementation/runtime gates may be N/A.

N/A must include a changed-cone rationale.

Gate 1 must verify every research question/deliverable is answered.

Gate 2 is especially important: unresolved architecture/product decisions must be surfaced rather than guessed.

Gate 4 must prove no implementation scope leaked into this phase.

Gate 10 must verify the new research-only Planner authority superseded the old "resume implementation after research" wording.

## Final report

Return:

1. starting remote HEAD;
2. local pre-existing asset status;
3. final branch + HEAD;
4. research sources used;
5. main technical conclusion;
6. bind-pose recommendation;
7. deformation map summary;
8. line-model recommendation;
9. shading recommendation;
10. renderer recommendation;
11. seated-source KEEP/ADAPT/RETIRE summary;
12. later prototype definitions;
13. open decisions requiring Planner/user approval;
14. files changed;
15. confirmation that no production implementation was performed;
16. Gates 1-20 PASS/FAIL/N/A;
17. Windows Git clean/push confirmation.

End with exactly:

    MASCOT_RIG_RESEARCH_003_COMPLETE

or:

    MASCOT_RIG_RESEARCH_003_BLOCKED: <reason>
