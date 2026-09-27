# Rust vs Zig vs Go Prototype Plan v0.3

Status: **revised after second Astra review; ready for final review before implementation**

## 1. Decision we are trying to make

Choose the primary implementation language and native UI approach for an eventual ultra-light desktop AI agent shell.

The comparison is between **Rust**, **Zig**, and **Go** implementations of the same narrow prototype.

The useful question is:

> Which realistic stack gives us the best combination of low steady-state memory, low latency, native desktop integration, correctness, implementation simplicity, and long-term maintainability for the product we intend to build?

This is not a hello-world benchmark and not a theoretical language-runtime comparison.

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
- external provider processes are accounted separately from the shell
- platform-specific native shims are acceptable
- correctness of text input and OS behavior matters more than framework purity

## 3. Shared contracts and fixture freeze gate

Before any candidate application implementation starts, these documents are normative:

- docs/ACCEPTANCE_MATRIX.md
- docs/TEXT_FIXTURES.md
- docs/MOCK_PROVIDER_CONTRACT.md
- docs/BENCHMARK_PROTOCOL.md

After plan approval, prepare and freeze the common fixture/harness **before Rust, Zig, or Go application implementation begins**.

The shared fixture manifest freezes at minimum:

- fixture version
- mascot asset ID/hash and logical dimensions
- composer/response dimensions
- text/action fixtures
- permitted native text variations
- response payloads and sizes
- scenario list
- fragmentation plan
- direct decoder-fragment vectors
- cancellation behavior
- exceptional recovery behavior
- timeout values
- frame limits
- expected terminal events
- benchmark font/input configuration

Any later change is versioned and invalidates every affected earlier correctness or performance result, including results collected before another candidate existed.

A candidate whose required correctness cases are FAIL or UNTESTED is not eligible for final performance comparison until corrected.

## 4. Prototype scope

All three implementations MUST provide equivalent observable behavior.

### P0. Floating mascot

A small transparent borderless mascot window:

- per-pixel transparency
- no visible rectangular background
- always available above ordinary application windows according to the benchmark policy
- draggable through the shared mascot hit region
- transparent exterior must not intercept clicks intended for a separate underlying app
- no full-screen transparent backing window
- no continuous redraw when stationary
- support the shared 1x/2x and mixed-scale display tests
- use the same mascot source asset and logical dimensions

The mascot asset is a benchmark placeholder, not the final product identity.

### P1. Composer

A global hotkey opens a small native chat/composer surface adjacent to or near the mascot.

Minimum behavior:

- real editable text path
- caret
- selection
- copy/paste
- keyboard navigation
- multiline input
- submit via keyboard
- close/hide without terminating the shell
- preserve mascot after chat closes
- satisfy docs/ACCEPTANCE_MATRIX.md
- satisfy docs/TEXT_FIXTURES.md

Rendering sample strings is not sufficient. The composer must meet the shared editing oracle.

### P2. Plain-text response view

The response view is non-editable but must render the same representative Unicode correctly.

It must satisfy the visual cases from docs/TEXT_FIXTURES.md, including:

- combining-mark presentation
- emoji-sequence presentation
- mixed LTR/RTL ordering
- Arabic contextual shaping
- Latvian and Cyrillic text

A candidate cannot pass through a correct native composer while using an incorrect cheaper response renderer.

### P3. Streamed response

All candidates use the same persistent mock-provider fixture defined in docs/MOCK_PROVIDER_CONTRACT.md.

The shell must:

- parse the shared framed stream correctly
- pass the direct decoder-fragment vectors
- read output without blocking the UI
- display chunks incrementally
- support the deterministic cooperative-cancellation barrier
- handle one provider-initiated client request
- detect child exit
- continuously drain stderr
- remain responsive under shared backpressure
- enforce the shared frame-size limit
- follow the shared exceptional-session recovery policy
- clean up the child within shared timeouts

Hide/show does not alter cancellation semantics: an active response continues while hidden and remains available when reopened. Explicit cancel is separate.

### P4. Lifecycle

The prototype must demonstrate:

- fresh-process startup
- first composer activation
- warm global-hotkey activation
- repeated show/hide
- repeated submit/complete cycles
- repeated cancel cycles
- exceptional provider recovery
- clean shutdown
- no unexplained monotonic growth in application-owned resources

No persistence layer is required.

## 5. Explicit non-scope

Do not add:

- terminal
- diff viewer
- code viewer/editor
- project tree
- Markdown rendering; plain text is enough
- syntax highlighting
- chat database/history architecture
- cloud sync
- accounts/auth
- RepoSuite
- Git integration
- browser automation
- browser extension
- screenshots or screenshot annotation
- microphone, STT, TTS
- local AI models
- full Codex product integration
- Devin ACP integration
- MCP
- plugins
- auto-update
- installer
- telemetry
- production branding
- elaborate animations

