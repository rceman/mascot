# Mascot Native UI Foundation v0.1 — Windows evidence

Phase A / M1A result: a compact Windows-native Mascot composer lab —
`WS_POPUP` + `WS_EX_NOREDIRECTIONBITMAP` window with a DirectComposition
premultiplied-alpha flip swap chain (no `WS_EX_LAYERED`), windowless
RichEdit text path, Direct2D painter, Lucide icon set, light/dark
shadcn-style theme, left/right placement.

HEAD: `b285fd0` (`b285fd0` full hash in `receipt.json`/`perf.json`), branch
`agent/native-ui-foundation-v0.1-swe2`, dirty = false.

## Environment

| | |
|---|---|
| OS | Windows 11, build 10.0.26200.9457 |
| CPU | Intel Core i9-9900K @ 3.60 GHz |
| GPU | NVIDIA GeForce RTX 4070 |
| Displays | primary 3840×2160 + virtual 1024×768; DPI awareness Per-Monitor-V2 |
| App scale | 1.0 (lab passes an explicit scale; DPI sheet renders 100/125/150/200%) |
| Font | Segoe UI Variable Text |
| Rust | rustc 1.94.0 (4a4ef493e 2026-03-02) |
| Devices | capture/selftest render on WARP (deterministic pixels); perf and interactive use hardware D3D11 |

## Build and run

```powershell
cargo build --release --workspace
```

All lab commands are subcommands of `mascot-ui-lab` (release binary).

### Interactive playground

```powershell
target/release/mascot-ui-lab.exe interactive [--theme dark] [--placement left|right] [--state <preset>] [--scale 1.0|1.25|1.5|2.0]
```

Opens the real floating composer window anchored near a screen corner.
Hotkeys while focused: **F1** toggle theme, **F2** toggle placement,
**F3/F4** cycle state presets, **F5** cycle scale.
Preset names are listed in `apps/mascot-ui-lab/src/presets.rs`
(`composer-empty`, `composer-text`, `response`, `send-focus-visible`, …).
Hovering Send/Stop/Copy shows the control's tooltip after a 500 ms
(`tokens::TOOLTIP_DELAY_MS`) hover delay; moving the cursor out of the
window clears both the hover accent and the tooltip via `WM_MOUSELEAVE`.

### capture

```powershell
target/release/mascot-ui-lab.exe capture --out <dir> [--allow-dirty]
```

Renders all state/theme/scale cells offscreen on WARP and writes
`states/*.png`, `zoomed/*`, contact sheets and `receipt.json`
(receipt records HEAD + dirty; `--allow-dirty` is needed only on a dirty
worktree — this evidence was produced without it).
Every DPI cell is self-validated: the editor's measured line height must
be ~19 DIP at every scale and text/caret geometry must stay inside the
editor rect; the command fails on violation.

### selftest

```powershell
target/release/mascot-ui-lab.exe selftest --out <dir>
```

End-to-end input/IME/clipboard run against a real window using SendInput.
Prerequisites:

- run it in an interactive desktop session — the harness must be able to
  put its window in the foreground (injections are guarded; blocked
  injections are counted in `selftest.json`),
- keyboard layouts: **Latvian Standard** (`00020426`, dead-key `'`
  + `a` → `ā` check) and a **Japanese IME profile**
  (`{03B5835F-F03C-411B-9CE2-AA23E1171E36}`/`A76C93D9…`, MS-IME — a stock
  Japanese IME install satisfies it).

Writes `selftest.json` (per-check pass + injection retry counts) and
checkpoint PNGs.

This run: **28 checks, all pass, first attempt on every check**
(`retried: {}`). `enter-submits` injects a plain Enter against a
non-empty composer and observes `activity == Submitting`;
`enter-submit-latency` is a dedicated regression assertion that fails if
`press_to_submit_ms >= 250` — measured **51 ms** this run.

The Enter path used to flake under the old harness: `pump_until` read
`app.state` through a `&mut App` that LLVM treated as `noalias` while the
wndproc mutated the same object through `APP_PTR`, so the poll could read
a hoisted stale value. The harness now holds only `*mut App` and polls
through `*const App` (every read is a real memory access), the deadline is
a fixed 500 ms, and no retry path exists.

### perf

```powershell
target/release/mascot-ui-lab.exe perf --out <file.json> --runs 3
```

Real visible DComp window on the hardware device; per-run raw values plus
medians, stage-by-stage private bytes/threads/handles/GDI/USER, 10 s
static and post-caret-timeout idle windows, and a 1000-cycle open/close
growth probe.

### mascot-icons-gen

```powershell
cargo run -p mascot-icons --bin mascot-icons-gen
```

