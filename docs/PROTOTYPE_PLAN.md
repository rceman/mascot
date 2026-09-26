# Rust vs Zig Prototype Plan v0.1

Status: **proposal for review before implementation**

## 1. Decision we are trying to make

Choose the primary implementation language and native UI approach for an eventual ultra-light desktop AI agent shell.

The comparison is between **Rust** and **Zig** implementations of the same narrow prototype.

The winner is not the language with the smallest hello-world binary. The useful question is:

> Which stack gives us the best combination of low steady-state memory, low latency, native desktop integration, correctness, implementation simplicity, and maintainability for the actual product we intend to build?

The prototype must therefore exercise the parts most likely to invalidate a stack choice early.

## 2. Product principles

The eventual product is intended to provide a small always-available interface to external agents such as Codex app-server and Devin ACP without carrying a browser runtime.

Hard principles:

- no Electron
- no Chromium embedded in the app
- no WebView-based primary UI
- no Tauri frontend
- no Node.js runtime
- no permanent 60/120 Hz redraw loop while idle
- UI state must remain bounded
- long histories and large attachments must not be retained in RAM by the shell
- external agent processes are measured separately from the shell itself
- platform-specific native shims are acceptable
- correctness of text input and OS behavior matters more than framework purity

## 3. Prototype scope

Both implementations MUST provide the same visible behavior.

### P0. Floating mascot

A small transparent borderless mascot window:

- per-pixel transparency
- no visible rectangular background
- always available above ordinary application windows where the OS supports it
- draggable
- correct hit testing: transparent pixels should not unnecessarily block the desktop
- no full-screen transparent backing window
- no continuous redraw when stationary
- support at least 1x and 2x display scale
- use the same mascot asset and same target display dimensions in both implementations

The mascot asset is a benchmark placeholder, not the final product identity.

### P1. Composer

A global hotkey opens a small native chat/composer surface adjacent to or near the mascot.

Minimum behavior:

- editable text
- caret
- selection
- copy/paste
- keyboard navigation
- multiline input
- submit via keyboard
- close/hide without terminating the process
- preserve mascot after chat closes

Text must be implemented as a real input control/path, not a demo string painted on a canvas.

### P2. Streamed response

Submitting text launches or talks to a tiny benchmark child process.

The child process emits a deterministic streamed response in small chunks.

The shell must:

- read output without blocking the UI
- display chunks incrementally
- support cancellation
- detect child exit
- drain stderr
- remain responsive during streaming

For this prototype, do NOT integrate Codex or Devin yet. The mock provider exists to compare shell/runtime behavior without network/model variance.

### P3. Lifecycle

The prototype must demonstrate:

- cold launch
- warm global-hotkey activation
- repeated show/hide
- repeated submit/cancel cycles
- clean shutdown
- no steady memory growth from repeated interactions

### P4. Minimal persistence

Persist only enough state to prove the intended direction:

- last window position
- last selected benchmark settings if any

Do not implement chat history/database architecture yet unless required by the chosen text/UI stack.

## 4. Explicit non-scope

Do not add the following to either prototype:

- terminal
- diff viewer
- code viewer/editor
- project tree
- Markdown renderer beyond what is necessary for the test
- syntax highlighting
- cloud sync
- accounts/auth
- RepoSuite
- Git integration
- browser automation
- browser extension
- screenshots or screenshot annotation
- microphone, STT, TTS
- local AI models
- Codex app-server
- Devin ACP
- MCP
- plugins
- auto-update
- installer
- telemetry
- production branding
- elaborate animations

Every extra subsystem makes the language comparison less useful.

## 5. Platform order

### Stage A — Windows

Implement and benchmark both prototypes on Windows first.

Windows is the initial comparison platform because it lets us validate:

- transparent native windows
- global hotkeys
- drag behavior
- text input
- process I/O
- memory and handle behavior

Do not begin macOS/Linux implementation until the Windows comparison is complete enough to decide whether both candidates deserve continuation.

### Stage B — macOS validation

If both stacks remain viable, port the same slice to macOS and validate:

- transparent floating panel behavior
- focus/activation
- global shortcut strategy
- text/IME path
- startup and memory

### Stage C — Linux feasibility

Linux comes after the language decision. Treat X11 and Wayland as different capability environments. Do not require fake parity where Wayland intentionally restricts behavior.

## 6. Fairness rules

The benchmark must compare equivalent products.

Both implementations must use:

- the same mascot source asset
- the same displayed mascot dimensions
- the same default chat dimensions
- the same visible text
- the same mock-provider protocol and output
- the same number of response chunks
- the same benchmark scenario durations
- release/optimized builds
- no debugger attached
- no intentionally preloaded heavy components in only one implementation

If a stack requires a materially different architecture, document the difference rather than hiding it.

