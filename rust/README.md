# Rust prototype

This directory will contain the Rust implementation of the shared prototype benchmark.

Implementation has **not started yet**.

## Rules

- Match the shared observable behavior and correctness contract exactly.
- Optimize for a realistic production foundation, not a benchmark trick.
- Mature native libraries and platform shims are permitted.
- Do not add product features outside the benchmark scope.
- Keep redraw event-driven while idle.
- Use the shared mascot asset, mock provider, fixture manifest, acceptance matrix, and benchmark tooling.
- Record every notable dependency and native/platform workaround.
- Do not write the final language verdict from inside this implementation.
- Required correctness cases must pass before headline performance numbers are considered eligible.

Before coding, create `ARCHITECTURE.md` and explain:

- windowing
- transparent presentation and hit testing
- how visible presentation timing will be observed
- text/IME ownership and supported behavior
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
