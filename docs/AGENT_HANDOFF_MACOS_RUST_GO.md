# Agent Handoff — macOS Rust vs Go Critical Slice

Task ID: MASCOT-MACOS-RUST-GO-001

## Objective

Run the mandatory macOS Stage B validation for the two surviving Windows candidates:

- Rust
- Go

Zig is intentionally out of scope for this stage.

This is **not** a repeat of the full Windows benchmark and not a product-UI redesign. The goal is to determine whether either candidate develops a structural macOS problem that could change the foundation decision.

## Base

Repository: rceman/mascot

This branch was created from the final Windows comparison head:

    7858b54a1f5ec07f370e8e767dc5d751eebbaed6

Work only on:

    agent/macos-rust-go-critical-slice-v1

Do not modify the Windows benchmark evidence except to add cross-links or macOS-specific notes where clearly appropriate.

## Read first

Read in full:

- README.md
- docs/PROTOTYPE_PLAN.md
- docs/BENCHMARK_PROTOCOL.md
- docs/AGENT_SOURCE_EFFICIENCY.md
- docs/BENCHMARK_RESULTS.md
- rust/ARCHITECTURE.md
- go/ARCHITECTURE.md
- benchmark/FIXTURE_FREEZE.md
- benchmark/manifest/fixture.json
- docs/TEXT_FIXTURES.md
- docs/MOCK_PROVIDER_CONTRACT.md

The Windows results are evidence, not a specification to copy blindly.

## Stage B scope

Implement the smallest realistic **native macOS critical slice** for Rust and Go that demonstrates:

1. transparent floating mascot window
2. borderless presentation
3. always-on-top policy appropriate for a desktop helper
4. mascot drag behavior
5. click-through outside the alpha hit region
6. non-activating mascot behavior where practical
7. global shortcut
8. activating a real text composer
9. caret, selection, copy/paste, multiline input
10. native macOS text input / composition
11. representative Unicode rendering
12. Japanese IME composition commit + cancel
13. streamed response rendering from the shared fixture
14. cancellation
15. hide/show lifecycle
16. clean child-process teardown
17. event-driven idle behavior
18. startup and idle resource measurements
19. an actual macOS .app bundle

Do not implement product Bubble UI yet. Reuse the benchmark interaction model where practical so this stage remains a platform-validation experiment.

## Candidate implementation rule

Extend the existing Rust and Go candidate implementations with a realistic macOS path rather than creating an unrelated toy program solely for this gate.

Platform-specific code is expected.

Record exactly:

- shared code reused from Windows candidate
- macOS-specific source files
- macOS-specific LOC
- macOS-specific source tokens using the already frozen o200k_base tokenizer
- unsafe/FFI/native bridge boundaries
- frameworks/libraries used
- project-owned shims/workarounds

Do not deliberately force identical architecture when one language has a more realistic native path.

## Native UI policy

No:

- Electron
- Chromium
- WebView-based primary UI
- Tauri frontend
- Node runtime

Use native macOS facilities and realistic language bindings/libraries.

For Rust, Objective-C/AppKit bindings/shims are acceptable.

For Go, cgo/Objective-C/AppKit shims are acceptable.

Any helper process owned by a candidate must be disclosed and counted.

## Shared fixture

Use the same fixture semantics and frozen payloads as Windows.

The committed Windows fixture executable is not portable; build the shared fixture source natively for darwin/arm64 while preserving:

- protocol
- payloads
- fragmentation vectors
- cancellation behavior
- frame limits
- sanitized environment contract

Record the darwin fixture binary hash and toolchain.

Do not change fixture semantics to make one candidate pass.

## macOS text/IME gate

Use the built-in macOS Japanese input source / IME.

Before accepting the text path, verify:

- Japanese input source available
- preedit/composition visibly works
- commit works
- cancel works
- submit during active composition does not accidentally send
- hide during composition leaves committed state correct

Also verify representative:

- Latvian
- Cyrillic
- combining marks
- emoji sequence
- Arabic shaping / bidi rendering

Use the same logical expectations as the Windows fixture where applicable, adapting only platform-specific input actions.

## App-bundle requirement

Each candidate must produce and launch as a real application bundle:

    *.app

No installer is required.

Record:

- bundle structure
- executable size
- adjacent/runtime payload
- Info.plist
- signing state
- quarantine/notarization limitations if any

Ad-hoc signing is acceptable for local validation when needed.

## Measurements

This is a narrow Stage B gate, not the full Stage A campaign.

For both Rust and Go collect at minimum:

