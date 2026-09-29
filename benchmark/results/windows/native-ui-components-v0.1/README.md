# Mascot Native UI Component Gallery v0.1 — Windows evidence

Component-gallery result: deterministic sheets for every Tier-A component,
rendered only through the production painter path
(`mascot_ui_win32::components` + `App::render_offscreen`), next to
same-content shadcn reference captures.

HEAD: `03eae71` (full hash in `receipt.json`/`perf.json`), branch
`agent/native-ui-component-gallery-v0.1-swe2`, dirty = false.

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

# gallery (fails on dirty tree; --allow-dirty for dev iterations)
target/release/mascot-ui-lab.exe components --capture <DIR>

# foundation evidence used for the identity check
target/release/mascot-ui-lab.exe capture --out <DIR>
target/release/mascot-ui-lab.exe selftest --out <DIR>
target/release/mascot-ui-lab.exe perf --out <DIR>\perf.json --runs 3

# reference captures (Chrome/Edge over CDP; needs network + chrome.exe)
powershell -ExecutionPolicy Bypass -File tools/ui-reference/capture-shadcn.ps1 -OutDir tools/ui-reference/shadcn
```

## Sheets

All sheets are opaque (cell alpha flattened over the section theme surface).

| File | Contents |
|---|---|
| `component-gallery-light.png` / `-dark.png` | every Tier-A component, all states, 100 % (1 px = 1 DIP) |
| `component-gallery-sizes.png` | stretchable widths 320/380/440 DIP, content-sized variants, fixed sizes, 100 % |
| `component-gallery-dpi.png` | 100/125/150/200 % renders at native pixels on per-theme bands (no resampling) |
| `component-gallery-shadcn-reference*.png` | per-component blocks at 200 %: light and dark theme bands with `label | shadcn reference | native` columns, wrapped deviation bullets |
| `component-inventory.json` | the 10 Tier-A / 10 Tier-B (planned) / 28 Tier-C (deferred) inventory with deviations |
| `shadcn-comparison.md` | per-capture measured table (ref computedStyle vs native tokens/palette) + theme-var table |
| `reference/` | 46 frozen shadcn PNGs + `provenance.json` + `PROVENANCE.md` — developer evidence, never read at runtime |
| `perf.json` | 3-run hardware perf medians + raw runs + stage breakdown |
| `receipt.json` | head `03eae71`, dirty=false, device, os_build, font, sheet dims, file list |
| `logs/` | `cargo-test.txt`, `clippy.txt`, `rustfmt.txt`, `foundation-identity.txt`, `selftest.json`, `perf-ab/` |

## Verification results

- `cargo test --release --workspace`: **68 passed / 0 failed**.
- `cargo clippy --release --workspace --all-targets`: 0 warnings in files
  touched by this branch; remaining warnings are pre-existing in
  `mascot-animation`, `mascot-animation-lab`, `mascot-render-win32`
  (`renderer.rs:350` arity, `image.rs:132` `downscale` loop, `stress.rs`
  `ptr_arg`).
- rustfmt: clean on all changed files except
  `crates/mascot-render-win32/src/image.rs` (pre-existing non-rustfmt style,
  intentionally kept).
- Foundation identity: **100/100 files byte-identical** to the pre-gallery
  baseline (`logs/foundation-identity.txt`).
- Determinism: two `components --capture` runs → 59/60 byte-identical; only
  the second receipt's `dirty` differs (true — the evidence dir exists as
  untracked by then; correct behaviour).
- Selftest: **28/28, first attempt on every check** (`retried: {}`,
  `foreground_guard_blocked_injections: 0`).

## Perf vs foundation baseline (`native-ui-v0.1/perf.json`)

| Metric | foundation (b285fd0) | gallery (03eae71) |
|---|---|---|
| static idle presents / WM_PAINT / CPU | 0 / 1 / 0.010 % | 0 / 1 / 0.010 % |
| focused idle (blink / quiet) | 4 / 0 | 4 / 0 |
| first show | 566.9 ms | 860.8 ms *(see A/B)* |
| warm open p50 / p95 | 3.58 / 4.38 ms | 4.31 / 7.58 ms |
| theme switch p50 / p95 | 0.65 / 2.79 ms | 0.72 / 2.83 ms |
| submit → response p50 / p95 | 5.75 / 6.49 ms | 6.81 / 16.99 ms |
| private memory (post-GPU-trim plateau) | ~88 MB | ~88–90 MB |
| 1000-cycle open/close growth | flat (88.2→87.9 MB) | flat (88.5→88.5 MB) |

This 3-run set landed on a noisy window (per-run first show 1108 / 861 /
753 ms; p95s inflated by the cold first run). The controlled A/B — strict
alternation of `4ab080d` vs this code, 5 runs each ×2 — shows medians within
noise on every metric (`logs/perf-ab/SUMMARY.md`): first show 626.0 vs
606.2 ms, warm open p50 3.78 vs 3.75 ms, submit p50 6.15 vs 6.16 ms. The
steady-state invariants (0 idle presents, 0 quiet presents, flat 1000-cycle
growth) are identical.
