# Visual review — 20260929T085620Z-smoke

Profile: `ltxv-2b-0.9.5` mode `smoke` (10 steps, CFG 3.0) — scored articulation comparison: true (approved neutral T-pose input)

| Check | Finding |
| --- | --- |
| character identity preservation | PASS — palette/markings/proportions/outline match reference in all frames |
| fixed camera / orientation | PASS — identical view all frames |
| torso / global drift | none visible |
| anatomy regeneration | none |
| limb growth/shrinkage | none |
| background drift | none — white background stable |
| outline/style drift | none |
| requested local motion occurred | FAIL — near arm stays horizontal; clip effectively static |
| non-target regions stable | yes |

Reviewer notes: LTX-0.9.5 at 10 steps is identity-stable but does not execute the requested isolated arm rotation. Prompt token count 368 > encoder cap 256 — tail constraints truncated (see receipt `prompt_truncated_256tok`).
