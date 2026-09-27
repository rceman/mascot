# Zig prototype

This directory will contain the Zig implementation of the shared prototype benchmark.

Implementation has **not started yet**.

## Rules

- Match the shared observable behavior and correctness contract exactly.
- Optimize for a realistic production foundation, not a benchmark trick.
- Mature native/C libraries and platform shims are permitted when justified.
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
- allocator/resource ownership
- application/helper process inventory
- pinned Zig/toolchain version
- runtime-safety settings
- LTO/stripping settings
- expected Windows-specific code
- expected macOS implementation path
- Linux/X11/Wayland feasibility assumptions
- direct dependencies and why each is justified

Do not force an all-Zig stack merely for language purity. The relevant questions are correctness, measured resource cost, and how much infrastructure the project would own.
