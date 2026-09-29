# Local I2V benchmark v0.1 — summary

Measured facts; not a universal model ranking.

| profile | mode | resolution | frames/fps | steps | peak VRAM MB | gen s | wall s | completed | orientation drift | non-target motion | identity | local motion |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| ltxv-2b-0.9.5 | smoke | 480x480 | 33/24 | 10 | 2199 | 0.0 | 4.25 | False | None | None | None | None |
| ltxv-2b-0.9.5 | smoke | 480x480 | 33/24 | 10 | 11808 | 0.0 | 73.61 | False | None | None | None | None |
| ltxv-2b-0.9.5 | smoke | 480x480 | 33/24 | 10 | 11896 | 290.44 | 354.3 | True | none - fixed view held for all frames | near-arm rotation not visibly executed; both arms remain horizontal throughout (motion adherence failed at 10-step smoke) | Identity preserved: orange/cream palette, markings, proportions, thick outline style all match the approved reference across all 33 frames. | prompt asked for near-arm planar rotation up/down/return; output is effectively a static clip - requested local motion did not occur visibly at smoke settings |
| ltxv-2b-0.9.5 | compare | 480x480 | 33/24 | 30 | 11869 | 737.44 | 801.77 | True | None | None | None | None |
| ltxv-2b-0.9.5 | smoke | 480x480 | 33/24 | 10 | 11749 | 3.81 | 70.25 | True | none - fixed view held for all frames | none visible; near-arm rotation not executed - arms remain horizontal throughout | Identity preserved: orange/cream palette, markings, proportions, thick outline style match reference across all 33 frames. | requested near-arm planar rotation did not occur visibly at 10 steps; clip effectively static |
| ltxv-2b-0.9.5 | compare | 480x480 | 33/24 | 30 | 11757 | 7.99 | 74.44 | True | none - fixed view held | none visible; near-arm rotation NOT executed - both arms remain horizontal in every frame | Identity preserved: orange/cream palette, markings, proportions, thick outline match reference across all 33 frames. | requested near-arm planar rotation up/down/return did not occur at 30 steps; output is effectively a static clip despite valid conditioning |
| wan21-vace-1.3b | smoke | 480x480 | 33/16 | 10 | 11982 | 91.02 | 148.66 | False | None | None | None | None |
| wan21-vace-1.3b | smoke | 480x480 | 33/16 | 10 | 11937 | 97.71 | 152.44 | False | n/a - output corrupted | n/a - output corrupted | frames 0 conditioned correctly on input; generated frames are corrupted blocky mosaic artifacts - character unrecognizable | cannot evaluate - generated frames are block-noise mosaic |
| wan22-ti2v-5b | smoke | 480x480 | 33/24 | 8 | 744 | 0.0 | 4.79 | False | None | None | None | None |
| wan22-ti2v-5b | smoke | 480x480 | 33/24 | 8 | 11908 | 0.0 | 174.42 | False | None | None | None | None |
| wan22-ti2v-5b | smoke | 480x480 | 33/24 | 8 | 11852 | 0.0 | 149.45 | False | None | None | None | None |
| wan22-ti2v-5b | smoke | 480x480 | 33/24 | 8 | 11826 | 0.0 | 152.1 | False | None | None | None | None |
| wan22-ti2v-5b | smoke | 480x480 | 33/24 | 8 | 11850 | 0.0 | 58.37 | False | None | None | None | None |
| wan22-ti2v-5b | smoke | 480x480 | 33/24 | 8 | 11872 | 223.23 | 283.55 | True | none - fixed view held | none on-target; near arm does not rotate. From ~frame 20 a small hallucinated sketchy object appears at the right edge near the arm. | Identity preserved: palette, markings, proportions, outline match reference in all frames. | requested near-arm rotation not executed; 10-step clip essentially static on-target |
| wan22-ti2v-5b | compare | 480x480 | 33/24 | 30 | 11982 | 173.76 | 249.41 | True | none - fixed view held | no on-target motion; near arm static throughout | Identity preserved across all frames; palette/markings/proportions match reference. | requested near-arm rotation not executed at 30 steps either |

- `20260929T070733Z-smoke` deviations: fps 24 != benchmark target 16 (model native rate)
- `20260929T070733Z-smoke` FAILED: ValueError: `tiktoken` is required to read a `tiktoken` file. Install it with `pip install tiktoken`.
- `20260929T070834Z-smoke` deviations: fps 24 != benchmark target 16 (model native rate)
- `20260929T070834Z-smoke` FAILED: RuntimeError: mat1 and mat2 must have the same dtype, but got Float and BFloat16
- `20260929T071029Z-smoke` deviations: fps 24 != benchmark target 16 (model native rate)
- `20260929T071748Z-compare` deviations: fps 24 != benchmark target 16 (model native rate)
- `20260929T085620Z-smoke` deviations: fps 24 != benchmark target 16 (model native rate)
- `20260929T085752Z-compare` deviations: fps 24 != benchmark target 16 (model native rate)
- `20260929T085938Z-smoke` deviations: mechanical run succeeded through denoise; failed at frame save (numpy output_type) - superseded by 20260929T090231Z-smoke which showed 480x480 output corruption
- `20260929T085938Z-smoke` FAILED: AttributeError: 'numpy.ndarray' object has no attribute 'save'
- `20260929T090231Z-smoke` deviations: VACE conditioning stream corrupts output at 480x480 square; same setup at native 832x480 was coherent (diagnostic probe) -> unsupported for 1:1 contract
- `20260929T090231Z-smoke` FAILED: quality_verdict=unsupported_480x480 (valid MP4 but corrupted content; not acceptable evidence)
- `20260929T093946Z-smoke` deviations: fps 24 != benchmark target 16 (model native rate)
- `20260929T093946Z-smoke` FAILED: UnboundLocalError: cannot access local variable 'blocks_class' where it is not associated with a value
- `20260929T094226Z-smoke` deviations: fps 24 != benchmark target 16 (model native rate)
- `20260929T094226Z-smoke` FAILED: RuntimeError: Sizes of tensors must match except in dimension 1. Expected size 60 but got size 30 for tensor number 1 in the list.
- `20260929T094744Z-smoke` deviations: fps 24 != benchmark target 16 (model native rate)
- `20260929T094744Z-smoke` FAILED: TypeError: unsupported operand type(s) for *: 'NoneType' and 'int'
- `20260929T095113Z-smoke` deviations: fps 24 != benchmark target 16 (model native rate)
- `20260929T095113Z-smoke` FAILED: RuntimeError: Given groups=1, weight of size [3072, 48, 1, 2, 2], expected input[1, 100, 9, 30, 30] to have 48 channels, but got 100 channels instead
- `20260929T095531Z-smoke` deviations: fps 24 != benchmark target 16 (model native rate)
- `20260929T095531Z-smoke` FAILED: RuntimeError: Given groups=1, weight of size [3072, 48, 1, 2, 2], expected input[1, 100, 9, 30, 30] to have 48 channels, but got 100 channels instead
- `20260929T095702Z-smoke` deviations: fps 24 != benchmark target 16 (model native rate)
- `20260929T100234Z-compare` deviations: fps 24 != benchmark target 16 (model native rate)