- fresh launch to first visible mascot
- first composer activation
- warm composer activation
- mascot-only idle RSS
- mascot-only idle physical footprint if available
- composer-open RSS / footprint
- streaming active peak RSS / footprint
- idle one-core CPU
- thread count
- child-process inventory
- clean shutdown
- short repeated show/hide stability check
- short repeated submit/cancel stability check

Use the same Mac, same power state, same fixture, same display, and equivalent release builds.

For short latency/resource measurements, use enough repetitions to expose gross differences; do not reproduce the entire Windows 30/100/3x100 campaign unless uncertainty actually requires it.

Document the macOS measurement source/tool for every metric.

Do not perform benchmark-only forced GC, allocator purge, cache purge, or process-footprint trimming.

## Build / agent-efficiency evidence

Record:

- clean release build time
- incremental no-op build time
- incremental one-file-change build time
- one representative edit -> build -> targeted test loop
- final total candidate source tokens
- macOS-specific source tokens
- macOS-specific LOC
- native/FFI bridge lines
- project-owned workarounds

Do not rewrite architecture merely to optimize these numbers.

## Devin ACP compatibility

For macOS Stage B, use **Devin ACP instead of Codex app-server** as the real-provider compatibility gate.

Do not run the Codex app-server compatibility gate for this macOS task unless explicitly requested later.

Use the locally available/current Devin ACP interface and record:

- Devin/ACP client or endpoint identity/version
- transport used
- launch/connect procedure
- initialization/session establishment
- one bounded read-only or otherwise non-destructive interaction supported by the available ACP contract
- streamed/notification evidence where the ACP contract exposes it
- interrupt/cancel if supported by the available ACP contract; otherwise normal completion
- clean disconnect/teardown
- candidate-owned adapter/process inventory

Do not invent ACP methods or message shapes. Discover and use the actual installed/available Devin ACP contract.

The gate is compatibility-focused and untimed with respect to model/network response latency.

If Devin ACP itself requires an interactive login/authorization step that cannot be completed unattended, pause and report exactly what the user must approve.

## Visual evidence

Capture for Rust and Go:

- mascot-only screenshot
- composer-open screenshot
- response-visible screenshot
- one short interaction recording showing:
  - launch
  - drag
  - click-through
  - shortcut
  - typing/composition
  - submit
  - stream
  - hide

Do not redesign or polish the UI for screenshots.

## Required outputs

Create:

    benchmark/results/macos/
    benchmark/results/macos/RESULTS.json
    benchmark/results/macos/summary.md
    benchmark/results/macos/raw/
    benchmark/results/macos/visual/
    docs/MACOS_RUST_GO_REPORT.md

The report must contain one factual Rust-vs-Go table and discuss:

- correctness
- native-window behavior
- activation/focus behavior
- IME/text behavior
- response rendering
- process I/O
- startup/idle footprint
- stability
- macOS-specific glue
- source-token/LOC burden
- build/iteration friction
- workarounds and structural risks

Do not hide a platform workaround because the app still passes.

## Stop conditions

Stop a candidate and report before papering over it if:

- transparent/nonactivating window behavior requires a fragile unsupported hack
- global shortcut/focus behavior is structurally unreliable
- native text/IME correctness requires a disproportionate custom subsystem
- idle behavior requires continuous redraw/polling
- child cleanup is unreliable
- the chosen realistic stack cannot produce a usable .app bundle without an architecture change

A modest amount of AppKit glue is not itself a failure.

## Environment / installs

You may install ordinary non-destructive development prerequisites without asking again, including Rust/Go toolchains and normal package-manager dependencies.

Prefer existing tools when already installed.

Do not:

- disable macOS security controls
- change SIP
- modify unrelated system settings
- automatically reboot
- remove existing toolchains/apps

If an interactive admin/security approval is required and cannot be completed unattended, pause and report exactly what the user must approve.

## Git policy

Commit logical milestones.

Do not squash away evidence needed to compare platform-specific engineering effort.

Push the branch and leave the worktree clean.

## Final report

Return:

1. base SHA
2. final branch and HEAD
3. Mac hardware / macOS version / architecture
4. Rust stack + bundle + result summary
5. Go stack + bundle + result summary
6. correctness table
7. resource/latency table
8. macOS-specific LOC/token/FFI table
9. build/feedback-loop table
10. visual evidence paths
11. Devin ACP gate status
12. structural risks / workarounds
13. unavailable measurements and reasons
14. commits created
15. clean-worktree confirmation

End with exactly one of:

    MACOS_CRITICAL_SLICE_COMPLETE

or

    MACOS_CRITICAL_SLICE_BLOCKED: <reason>

Do not begin the production Bubble Shell in this task.