Every extra subsystem makes the comparison less useful.

## 6. Platform decision gates

### Stage A — Windows screening

Implement and benchmark all three candidates as native Windows applications.

Windows validates:

- transparent native windows
- hit testing and drag behavior
- always-on-top policy
- global hotkeys
- real text input / IME
- response rendering
- process I/O
- startup/activation latency
- memory, CPU, handles, USER/GDI objects, windows and child lifecycle

Windows may eliminate a candidate with an unresolved structural problem.

A Windows result alone cannot establish the final cross-platform language choice.

### Stage B — mandatory macOS validation

Every candidate eligible for final selection must pass the macOS critical slice, including a sole Windows survivor.

Validate:

- floating/nonactivating window behavior
- activation into a working composer
- global shortcut strategy
- text/IME composition
- response rendering
- native resource lifetime
- process I/O
- basic startup and idle footprint

Use an actual macOS application bundle. An installer is not required.

### Stage C — Linux feasibility before final selection

Full Linux implementation and benchmarking may follow the language decision.

Before final selection, document for every surviving stack:

- Linux dependency path
- X11 support path
- Wayland capability floor
- accepted degraded behavior for positioning / always-on-top / shortcuts
- any compositor-specific protocol requirement

If a decision-threatening uncertainty remains, perform a narrow technical spike. Do not build full Linux candidates merely for symmetry.

## 7. Fairness rules

All candidates must use:

- the same mascot source asset
- the same logical mascot dimensions
- the same composer/response dimensions
- the same shared correctness matrix
- the same text/visual fixtures
- the same mock-provider executable and manifest
- the same visible fixture content
- the same benchmark scenarios and durations
- equivalent release/optimized build intent
- no debugger attached during headline measurements

Initialization/preloading strategies may differ when they are realistic shipping choices.

Their costs must be exposed through:

- startup measurement
- first-use measurement
- warm-state measurement
- retained-memory measurement

No benchmark-only prewarming or unmeasured preparatory work is allowed.

If a stack requires a materially different architecture, document the difference rather than hiding it.

## 8. Candidate-stack freedom

We compare realistic product stacks, not artificially symmetric dependency graphs.

For Rust, Zig, and Go:

- CPU, GPU, native-widget and custom-rendered approaches are allowed
- mature native/C/system libraries are allowed
- platform-native shims are allowed
- dependency symmetry is not required
- allocator/threading/cache strategy may differ
- correctness and accounting requirements do not differ
- application-owned helper processes are counted

Go-specific freedom:

- direct Win32 use is allowed
- cgo is allowed when justified
- Gio or another native rendering layer is allowed
- Objective-C/AppKit bridge code is allowed on macOS
- Go runtime/GC memory and CPU are part of the application cost and are never excluded from headline process metrics

The implementation agent must justify the chosen stack by:

- correctness
- measured resource cost
- amount of project-owned infrastructure
- long-term maintenance implications

A focused correction of an accidental busy loop, unsuitable first library choice, or obvious configuration mistake is permitted before treating the result as evidence against the language.

## 9. Resource-discipline requirements

All prototypes must follow the same product-level constraints:

- no full-screen RGBA backing surface for a tiny mascot
- no retained duplicate decoded image buffers without reason
- bounded text/glyph/layout caches
- release temporary image buffers after upload/presentation where practical
- event-driven redraw while idle
- avoid polling loops
- bounded subprocess queues
- fixed response/history retention policy from the mock contract
- disclose application-owned helper processes
- disclose retained renderer/text/native caches that materially affect steady state

A stable one-time cache warm-up is not automatically a leak. Repeated equivalent workloads must plateau.

For Go, GC/runtime diagnostics are supplemental only. They do not replace OS-level process accounting.

## 10. Acceptance targets

Targets are engineering guidance, not automatic language verdicts.

On Windows, memory target bands refer to **aggregate application-owned private working set**, excluding only the common provider fixture and measurement tools.

Also report private commit and peaks separately.

Targets:

- mascot-only steady state: target < 20 MiB, stretch < 15 MiB
- small chat open: target < 50 MiB, hard concern above 80 MiB
- idle CPU after settling: effectively zero; target < 0.1% of one core
- warm hotkey to visibly presented and input-ready composer: target p95 < 50 ms
- complete frame receipt to first presentation containing that content: target < 33 ms under normal load
- no unexplained monotonic application-resource growth across repeated operation batches

