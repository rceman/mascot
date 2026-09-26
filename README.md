# mascot

Experimental native desktop-agent shell benchmark.

This repository exists to answer one concrete engineering question:

> For an ultra-low-memory, ultra-low-latency native desktop agent shell, which implementation gives us the better foundation: Rust or Zig?

The repository intentionally contains two implementations of the same narrow prototype:

- `rust/`
- `zig/`

They must implement the same user-visible behavior and be measured with the same benchmark protocol. This is not an IDE benchmark and not a language shootout in the abstract.

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

**Planning only. No implementation should start until the prototype plan has been reviewed.**

Canonical documents:

- [Prototype plan](docs/PROTOTYPE_PLAN.md)
- [Benchmark protocol](docs/BENCHMARK_PROTOCOL.md)
- [Astra review prompt](docs/ASTRA_REVIEW_PROMPT.md)

## Non-goals for the first prototype

No terminal, diff viewer, code viewer, project tree, browser runtime, Electron, Chromium, WebView, Tauri frontend, cloud sync, RepoSuite, browser automation, or local AI model.

The first decision is narrower: can we build a tiny, correct native shell with a transparent mascot, real text input, global hotkey and streamed agent output while keeping memory and latency extremely low?
