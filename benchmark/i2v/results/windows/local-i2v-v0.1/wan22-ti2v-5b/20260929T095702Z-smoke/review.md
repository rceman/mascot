# Visual review — 20260929T095702Z-smoke

Profile: `wan22-ti2v-5b` mode `smoke` (10 steps, CFG 5.0, transformer int8) — scored articulation comparison: true (approved neutral T-pose input)

| Check | Finding |
| --- | --- |
| character identity preservation | PASS — palette/markings/proportions match reference |
| fixed camera / orientation | PASS — fixed view held |
| torso / global drift | none visible |
| anatomy regeneration | none |
| limb growth/shrinkage | none |
| background drift | MINOR — faint speckle texture develops; a small hallucinated sketchy object appears at the right edge from ~frame 20 |
| outline/style drift | slight — sketchy texture increase in late frames |
| requested local motion occurred | FAIL — near arm does not rotate; clip essentially static on-target |
| non-target regions stable | mostly — stray object intrusion late frames |

Reviewer notes: Wan2.2-TI2V-5B ran a valid 480x480 path (WanImageToVideoPipeline, expand_timesteps conditioning, int8 transformer+int8 UMT5). Identity stable but motion weak/absent and a stray hallucinated object intrudes late in the clip. Generation ~22 s/step at int8 — slower per-step than LTX bf16 (~0.8 s/step).
