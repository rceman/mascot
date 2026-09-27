# Go prototype

This directory will contain the Go implementation of the shared prototype benchmark.

Implementation has **not started yet**.

## Rules

- Match the shared observable behavior and correctness contract exactly.
- Optimize for a realistic production foundation, not a benchmark trick.
- Mature native/C libraries, cgo, and platform shims are permitted when justified.
- Do not omit text/input behavior merely to reduce footprint.
- Do not add product features outside the benchmark scope.
- Keep redraw event-driven while idle.
- Use the shared mascot asset, mock provider, fixture manifest, acceptance matrix, text fixtures, and benchmark tooling.
- Record every notable dependency and native/platform workaround.
- Do not write the final language verdict from inside this implementation.
- Required correctness cases must pass before headline performance numbers are considered eligible.
- Go runtime/GC costs are part of the application and must not be excluded from OS-level process metrics.

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
- Go runtime/GC strategy
- GOMAXPROCS, GOGC, and GOMEMLIMIT policy
- cgo usage
- native allocation ownership
- application/helper process inventory
- pinned Go/toolchain version
- release/build flags and stripping settings
- expected Windows-specific code
- expected macOS implementation path
- Linux/X11/Wayland feasibility assumptions
- direct dependencies and why each is justified

Do not avoid Go merely because it has a runtime/GC, and do not hide that runtime's cost. The purpose of this candidate is to measure whether Go's simpler concurrency/process/networking model can remain within the same product-level resource and correctness constraints.