Framework/library choice is part of the comparison. We are comparing realistic candidate stacks, not forcing identical low-level dependencies.

## 7. Candidate-stack freedom

The implementer may propose the smallest defensible stack for each language, but must document why.

### Rust

Do not automatically use a large GUI framework.

Candidates may include:

- native Windows APIs plus a small custom rendering/text layer
- SDL3 where it materially reduces platform work
- tiny-skia or equivalent CPU rendering
- cosmic-text/swash or another serious text path

GPU rendering is not required for the first prototype.

### Zig

Do not force an all-Zig dependency stack if a mature C/native component is the better engineering choice.

Candidates may include:

- Win32 directly
- SDL3 where useful
- a small custom renderer
- native/C text libraries where necessary

The Zig prototype must not obtain a benchmark advantage by omitting correct input behavior required from Rust.

## 8. Memory architecture requirements

Both prototypes must follow the same resource discipline:

- no full-screen RGBA surface for a tiny mascot
- no retained duplicate decoded image buffers without reason
- bounded text/glyph/layout caches
- release temporary image buffers after upload/presentation
- event-driven redraw
- avoid polling loops
- bounded subprocess queues
- no unbounded transcript accumulation
- close/release chat resources when hidden where practical

Peak memory and post-operation memory are both relevant.

## 9. Acceptance targets

These are engineering targets, not assumed outcomes.

For the application-owned process(es), excluding the benchmark child/provider process:

- mascot-only steady state: target < 20 MiB, stretch < 15 MiB
- small chat open: target < 50 MiB, hard concern above 80 MiB
- idle CPU after settling: effectively zero; target < 0.1% of one core averaged over a meaningful interval
- warm hotkey to visible composer: target p95 < 50 ms
- received response chunk to visible update: target < 33 ms under normal load
- no monotonic memory growth across repeated show/hide and submit/cancel cycles

These thresholds are not pass/fail language verdicts by themselves. The final choice considers engineering complexity too.

## 10. Required benchmark scenarios

At minimum:

1. cold start to first visible mascot
2. 60 seconds mascot idle
3. open/close composer 100 times
4. type and edit representative Unicode text
5. submit deterministic mock response
6. cancel midway
7. run 100 submit/complete cycles
8. keep composer open and idle 10 minutes
9. move mascot between monitors/scales where available
10. final memory measurement after returning to mascot-only state

The benchmark protocol defines exact collection details.

## 11. Representative text correctness set

At minimum test:

- ASCII English
- Latvian diacritics
- Cyrillic
- combining marks
- emoji including multi-codepoint sequences
- mixed LTR/RTL sample
- dead-key input
- IME composition on a machine/input method where available

A language stack that saves a few MiB but requires us to build a fragile text editor from scratch should be treated accordingly.

## 12. Repository shape after implementation begins

Expected high-level layout:

```text
mascot/
├── assets/
├── docs/
├── benchmark/
├── rust/
└── zig/
```

The two implementations should not share compiled code. They may share:

- static assets
- protocol fixtures
- benchmark scripts
- test vectors
- documentation

## 13. Deliverables from each implementation

Each candidate must provide:

- build instructions
- release-build command
- dependency list with purpose
- architecture note
- executable prototype
- automated or reproducible benchmark procedure
- raw benchmark output
- known platform limitations
- implementation LOC summary excluding vendored/generated code
- brief list of hacks/workarounds required

No final language verdict should be written by the implementation agent.

## 14. Decision criteria

After measurements, compare:

1. idle and active memory
2. startup/hotkey latency
3. idle wakeups/CPU
4. correctness of native window behavior
5. text/input correctness
6. implementation complexity
7. dependency surface
8. OS-specific glue size
9. debugging experience
10. build/release complexity
11. repeated-operation stability
12. amount of infrastructure we would have to own long-term

A small memory win is not automatically decisive.

Example principle:

- 4 MiB less memory with substantially more fragile text/native infrastructure probably does not justify a stack.
- 20–30 MiB less memory plus simpler runtime behavior may justify revisiting the trade-off.

## 15. Stop conditions

Pause a candidate implementation and report rather than papering over the problem if:

- transparent presentation requires a fragile unsupported hack
- correct text input clearly requires building a large custom subsystem
- idle rendering cannot be made event-driven
- a dependency unexpectedly embeds a browser/WebView runtime
- repeated interaction shows unexplained unbounded memory/resource growth
- platform glue becomes larger than the product slice itself

## 16. What happens after this comparison

Only after choosing the foundation do we add real product capabilities, roughly in this order:

1. daemon/shell separation
2. real Codex app-server adapter
3. Devin ACP adapter
4. file/clipboard/window/screenshot context
5. push-to-talk / speech
6. browser context/control
7. cloud/session synchronization
8. richer session/history UI
9. RepoSuite integration as a much later structured-memory layer

The benchmark should not pre-build these stages.