The targets do not change for Go. If the Go runtime materially increases the floor, that is part of the result.

## 11. Required benchmark scenarios

At minimum:

1. fresh-process startup to first visible mascot
2. post-reboot first-launch samples, reported separately
3. 60 seconds mascot idle
4. first composer activation
5. warm composer activation
6. open/close composer in repeated batches
7. execute shared Unicode/IME/text-visual acceptance cases
8. submit deterministic mock response
9. execute canonical cancellation barrier
10. repeated submit/complete batches
11. repeated cancellation batches
12. backpressure/failure/oversized-frame cases
13. composer-open idle
14. mixed-scale display transition
15. return to mascot-only warm state and measure retained footprint/resources

Exact collection procedure lives in docs/BENCHMARK_PROTOCOL.md.

## 12. Repository shape

Expected structure:

    mascot/
    ├── assets/
    ├── benchmark/
    ├── docs/
    ├── rust/
    ├── zig/
    └── go/

Candidate-owned application logic must be independently implemented.

All candidates may reuse:

- the same mature third-party/native libraries
- system frameworks
- static assets
- the common mock-provider executable
- fixture manifests
- test vectors
- benchmark tools
- documentation

Any new project-owned native infrastructure must be disclosed and counted where it is maintained.

## 13. Deliverables from each candidate

Each candidate must provide:

- build instructions
- exact release-build command
- compiler/toolchain version
- release-safety settings
- allocator/runtime settings where relevant
- GC/runtime settings where relevant
- LTO/stripping configuration where applicable
- required DLL/framework/runtime files
- direct dependency list with purpose
- notable transitive/native dependencies
- architecture note
- executable prototype
- correctness matrix results
- automated or reproducible benchmark procedure
- raw benchmark output
- known platform limitations
- handwritten LOC summary excluding vendored/generated code
- platform-specific LOC
- unsafe/FFI/native bridge LOC where applicable
- helper-process inventory
- list of project-maintained patches/forks/workarounds

No implementation agent writes the final language verdict.

## 14. Pre-final real-provider compatibility gate

The deterministic mock remains the comparative benchmark workload.

Before final language selection, every surviving candidate must also pass a small **untimed Codex app-server compatibility gate**:

1. launch a pinned app-server version over stdio
2. complete initialization
3. start one read-only interaction
4. receive streaming output
5. interrupt a turn or complete normally
6. clean up process and pipes

Do not compare model/network response times.

Devin ACP remains deferred unless review identifies an ACP-specific architectural obligation not covered by the structured mock plus Codex gate.

## 15. Decision criteria

Review raw data and engineering evidence across:

1. correctness eligibility
2. idle and active private working set
3. private commit and active peaks
4. startup/first-use/warm activation latency
5. idle CPU/redraw/wakeup behavior
6. repeated-operation stability
7. transparent-window/hit-test correctness
8. text/input/IME correctness
9. response-rendering correctness
10. implementation complexity
11. dependency surface
12. platform-specific integration burden
13. debugging/tooling friction
14. build/release complexity
15. amount of infrastructure the project would own long-term
16. Windows-to-macOS portability
17. runtime/GC cost where applicable

Do not reduce the decision to a weighted score before reviewing raw results.

A small memory win is not automatically decisive.

Examples:

- 4 MiB less memory with substantially more fragile text/native infrastructure probably does not justify a stack.
- 20–30 MiB less memory plus simpler runtime behavior may justify additional integration work.
- A modest Go memory premium may be acceptable if it buys materially simpler, more reliable orchestration; the benchmark must show the actual premium rather than assume it.

## 16. Stop / pause conditions

Pause a candidate and report rather than papering over the problem if:

- transparent presentation depends on a fragile unsupported hack
- required text/IME correctness clearly requires a disproportionate custom subsystem
- idle rendering cannot be made event-driven
- a dependency unexpectedly embeds a browser/WebView runtime
- repeated equivalent workloads show unexplained unbounded memory/resource growth
- process cleanup cannot reliably avoid orphaned children
- native/platform behavior remains structurally unsupported by the chosen stack

Do not pause merely because platform glue LOC exceeds generic application LOC in this small prototype. Judge maintainability and ownership complexity, not the ratio alone.

## 17. What happens after this comparison

Only after choosing the foundation do we expand product scope, roughly:

1. daemon/shell separation
2. real Codex app-server adapter
3. Devin ACP adapter
4. file/clipboard/window/screenshot context
5. push-to-talk / speech
6. browser context/control
7. cloud/session synchronization
8. richer session/history UI
9. RepoSuite integration much later

The benchmark must not pre-build these stages.
