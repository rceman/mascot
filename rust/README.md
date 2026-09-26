# Rust prototype

This directory will contain the Rust implementation of the benchmark defined in `../docs/PROTOTYPE_PLAN.md`.

Implementation has **not started yet**.

## Rules

- Match the required behavior exactly.
- Optimize for a realistic production foundation, not a benchmark trick.
- Avoid large GUI frameworks unless justified in the implementation note.
- Do not add product features outside the benchmark scope.
- Keep redraw event-driven while idle.
- Use the shared benchmark fixtures/assets.
- Record every notable dependency and native/platform workaround.
- Do not write a language verdict from inside this implementation.

Before coding, propose the intended Rust stack in a short `ARCHITECTURE.md` and explain:
- windowing
- transparent presentation
- text/input
- child-process streaming
- hotkeys
- resource ownership
- expected platform-specific code
