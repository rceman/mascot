# Visual review — 20260929T071029Z-smoke

Profile: `ltxv-2b-0.9.5` mode `smoke` (10 steps, CFG 3.0) — scored articulation comparison: true (approved neutral T-pose input)

| Check | Finding |
| --- | --- |
| character identity preservation | PASS — palette/markings/proportions/outline match reference across all frames |
| fixed camera / orientation | PASS — identical view all frames |
| torso / global drift | none visible |
| anatomy regeneration | none |
| limb growth/shrinkage | none |
| background drift | none — white background stable |
| outline/style drift | none |
| requested local motion occurred | FAIL — near arm does not visibly rotate; both arms stay horizontal; clip is effectively static |
| non-target regions stable | yes |

Reviewer notes: 10-step smoke at CFG 3.0 produces an identity-stable but static clip. Whether 30-step compare unlocks the arm motion is the open question — recorded in the compare run.
