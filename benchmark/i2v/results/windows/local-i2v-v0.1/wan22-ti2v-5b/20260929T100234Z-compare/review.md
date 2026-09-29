# Visual review — 20260929T100234Z-compare

Profile: `wan22-ti2v-5b` mode `compare` (30 steps, CFG 5.0, int8 transformer) — scored articulation comparison: true

| Check | Finding |
| --- | --- |
| character identity preservation | PASS |
| fixed camera / orientation | PASS |
| torso / global drift | none |
| anatomy regeneration | none |
| limb growth/shrinkage | none (minor far-arm outline wobble) |
| background drift | minor paper-grain flicker; no object intrusions |
| outline/style drift | slight flicker |
| requested local motion occurred | FAIL — near arm static in all 33 frames |
| non-target regions stable | yes |

Reviewer notes: 30-step output is cleaner than the 10/8-step smoke (no stray object) but still does not execute the requested motion. ~1.0 s/step at int8 in steady state; smoke run was slower due to first-run JIT/load overlap.