Regenerates `crates/mascot-icons/src/generated.rs` from
`crates/mascot-icons/source/*.svg` (upstream Lucide geometry).
`mod generated;` is `#[rustfmt::skip]` — the generator owns the layout and
the `generated_is_in_sync_with_sources` test guards drift.

## Verification (this run)

`cargo test --release --workspace`: **61 passed, 0 failed**
(`logs/cargo-test.txt`). `cargo clippy --release` on the four new
packages: no warnings in them (7 pre-existing warnings in the untouched
`mascot-animation`/`mascot-render-win32` siblings; `logs/clippy.txt`).
`rustfmt --edition 2024 --check` on all tracked `.rs` in the new packages
except `generated.rs` (`#[rustfmt::skip]`): clean (`logs/rustfmt.txt`).

## Artifact index

| Path | Contents |
|---|---|
| `contact-sheet-light.png` / `-dark.png` | all 18 states × theme, 1:1 cells |
| `dpi-contact-sheet.png` | 3 states × 2 themes × scales 100–200% |
| `icon-sheet.png` | the 12-icon Lucide set in both themes |
| `states/` | 36 per-state PNGs |
| `zoom/` | 12 curated 3× crops + 2× DPI cells |
| `receipt.json` | capture receipt: HEAD, dirty, device, file list |
| `selftest/` | `selftest.json` + 3 checkpoint PNGs + `selftest-screen.png` |
| `perf.json` | 3 hardware runs: medians, per-run metrics, stage snapshots |
| `uia.txt` | UIAutomationClient walk: window → Document → Value/Text |
| `logs/` | `cargo-test.txt`, `clippy.txt`, `rustfmt.txt` |

## Perf (hardware D3D11, release, 3 runs)

### Headline (medians across runs)

| Metric | p50 | p95 |
|---|---|---|
| Process create → first visible frame | 566.9 ms | — (per-run: 555.9 / 566.9 / 603.9; range 555.9–603.9) |
| Theme switch | 0.65 ms | 2.79 ms |
| Warm open | 3.58 ms | 4.38 ms |
| Submit → response | 5.75 ms | 6.49 ms |
| Rig load | 0.46 ms | — |

### Idle (10 s windows, per run)

| Run | static presents | WM_PAINT | static CPU | blink presents | quiet presents |
|---|---|---|---|---|---|
| 0 | 0 | 1 | 0.03 % | 4 (caret-blink) | 0 |
| 1 | 0 | 1 | 0.01 % | 4 | 0 |
| 2 | 0 | 1 | 0.01 % | 4 | 0 |

Event-driven: zero presents while idle; the only idle presents are the
caret-blink toggles until the caret timeout.

### First-show stage breakdown (last run)

| Stage | private MB | threads | handles | cum. ms |
|---|---|---|---|---|
| renderer (D3D11+D2D+DComp) | 53.6 | 18 | 344 | 262 |
| + rig load | 81.5 | 18 | 344 | 366 |
| + fonts/painter | 81.5 | 18 | 344 | 424 |
| + RichEdit host | 81.7 | 18 | 367 | 488 |
| + sprite render | 141.2 | 34 | 417 | 569 |
| + first present | 147.0 | 36 | 445 | 635 |
| + `IDXGIDevice3::Trim` + `ID2D1Device::ClearResources(0)` | **84.4** | 36 | 445 | 703 |

The sprite-stage jump is the first real rig raster: the D2D effect graph
compiles shaders and the driver allocates its internal heaps (+16
driver-owned threads; the app creates none). Trim + ClearResources
reclaim ~63 MB of it; the retained app-side delta is ~3 MB (sprite
bitmap + swapchain buffers). Steady state is ~84–89 MB.

### Growth (1000 open/close cycles)

private bytes 89.3 → 88.3 MB (flat), handles 453 → 454, threads 37 → 37 —
plateau; no per-cycle growth.

## Baseline comparison (committed numbers)

| | this lab | `animation-rig-v0.2` D2D lab | Rust GDI benchmark (`RESULTS.json`) |
|---|---|---|---|
| idle private | ~84–89 MiB | ~97–102 MiB | r0 private WS ~7.3 MB |
| idle threads | 36–37 | ~36 | — |
| idle handles | ~445–454 | ~455 | — |
| first show | 555.9–603.9 ms, median 566.9 (cold, incl. device+rig+fonts+sprite) | — | fresh startup median ~94 ms |
| warm open | ~3.6 ms p50 | — | warm activation ~156 ms |

Differences in what is being measured: the GDI benchmark's ~94 ms is a
minimal-window startup, not a D3D/DComp/RichEdit path; the rig lab's idle
footprint counts the same D3D11 driver baseline (~50 MiB renderer stage +
driver heaps). This lab adds the RichEdit host (+~0.2 MB private, +11 GDI
objects, +8 USER objects at that stage) and the sprite-stage transient on
top of it.
