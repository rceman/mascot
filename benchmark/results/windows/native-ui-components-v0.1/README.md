# Mascot Native UI Component Gallery v0.1 — Windows evidence

Component-gallery result: deterministic sheets for every Tier-A component,
rendered only through the production painter path
(`mascot_ui_win32::components` + `App::render_offscreen`), next to
same-content shadcn reference captures, plus shadcn-reference motion strips.

## Identity

| Field | Value | Where |
|---|---|---|
| `code_head` | `241c487` — last code-changing commit; the gallery, foundation captures, selftest and logs were generated from it (`receipt.json`, `../native-ui-v0.1/receipt.json`). `perf.json` was measured at `2dcb2ea`; the only code change since is `apps/mascot-ui-lab/src/stamp_evidence.rs`, which is not on the perf path, so the full benchmark was not rerun | `receipt.json`, `perf.json`, `../native-ui-v0.1/receipt.json` |
| `code_dirty` | `false` (changes under `benchmark/results/` are excluded by design) | same |
| `evidence_head` | the commit that added this evidence; written by `mascot-ui-lab stamp-evidence` in the following commit, which is the final branch head and changes only `evidence_head` fields | same (`evidence_head_rule`) |

`receipt.json` lists every produced file with its git blob hash.

## Environment

| | |
|---|---|
| OS | Windows 11, build 10.0.26200.9457 |
| CPU | Intel Core i9-9900K @ 3.60 GHz |
| GPU | NVIDIA GeForce RTX 4070 |
| Font | Segoe UI Variable Text |
| Rust | rustc 1.94.0 (4a4ef493e 2026-03-02) |
| Devices | gallery + selftest render on WARP (deterministic pixels); perf on hardware D3D11 |
| Reference browser | chrome.exe 154.0.8037.58 (headless CDP, DPR 2); upstream shadcn-ui/ui commit db2db460 |

## Regenerate

```powershell
cargo build --release --workspace

# gallery (fails on a dirty tree; --allow-dirty for dev iterations)
target/release/mascot-ui-lab.exe components --capture <DIR>

# foundation evidence (../native-ui-v0.1)
target/release/mascot-ui-lab.exe capture --out <DIR>
target/release/mascot-ui-lab.exe selftest --out <DIR>

# canonical perf: strictly alternating A/B vs the foundation build
target/release/mascot-ui-lab.exe perf-ab --baseline-exe <4ab080d build>\mascot-ui-lab.exe `
  --baseline-head 4ab080d818d41b855b77590ff65a900c1ec3dc65 --out <THIS DIR> --rounds 2 --runs 5

# pixel comparison of two evidence trees
target/release/mascot-ui-lab.exe diff-images <DIR_A> <DIR_B>

# after committing evidence: stamp evidence_head (fails on a dirty tree or code changes)
target/release/mascot-ui-lab.exe stamp-evidence <DIR> [<DIR>...]

