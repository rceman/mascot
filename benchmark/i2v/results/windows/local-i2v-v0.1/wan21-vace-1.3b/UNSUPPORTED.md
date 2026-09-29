# wan21-vace-1.3b — UNSUPPORTED at 480x480 (this benchmark)

Evidence:
- `20260929T090231Z-smoke` produced a valid MP4 but generated frames are corrupted
  block-mosaic noise; conditioning worked on frame 0 only.
- Diagnostic probe (not a benchmark lane): same pipeline/embeds/conditioning at the
  official 832x480 profile produced coherent, correctly-conditioned output.
- Pure T2V at 480x480 (no VACE conditioning) also produced coherent output.
- Conclusion: the VACE conditioning stream does not support the 1:1 480x480 path
  required by the product contract. Model contract is ~480x832 (16:9-class).
- Per Planner directive (1:1-only), this candidate is marked unsupported rather
  than running a second aspect-ratio lane.
