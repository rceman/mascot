# Rust candidate

Rust implementation of the shared prototype benchmark. Settled design: `ARCHITECTURE.md`.

This first buildable snapshot is not correctness-ready. See `../benchmark/corrections/rust-001-review.md` before running further tests or measurements.

## Rules

- Match the shared observable behavior and correctness contract exactly.
- Optimize for a realistic production foundation, not a benchmark trick.
- Mature native libraries and platform shims are permitted when justified.
- Do not omit text/input or response-rendering behavior merely to reduce footprint.
- Do not add product features outside the benchmark scope.
- Keep redraw event-driven while idle.
- Use the shared mascot asset, mock provider, fixture manifest, acceptance matrix, text fixtures, and benchmark tooling.
- Record every notable dependency and native/platform workaround.
- Do not write the final language verdict from inside this implementation.
- Required correctness cases must pass before headline performance numbers are considered eligible.

Before coding, create ARCHITECTURE.md and explain:

- windowing
- transparent presentation and hit testing
- how visible presentation timing will be observed
- text/IME ownership and supported behavior
- response text rendering
- first-use versus warm initialization
- child-process lifetime and cooperative cancellation
- hotkeys/focus behavior
- cache and queue bounds
- resource ownership
- application/helper process inventory
- toolchain version
- allocator/runtime-safety settings
- LTO/stripping settings
- expected Windows-specific code
- expected macOS implementation path
- Linux/X11/Wayland feasibility assumptions
- direct dependencies and why each is justified

Do not choose a large GUI framework merely for convenience, but do not reject one merely because it is large. The relevant questions are correctness, measured resource cost, and how much infrastructure the project would own.

## Build

Prerequisites: Rust 1.94.0 (pinned by `rust-toolchain.toml`, MSVC target `x86_64-pc-windows-msvc`), Visual Studio 2022 Build Tools, Windows SDK 10.0.26100.0 (`rc.exe` is invoked by `build.rs` to embed `app.manifest` via `app.rc`).

```powershell
cargo +1.94.0 test --locked
cargo +1.94.0 build --release --locked --target x86_64-pc-windows-msvc
```

Release profile: `opt-level="s"`, thin LTO, one codegen unit, `panic="abort"`, stripped symbols, overflow checks on, static CRT (`+crt-static`).

Deployed binary: `W:\devin_folder\mascot\out\rust\mascot.exe`.

## Run

```powershell
# GUI mode (mascot + hotkeys; no control transport)
mascot.exe --fixture W:\devin_folder\mascot\benchmark\manifest\fixture.json

# Control mode (NDJSON stdin/stdout protocol per harness/control-v1.json)
mascot.exe --fixture W:\devin_folder\mascot\benchmark\manifest\fixture.json --control

# Direct decoder-vector mode (no windows, no provider)
mascot.exe --decode-vectors W:\devin_folder\mascot\benchmark\decoder-vectors\vectors.json
```

## Dependencies

Direct: `windows-sys =0.61.2`, `serde =1.0.228` (derive), `serde_json =1.0.150`, `png =0.18.1`, `base64 =0.22.1`. Locked transitives are in `Cargo.lock`. `windows-sys 0.61.2` ships no `Win32_UI_Controls_RichEdit` feature, so the RichEdit message constants/structs used are declared locally in `src/text.rs` against the installed SDK values; `Win32_UI_Shell` (subclassing), `Win32_System_Ole` (`OleInitialize`) and `Win32_Security_Cryptography` (CNG `BCrypt*` hashing) features were added beyond the initial list.

## Layout

- `src/main.rs` — CLI dispatch, decoder-vector mode, message loop.
- `src/config.rs` — manifest/config loading and validation (asset identity hashed with the Windows CNG SHA-256 provider).
- `src/framing.rs` — exact shared 65536-byte NDJSON framer.
- `src/provider.rs` — provider coordinator/stdout-validator/stderr-tail threads, session lifecycle.
- `src/platform.rs` — Win32 mascot/composer windows, DPI handling, control dispatch.
- `src/text.rs` — `RICHEDIT50W` controls, IMM composition tracking, text readback.
- `src/control.rs` — bounded control stdin reader / stdout writer.
- `src/queue.rs` — bounded queue primitive.

## Known untested areas

Smoke verification (`verify_candidate_smoke.py`) covers decoder vectors, lazy startup, geometry, programmatic F10 text round-trip, hide and shutdown only. Not yet exercised by a passing gate: real provider streaming/cancel scenarios, physical IME composition paths, WM_DPICHANGED transitions across monitors, backpressure/oversized/error sessions, drag and click-through behavior, and sustained-stability resource accounting.