# reference captures (Chrome/Edge over CDP; needs network + chrome.exe)
powershell -ExecutionPolicy Bypass -File tools/ui-reference/capture-shadcn.ps1 -OutDir tools/ui-reference/shadcn
powershell -ExecutionPolicy Bypass -File tools/ui-reference/capture-shadcn.ps1 -OutDir tools/ui-reference/shadcn -Motion
```

## Artifacts

All sheets are opaque (cell alpha flattened over the section theme surface).

| File | Contents |
|---|---|
| `component-gallery-light.png` / `-dark.png` | every Tier-A component, all states, 100 % (1 px = 1 DIP) |
| `component-gallery-sizes.png` | stretchable widths 320/380/440 DIP, content-sized variants, fixed sizes, 100 % |
| `component-gallery-dpi.png` | 100/125/150/200 % renders at native pixels on per-theme bands |
| `component-gallery-shadcn-reference*.png` | per-component blocks at 200 %: `label | shadcn reference | native` in light and dark bands, with deviation bullets |
| `component-gallery-motion.png` | motion rows 1–4 (Button hover, IconButton primary/ghost hover, focus ring): shadcn strip above native strip, t = 0…150 ms in 25 ms steps, 200 %, light and dark |
| `component-gallery-motion-2.png` | motion rows 5–8 (Tooltip open, Tooltip close, reduced-motion hover, reduced-motion tooltip) |
| `motion.json` | durations, easings, sampled colours/ring/tooltip opacity-scale-offset per frame, placed rects per strip and frame, shadcn computed styles |
| `component-inventory.json` | 10 Tier-A / 10 Tier-B (planned) / 28 Tier-C (deferred), per-entry `motion`, top-level `loading` = deferred |
| `shadcn-comparison.md` | measured shadcn computed styles vs native tokens + theme-var table |
| `reference/` | frozen shadcn PNGs (46 static + 70 motion) with `provenance.json`, `motion-provenance.json`, `PROVENANCE.md`; developer evidence, never read at runtime |
| `perf.json` | **canonical** perf: alternating A/B (`kind: alternating-ab`) |
| `receipt.json` | code identity, device, sheet dims, file list with blob hashes |
| `logs/perf-ab/` | the four raw perf JSONs (A1, B1, A2, B2; 5 runs each) behind `perf.json` |
| `logs/` | `cargo-test.txt`, `clippy.txt`, `rustfmt.txt`, `selftest.json`, `determinism.txt`, `gallery-diff.txt` |
| `logs/history/` | superseded raw measurements kept as evidence: `perf-03eae71.json` (the noisy 3-run set, first show 753–1108 ms), `perf-ab-03eae71/`, `foundation-identity-03eae71.txt` |

## Verification results (code_head 241c487)

- `cargo test --release --workspace`: 17 suites, 0 failed (`logs/cargo-test.txt`).
- `cargo clippy --release --workspace --all-targets`: 0 warnings in code added by this branch. All
  remaining warnings are in pre-existing lines of `mascot-animation`, `mascot-animation-lab` and
  `mascot-render-win32` (including `image.rs:132`, outside the branch's `blend_over` addition).
- rustfmt: clean on every changed file except `crates/mascot-render-win32/src/image.rs`, which keeps its
  pre-existing hand formatting. 6 of its 7 rustfmt hunks are in pre-existing lines; 1 (line 181) is in
  the `blend_over` helper added by `c39f45c` and follows the file's existing style.
- Determinism: two `components --capture` runs, 127/127 files byte-identical (`logs/determinism.txt`).
- Gallery vs previous evidence (`logs/gallery-diff.txt`): 49/55 identical; the light/dark/sizes/dpi sheets
  change only in the tooltip bands (tooltip colour correction), reference sheets 1–2 grew by the new
  deviation rows; new: the two motion sheets and the 70 motion reference frames.
- Selftest: 33/33, first attempt on every check (`retried: {}`, 0 blocked injections), including
  `cursor-shape`, `send-click-submits`, `motion-hover`, `motion-tooltip`, `motion-reduced`.

## Perf — `perf.json` (canonical)

Strict alternation A1, B1, A2, B2; 5 runs per set, 10 per side. A = foundation `4ab080d`, B = `2dcb2ea`.
Hardware D3D11, release, cursor parked away from the window.

| Metric | A median | B median | Δ |
|---|---|---|---|
| first show | 584.0 ms | 592.8 ms | +8.8 ms (+1.5 %; A range 567–624, B 559–629) |
| warm open p50 | 3.69 ms | 3.74 ms | +1.4 % |
| submit → response p50 | 5.97 ms | 6.23 ms | +0.26 ms |
| theme switch p50 | 0.70 ms | 0.71 ms | +1.6 % |
| static idle CPU | 0.015 % | 0.024 % | +0.01 pp |
| private memory (1000-cycle plateau) | 89.7 MB | 89.2 MB | −0.5 % |

Zero-idle holds in every one of the 20 runs: static idle 0 presents, focused-quiet 0 presents, 4
caret-blink presents then quiet, 1 WM_PAINT. `growth_plateau_all = true`. The motion timer is armed only
while a transition runs (selftest `motion-*` checks: 0 motion presents in the 1 s after settling).
