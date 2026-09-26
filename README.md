# mascot

Experimental native desktop-agent shell benchmark.

This planning repository will contain two independently implemented candidates for the same narrow benchmark:

- `rust/`
- `zig/`

The goal is to answer one concrete engineering question:

> For an ultra-low-memory, ultra-low-latency native desktop agent shell, which realistic implementation stack gives us the better foundation: Rust or Zig?

This is not an IDE benchmark and not a theoretical language shootout.

## Product direction

The eventual product may become a tiny always-available desktop agent with:

- floating mascot / bubble
- text and push-to-talk interaction
- screenshot / window / clipboard / file context
- drag-and-drop context
- Codex app-server and Devin ACP adapters
- browser/computer-use integration later
- cloud/session sync later
- RepoSuite integration much later

The benchmark deliberately excludes most of that.

## Current stage

**Planning only. No implementation should start until the revised experiment design passes review.**

Canonical documents:

- [Prototype plan](docs/PROTOTYPE_PLAN.md)
- [Benchmark protocol](docs/BENCHMARK_PROTOCOL.md)
- [Shared correctness matrix](docs/ACCEPTANCE_MATRIX.md)
- [Mock provider contract](docs/MOCK_PROVIDER_CONTRACT.md)
- [Astra review prompt](docs/ASTRA_REVIEW_PROMPT.md)

## Decision gates

Windows is the initial screening and performance-comparison platform.

Windows alone cannot establish the final cross-platform choice.

Every candidate eligible for final selection must also pass:

- the critical macOS validation slice
- a documented Linux/X11/Wayland feasibility review
- a minimal untimed Codex app-server compatibility check

The benchmark does not predict the complete future product's eventual resource usage.

## Non-goals for the first prototype

No terminal, diff viewer, code viewer, project tree, browser runtime, Electron, Chromium, WebView, Tauri frontend, cloud sync, RepoSuite, browser automation, screenshots, voice, or local AI model.

The first decision is narrower: can we build a tiny, correct native shell with a transparent mascot, real text/IME input, global hotkey and structured streamed agent-style I/O while keeping memory and latency extremely low?
