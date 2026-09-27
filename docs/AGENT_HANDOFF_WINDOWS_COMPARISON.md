# Agent Handoff — Windows Rust/Zig/Go Comparison

Task ID: MASCOT-WIN-COMPARE-001

## Objective

On Windows, build **all three candidate applications** — Rust, Zig, and Go — against the same frozen benchmark fixture, then run the shared benchmark and produce a factual comparison report.

This is one end-to-end task for one agent so the implementation effort, machine, benchmark harness, and interpretation stay as consistent as possible.

Do not choose a language by intuition before measurement.

## Base

Repository: rceman/mascot

Start from the current approved planning head on:

    plan/rust-zig-prototype-v0.1

Create one implementation branch from that exact head:

    agent/windows-rust-zig-go-comparison-v1

Do not modify main directly.

## Read first

Read these files in full before coding:

- README.md
- docs/PROTOTYPE_PLAN.md
- docs/BENCHMARK_PROTOCOL.md
- docs/ACCEPTANCE_MATRIX.md
- docs/TEXT_FIXTURES.md
- docs/MOCK_PROVIDER_CONTRACT.md
- docs/AGENT_HANDOFF_FIXTURE_HARNESS.md
- rust/README.md
- zig/README.md
- go/README.md

These documents are normative.

If two requirements conflict, stop and report the conflict. Do not silently reinterpret the benchmark.

## High-level sequence

Execute in this order:

1. Preflight the Windows environment.
2. Prepare, validate, version, and freeze the shared fixture/harness.
3. Freeze one shared mascot asset and UI dimensions.
4. Write architecture notes for Rust, Zig, and Go before implementation.
5. Implement Rust candidate.
6. Implement Zig candidate.
7. Implement Go candidate.
8. Run shared correctness gates for all three.
9. Correct only candidate-specific defects needed to reach equivalent correctness.
10. Run the Windows benchmark with balanced candidate order.
11. Run stability/resource-growth scenarios.
12. Produce raw results and a factual comparison report.
13. Commit all work and leave the branch clean.

Do not skip directly to candidate implementation before the fixture/harness freeze.

---

# Phase 0 — Windows preflight

Record:

- Windows version/build
- CPU
- RAM
- power mode
- display topology
- display scale factors
- refresh rate
- Rust toolchain
- Zig toolchain
- Go toolchain
- native build tools / Windows SDK
- Git commit used as task base

Stage A must run native Windows executables. Do not benchmark Linux binaries through WSLg.

Use equivalent Windows-native deployment locations for all three binaries.

If the required two-scale display setup is not available, document it. A reproducible virtual display is acceptable if it exercises the same Windows DPI transition path for every candidate.

---

# Phase 1 — Shared fixture/harness freeze

Implement the common fixture/harness first, following:

    docs/AGENT_HANDOFF_FIXTURE_HARNESS.md

Required outputs include:

    benchmark/
    assets/
    benchmark/FIXTURE_FREEZE.md

Do not maintain separate fixtures per candidate.

The same provider binary, payloads, decoder vectors, text fixtures, manifest, launch configuration, and benchmark harness must be consumed by Rust, Zig, and Go.

## Fixture implementation choice

Choose one pragmatic implementation language for the shared harness/provider.

This language is **not** a candidate result.

Document the choice.

If Go is used for the shared provider/harness, explicitly freeze and sanitize:

- GOMAXPROCS
- GOGC
- GOMEMLIMIT
- relevant GODEBUG/runtime controls

Candidate-specific runtime environment must not change the excluded shared provider behavior.

## Mascot asset

Use one shared transparent mascot PNG for all candidates.

If an approved transparent mascot asset is already present in the working task context or repository, freeze it by:

- path
- SHA-256
- pixel dimensions
- logical display dimensions
- alpha/hit-mask rule

If no approved mascot asset is available, use one clearly marked temporary benchmark mascot asset that is identical for all three candidates, record it as PROVISIONAL in FIXTURE_FREEZE.md, and continue the Windows language comparison. Do not independently create a different asset per candidate.

The benchmark comparison must not be blocked by artwork polish.

---

# Phase 2 — Candidate architecture notes

Before coding each application, create:

    rust/ARCHITECTURE.md
    zig/ARCHITECTURE.md
    go/ARCHITECTURE.md

Each must state the intended Windows stack and justify:

- window creation
- per-pixel transparency
- always-on-top behavior
- hit testing
- dragging
- global hotkey
- composer implementation
- text/IME implementation
- response-text rendering
- rendering/presentation backend
- visible-presentation observation hook
- subprocess/stdin/stdout/stderr handling
- cancellation
- parser/framing
- cache bounds
- idle redraw behavior
- allocator/runtime/GC strategy
- application/helper process inventory
- expected native bridge code
- direct dependencies
- build flags
- expected memory/runtime tradeoffs

Do not copy one stack mechanically into all three if a different realistic native stack is better for a language.

Do not deliberately handicap any candidate for dependency symmetry.

---

# Phase 3 — Rust candidate

Implement the full Windows slice in:

    rust/

It must satisfy the same acceptance matrix as the other candidates.

Do not add product features outside benchmark scope.

Do not treat large Rust GUI frameworks as forbidden, but justify every major dependency against:

- correctness
- process footprint
- startup
- idle behavior
- infrastructure ownership

Build an optimized/release binary.

Record exact build command and dependency set.

---

# Phase 4 — Zig candidate

Implement the full Windows slice in:

    zig/

It must satisfy the same acceptance matrix as Rust and Go.

Mature C/native libraries are allowed.

Do not omit or weaken:

- IME
- bidi/Arabic shaping
- emoji/combining rendering
- response rendering
- correct click-through/hit testing

merely to obtain a smaller binary or RSS.

Pin the Zig version used.

Build an optimized/release binary and record exact flags/dependencies.

---

# Phase 5 — Go candidate

Implement the full Windows slice in:

    go/

It must be a real candidate shell, not only a daemon behind another candidate's UI.

Allowed where justified:

- direct Win32
- cgo
- mature native libraries
- Gio or another native rendering layer

Record:

- Go version
- build flags
- GOMAXPROCS
- GOGC
- GOMEMLIMIT
- non-default runtime controls
- cgo usage
- native allocation ownership

Do not exclude Go runtime/GC memory or CPU from application totals.

Do not force GC before headline memory readings.

---

# Phase 6 — Correctness gate

Run docs/ACCEPTANCE_MATRIX.md for all three.

Required result:

- every required case PASS

UNTESTED is not PASS.

Performance data from a candidate with required FAIL/UNTESTED remains diagnostic only and is not eligible for final comparison.

## Text correctness

Apply docs/TEXT_FIXTURES.md to:

- composer editing
- response rendering

Verify:

- combining marks
- emoji ZWJ sequence
- selected-sequence deletion
- Arabic contextual shaping
- bidi ordering
- Latvian
- Cyrillic
- dead-key input
- Microsoft Japanese IME
- submit/hide during composition
- clipboard
- multiline content

Do not accept "toolkit limitation" as a pass unless the shared fixture explicitly permits it.

## Provider correctness

Run all shared cases:

- normal stream
- fragmented/coalesced frames
- direct decoder vectors
- client_request/client_response
- canonical cancellation barrier
- stderr pressure
- backpressure
- maximum frame
- oversized frame recovery
- unexpected exit
- clean shutdown

---

# Phase 7 — Fair correction pass

Before benchmarking, each candidate gets one focused correction pass if needed for:

- an accidental busy loop
- obviously wrong release configuration
- incorrect dependency feature selection
- a straightforward correctness defect
- unbounded queue/cache caused by implementation mistake

Do not redesign one candidate repeatedly until it wins.

Document every correction in:

    benchmark/corrections/

A candidate may use a materially different stack only if the original stack fails a structural requirement. If so, document why.

---

# Phase 8 — Windows benchmark

Follow docs/BENCHMARK_PROTOCOL.md.

Use a balanced three-candidate order rather than always Rust -> Zig -> Go.

Freeze the exact run order in the harness.

At minimum collect:

- fresh-process startup
- post-reboot first launch if feasible
- first composer activation
- warm hotkey activation
- fresh mascot private working set
- warm mascot private working set
- private commit
- composer footprint
- streaming observed peak
- cancellation peak/recovery
- one-core CPU
- redraw/present count
- threads
- kernel handles
- USER objects
- GDI objects
- live child cleanup
- executable size
- runtime payload size

Run repeated stability batches exactly as specified in the benchmark protocol.

Do not:

- force GC
- force allocator purge
- trim working set
- prewarm one candidate differently
- run intrusive language-specific diagnostics only in headline measurements

