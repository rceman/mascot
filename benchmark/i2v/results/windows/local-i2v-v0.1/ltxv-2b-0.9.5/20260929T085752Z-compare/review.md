# Visual review — 20260929T085752Z-compare

Profile: `ltxv-2b-0.9.5` mode `compare` (30 steps, CFG 3.0) — scored articulation comparison: true (approved neutral T-pose input)

| Check | Finding |
| --- | --- |
| character identity preservation | PASS — palette/markings/proportions/outline match reference in all frames |
| fixed camera / orientation | PASS — identical view all frames |
| torso / global drift | none visible |
| anatomy regeneration | none |
| limb growth/shrinkage | none |
| background drift | none — white background stable |
| outline/style drift | none |
| requested local motion occurred | FAIL — near arm stays horizontal in every frame; clip is effectively static |
| non-target regions stable | yes |

Reviewer notes: at 30 steps CFG 3.0 LTX-0.9.5 is identity-stable but does not execute the requested isolated arm rotation. Prompt was truncated to 256 tokens (prompt_tokens=368) — tail constraints (End Condition list) were dropped by the encoder; noted in receipt.
