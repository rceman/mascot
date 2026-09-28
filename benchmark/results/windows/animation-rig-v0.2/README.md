# Rig v0.2 — native Windows evidence

Machine: Windows 11, NVIDIA GeForce RTX 4070, Rust 1.94.0 (MSVC). All builds, tests, renders and
measurements ran natively on Windows. Stress renders use the WARP software device (deterministic);
lab screenshots and perf use the hardware device. Architecture: `docs/MASCOT_RIG_V0.2_ARCHITECTURE.md`.

Regenerate: `cargo build --release --workspace && target/release/mascot-rig-stress.exe --out benchmark/results/windows/animation-rig-v0.2`
(then `mascot-animation-lab.exe --capture .../lab` and `--perf .../perf_hardware.json`).

## Contents

| Path | What |
|---|---|
| `contact_sheet.png` | all 42 stress poses |
| `poses/*.png` | individual stress renders (0.5× view, runtime outline + contact shadow) |
| `rest/` | canonical, runtime rest, side-by-side comparison, alpha diff ×3, RGB diff heat ×2 |
| `pivots.png`, `pivots_mirrored_posed.png` | bone hierarchy + pivot markers at rest and mirrored/posed |
| `joint_guards/*.png` | per-guard sweep strips (guard circle drawn over each sampled angle) |
| `stress_report.json`, `stress_stdout.txt` | machine-readable results |
| `lab/*.png` | animation-lab screenshots (rest, bone overlay, bounds, mirrored+shadow mid-clip, after clip) |
| `perf_hardware.json` | resource / cadence measurement of the lab |

## Stress suite (spec §14)

Rest; head −10/−5/+5/+10; each ear −12/−6/+6/+12; each arm and each leg −15/−10/+10/+15; tail
−20/−10/+10/+20; mixed greeting, posture, curious, stretch, extremes A/B (every major bone at a range
end simultaneously); mirrored rest, mirrored greeting, mirrored extremes A.

Automated checks (supplementary to visual review):

- **Enclosed transparent gaps** (background pixels fully enclosed by the silhouette): rest 0 px; max over
  all 42 poses 1 px (`mirror_greeting`, a single anti-aliasing pixel).
- **Joint guards**, 9 samples across each range, required coverage ≥ 0.995: `neck_seam`, `ear_near_root`,
  `ear_far_root`, `shoulder_near`, `shoulder_far`, `hip_near`, `hip_far`, `tail_root` all **1.0000 PASS**.
- **Rest comparison** vs `assets/mascot.png` (1254² canvas): mean |ΔRGB| over white 1.08 / 255;
  pixels with ΔRGB > 96: 1 753 (0.11 %); alpha Δ > 64: 1 548 (0.10 %). The differences lie along the outer
  contour: the runtime outline is a uniform 34.5 px dilation while the canonical stroke varies
  (p10 34 px, p90 46 px). No interior redraw or style drift.

## Visual review

Every pose was inspected at evidence scale and the problem joints at 1–3× with per-part tinting
(`scratch/` helpers, not committed). Defects found during iteration and fixed in the decomposition:

- ear root: ear moved to its root pivot; the ear/back ownership cut no longer emits a hidden contour
  (was a black wedge on rotation); body continues beneath the ear root.
- chin/laptop lid: the merged chin+lid stroke is split so each part carries a normal-width contour of
  its own edge (was a black blade at head ±10); the chest continues behind the chin so a lifted chin
  shows chest instead of background.
- neck, shoulder, hip cut edges feathered; never near the silhouette (was a soft black smudge).
- kept outline protrusions (tail tip, head-top step) overlap the runtime outline (was a hairline seam).
- far thigh hidden capsule narrowed (a fragment peeked under the lifted near foot).
- tried and rejected: a runtime "crack bleed" pass. Measured with and without, it removed no real
  seam and added orange hairlines along the silhouette.

Result at the declared safe ranges: no obvious seams, transparent holes, detached parts, doubled external
outlines or black wedges. Remaining, visible on close inspection:

- head ±10: the chin visibly lifts off / overlaps the laptop lid. The contact changes; the contours stay
  clean.
- leg ±15: the laptop base underside and the far thigh behind the near thigh become visible. These are
  plausible hidden surfaces with clean contours. Their shapes are reconstructions.
- dark-orange side shading is carried by the owning part, so a rotated part shows a small shading step
  against a static neighbour (e.g. ear root, head ±10 against the back).
- `ear_far` / `arm_far` are reconstructions, occluded at rest.

## Performance (`perf_hardware.json`, lab window 992×853 client, hardware device)

| Window | Duration | Frames rendered | CPU (% of one core) |
|---|---|---|---|
| idle, mascot visible, static | 10.0 s | **0** | 0.47 % |
| active clips (continuous clip playback) | 11.3 s | 1 064 (94.5 fps, vsync-paced) | 0.55 % |
| after animation completes | 10.0 s | **0** | 0.62 % |

Frame cadence while active: p50 10.42 ms, p95 10.60 ms, p99 10.69 ms (display refresh), max 75 ms (first
frame after wake). Memory: working set 51–55 MiB, private 97–102 MiB (mostly D3D/D2D driver
allocations). Threads 35–38 (driver/DWM worker threads; the app itself is single-threaded). Handles
453–467, GDI objects 13, USER objects 41–44, all stable across windows.

Static → no render/update loop: the canvas renders only on `WM_PAINT`. Paints are requested by UI input
or by an animation frame, and stop when the player becomes static. The idle director arms one one-shot
timer. CPU figures are GetProcessTimes deltas, whose granularity is 15.6 ms.

## Tests

`cargo test --release --workspace`: 23 tests, all passing. 1 is a renderer premultiply round-trip test. 18 are runtime unit tests: math, topological
skeleton, mirror about the root pivot, parent/child pivots, rigid sprite placement, mesh skinning at
bind pose and under bone motion, clip easing/interpolation, player completion/reset, idle-director
determinism, draw-list z order/opacity, rig validation. 4 are integration tests on the shipped
`rig.json`/`clips.json`: required hierarchy, sprite sizes, all required clips bind and stay in safe
ranges, and every clip completes and returns exactly to rest.