Runtime-specific diagnostic passes are separate.

---

# Phase 9 — Runtime diagnostics

These explain headline OS metrics; they do not replace them.

## Go

Record where available:

- HeapAlloc
- marked-live metric if used, labeled correctly
- HeapSys
- HeapInuse
- GC count
- GC pause total
- goroutines
- GOMAXPROCS
- GOGC
- GOMEMLIMIT
- cgo/native allocation notes

Remember:

- HeapAlloc is not the same as reachable/live memory
- HeapSys is not resident RAM
- GC pause total is not total GC CPU cost
- GOMEMLIMIT is not a total-process cap

## Rust/Zig

If cheap allocator/runtime diagnostics are available, record them as supplemental data.

Do not add substantial instrumentation overhead only to one candidate.

---

# Phase 10 — Results

Create:

    benchmark/results/windows/
    benchmark/results/windows/raw/
    benchmark/results/windows/summary.md
    benchmark/results/windows/RESULTS.json

The summary must include one common table with Rust, Zig, and Go.

At minimum include:

| Metric | Rust | Zig | Go |
|---|---:|---:|---:|
| Correctness eligibility | | | |
| Fresh startup median | | | |
| First composer activation | | | |
| Warm hotkey p95 | | | |
| Fresh mascot PWS | | | |
| Warm mascot PWS | | | |
| Warm mascot private commit | | | |
| Composer PWS | | | |
| Composer private commit | | | |
| Streaming observed peak PWS | | | |
| Streaming observed peak commit | | | |
| Idle one-core CPU | | | |
| Idle redraw count | | | |
| Threads | | | |
| Kernel handles | | | |
| USER objects | | | |
| GDI objects | | | |
| Stripped executable size | | | |
| Runtime payload size | | | |
| Handwritten LOC | | | |
| Platform-specific LOC | | | |

Also include:

- exact candidate configuration
- toolchain versions
- dependency/backend choice
- build flags
- allocator/runtime policy
- known limitations
- corrections applied
- raw result references

Do not combine best memory from one configuration with best latency from another.

---

# Phase 11 — Comparison report

Create:

    docs/WINDOWS_COMPARISON_REPORT.md

This report is factual and evidence-based.

It must discuss:

- measured memory
- measured startup/interaction latency
- idle CPU/redraw behavior
- stability/resource growth
- correctness
- dependency surface
- amount of native/platform glue
- implementation effort
- debugging friction
- build complexity
- notable runtime/GC behavior
- workarounds/hacks

Do **not** hide a candidate's engineering complexity just because its benchmark number is good.

Do **not** choose a final cross-platform product language yet.

Windows is a screening/comparison stage.

The report may state factual Windows findings such as:

- candidate A used less private working set
- candidate B required fewer project-owned workarounds
- candidate C had materially higher retained commit

But final product selection still requires the plan's macOS validation and Linux feasibility gate.

---

# Build/test policy

Use the local Windows machine and native toolchains.

Do not use GitHub Actions/CI for ordinary development or benchmarking.

Run builds/tests locally.

Do not repeatedly rerun expensive full benchmark suites after every small source edit. Use targeted checks during implementation, then the frozen full run once candidates are correctness-ready.

---

# Git policy

Work only on:

    agent/windows-rust-zig-go-comparison-v1

Commit logical milestones.

Suggested commits:

1. shared fixture/harness freeze
2. Rust candidate
3. Zig candidate
4. Go candidate
5. correctness fixes
6. benchmark tooling qualification
7. Windows benchmark results
8. comparison report

Do not rewrite published benchmark evidence after the fact. If a rerun is required, create a new result run/version and explain why.

Keep the worktree clean at completion.

---

# Final report to Planner

Return:

1. base SHA
2. final branch
3. final HEAD
4. fixture version and freeze commit
5. Rust architecture/build/result summary
6. Zig architecture/build/result summary
7. Go architecture/build/result summary
8. correctness matrix status
9. benchmark run IDs
10. result table
11. comparison report path
12. unresolved Windows issues
13. blockers for later macOS validation
14. exact commits created
15. clean-worktree confirmation

End with exactly one of:

    WINDOWS_COMPARISON_COMPLETE

or

    WINDOWS_COMPARISON_BLOCKED: <reason>

Do not claim a final cross-platform language winner in this task.
