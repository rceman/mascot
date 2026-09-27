# Benchmark Protocol v0.3

This protocol keeps the Rust, Zig, and Go comparison repeatable and prevents artificial wins from different accounting, lifecycle, runtime, or presentation choices.

## 1. General rules

- Benchmark release/optimized builds only.
- Do not attach debugger/profiler during headline measurements.
- Run native Windows executables for Stage A; do not benchmark via WSLg.
- Benchmark all candidates from equivalent Windows-native deployment locations.
- Record OS build, CPU, RAM, power mode, display topology/scaling, refresh rate, compiler/toolchain versions, and commit SHA.
- Run all candidates on the same machine under the same display configuration.
- Alternate candidate order in balanced blocks.
- Report raw per-run data and distributions; do not report only a composite score.
- The common provider fixture and measurement tools are excluded from application totals. Candidate-specific helpers are not excluded.
- Candidate-specific runtime diagnostics are supplemental; OS-level process accounting remains authoritative for headline resource comparison.
- Each result set identifies the candidate build, target architecture, dependency/backend configuration, and allocator/runtime policy. Alternative configurations are reported separately; do not combine their best memory, CPU, and latency values into one candidate result.
- Benchmark-only forced GC, scavenging, allocator purges, working-set trimming, or cleanup immediately before sampling are prohibited. Deliberate shipping cleanup behavior is allowed only when declared and its resource/latency costs are measured.
- Supplemental diagnostics use documented low-intrusion snapshots or separate diagnostic runs. Do not introduce substantial language-specific instrumentation overhead only into one candidate's headline measurements.

## 2. Application-owned process inventory

Track the process inventory throughout the entire run, not only at startup.

For every application-owned process, record:

- PID
- role
- parent PID
- creation timestamp
- exit timestamp
- whether counted in aggregate application metrics

Only these may be excluded:

- the byte-identical shared mock-provider fixture
- measurement/instrumentation processes

Any candidate-specific worker, helper, renderer service, bridge, broker, crash handler, or runtime helper process is part of that candidate's application cost from birth to exit.

## 3. Required Windows metrics

Mandatory where the OS exposes them:

- private working set
- private bytes / private commit
- total working set
- process count
- thread count
- kernel handle count
- USER object count
- GDI object count
- live native window count
- live child-process count
- CPU user+kernel time
- redraw/present count while idle
- startup duration
- first composer activation duration
- warm hotkey activation duration
- stream receipt-to-presentation latency
- stream emission-to-presentation latency
- executable size
- required adjacent runtime/DLL payload size

If graphics APIs allocate process-attributable GPU memory, report it separately. If attribution is unavailable, state that explicitly rather than reporting zero.

### Memory target metric

The 20/50/80 MiB target bands refer to **aggregate application-owned private working set on Windows**.

Private commit and active peaks are mandatory companion metrics and must never be collapsed into the same column.

## 4. Common timing and observer contract

Before collecting headline latency data, qualify one common benchmark observer procedure and freeze it in the benchmark manifest.

### 4.1 Hotkey origin

The headline hotkey origin is the external harness input-injection/observation point, not the application's registered-hotkey callback.

For Windows Stage A, the harness timestamps immediately before the shared hotkey injection call.

The same injection path is used for all candidates.

### 4.2 Fresh-process startup with warm OS caches

Start:

- external harness issues the process launch request

End:

- first mascot frame is visibly presented according to the common visibility observer

Run at least 30 times per candidate initially.

Report:

- every raw sample
- median
- range
- p95 only when the sample count/distribution makes it useful

### 4.3 Post-reboot first launch

This is distinct from ordinary fresh-process startup.

Run at least 3 post-reboot observations per candidate initially.

Balance which candidate receives the actual first launch after boot.

Report raw values and median/range. Do not present a small-sample p95 as statistically strong.

### 4.4 First composer activation

In a newly launched process that has never opened the composer:

external hotkey injection -> composer visibly presented **and input-ready**.

Measure separately from warm activation.

### 4.5 Warm activation

With the composer path already exercised:

external hotkey injection -> composer visibly presented **and input-ready**.

Run at least 100 activations distributed across at least 3 process lifetimes.

Report median, p95, range, and per-process distribution.

### 4.6 Visibility and input-readiness observer

Before headline collection, the harness must document and validate one common procedure for all candidates.

At minimum distinguish:

- visible composer presentation
- input readiness

A valid approach may combine an external screen/compositor observation with a controlled injected text probe.

Application-side redraw requests, queue submissions, or presentation API returns are diagnostic markers only unless independently correlated with visible output.

Record observer resolution and uncertainty.

Differences smaller than observer resolution or normal run-to-run variation must not be treated as decisive.

### 4.7 Stream presentation

For each logical response chunk, record:

1. mock emission timestamp
2. complete logical-frame receipt timestamp before JSON decoding
3. first visible presentation containing that chunk

Multiple chunks may share one presentation.

Do not require one redraw per chunk.

Hidden-window streaming is a correctness/lifecycle case and is not included in visible-stream headline latency.

### 4.8 Mock emission timestamp

The mock-provider contract defines emission timestamp as:

- immediately before the first physical write attempt containing any bytes of that logical frame

This definition is shared by all candidates.

### 4.9 Clock contract

For Stage A, use a documented Windows high-resolution clock strategy compatible with QPC-derived timestamps.

Do not assume language-specific monotonic-clock epochs are interchangeable across processes.

Use one of:

- a common external measurement owner, or
- shared QPC-derived timestamps with documented conversion

## 5. CPU and idle behavior

Headline one-core CPU percentage is:

    100 * application user+kernel CPU-time delta / wall-clock delta

Do not divide by logical CPU count.

For idle scenarios record:

- composer focused/unfocused
- caret active/blinking or not
- pointer position
- no provider output
- no intentional animation
- display refresh rate
- power mode

Measure stationary mascot idle separately from focused-composer idle.

The stationary mascot target is below 0.1% of one core averaged over the defined idle window.

Record redraw/present counts. A stationary mascot should not maintain a periodic render loop.

"Wakeups" must name the measurement source. If ETW/WPA is used, report the relevant timer/wait/ready observations as diagnostic data; do not equate all context switches with timer wakeups.

## 6. Memory/resource states

Use a fixed settling period followed by a common collection window. Do not rely on one instantaneous sample.

### R0 — fresh mascot-only

- launch
- do not open composer
- settle 60 s
- collect a 30 s steady-state window

Purpose: startup footprint before text/provider paths are exercised.

### R1 — first composer open

- from fresh process, open composer for the first time
- measure active peak
- settle 60 s
- collect a 30 s steady-state window

Purpose: expose lazy font/input/toolkit/runtime initialization.

### R2 — warm composer open

- composer path already exercised
- open composer
- settle 60 s
- collect a 30 s steady-state window

### R3 — warm mascot-only after use

- exercise composer and provider path
- hide composer
- sample approximately 1 s, 10 s, and 60 s after hide
- collect a 30 s steady-state window after the 60 s point

Purpose: capture what an all-day resident mascot actually retains.

### R4 — normal streaming

- execute canonical mock response
- sample at fixed 100 ms interval
- retain observed active maxima
- also record OS high-water values where exposed

### R5 — cancellation

- execute canonical cancellation case
- capture active peak
- sample post-operation at approximately 1 s, 10 s, and 60 s

### R6 — focused composer idle

- composer open and focused
- no typing/provider output for 10 minutes
- record CPU, redraws, resource counts, before/after memory

## 7. Repeated-operation stability

Do not infer leaks from fresh-versus-warm differences alone.

Warm the relevant path first.

Then run **three successive batches of 100 operations** for:

- composer open/close
- normal submit/complete
- cancellation

Sample at least every 10-25 operations:

- private working set
- private commit
- kernel handles
- USER objects
- GDI objects
- threads
- live native windows
- live children
- application-owned renderer surfaces/resources where measurable
- application-owned text/layout/glyph cache counts or bytes where exposed
- bounded queue sizes

Use both:

1. fixed repeated content to test plateau behavior
2. bounded varying content/request IDs to exercise cache keys and eviction

The varying fixture must actually exercise declared cache limits/eviction if such caches exist.

A one-time warm-up increase is acceptable if later equivalent batches plateau.

Unexplained monotonic growth requires investigation.

Do not force working-set trimming or benchmark-only cache purges.

## 8. Mock provider and parser fixtures

The normative provider contract is:

- docs/MOCK_PROVIDER_CONTRACT.md

All candidates use the same executable and frozen manifest.

The benchmark includes:

- normal deterministic streaming
- fragmented/coalesced physical writes
- direct decoder-fragment vectors
- UTF-8 boundary split
- provider-initiated client request
- cooperative cancellation barrier
- concurrent stderr pressure
- backpressure
- maximum valid frame
- oversized frame with fixed recovery
- unexpected child exit
- explicit shutdown/cleanup

Provider write fragmentation alone is not evidence of fragmented reads.

The direct decoder-fragment vectors must feed exact byte fragments to the candidate framing/UTF-8 decoder and produce evidence independent of OS pipe read boundaries.

The mock remains the comparative performance workload.

## 9. UI fixture

Use equivalent:

- mascot source image
- logical mascot dimensions
- composer dimensions
- response-view dimensions
- font configuration from the frozen fixture
- fixed four-message prior-history fixture
- animation policy
- display configuration
- focus/caret state for each measurement

Do not make one candidate visually or behaviorally simpler to improve its numbers.

## 10. Text correctness

Correctness eligibility is defined by:

- docs/ACCEPTANCE_MATRIX.md
- docs/TEXT_FIXTURES.md

Required cases must be PASS.

UNTESTED is not passing.

Use the same named Windows CJK IME and frozen text/action fixture for all candidates.

Performance measurements may be collected for a failing candidate as diagnostic data, but they are not eligible for final comparison until required correctness is restored.

## 11. Sampling and repetitions

Initial minimums:

| Measurement | Minimum |
|---|---:|
| Fresh-process warm-cache launches | 30 per candidate |
| Post-reboot first launch | 3 per candidate |
| First composer activation | 10 process lifetimes per candidate |
| Warm activation | 100 per candidate across >=3 process lifetimes |
| Resource steady-state scenarios | 3 independent runs |
| Stability | 3 successive batches of 100 operations |

Increase sample counts only when uncertainty could plausibly change the decision.

Report per-run distributions rather than only pooled values.

Use balanced ordering across Rust, Zig, and Go rather than always running them in the same sequence.

## 12. Peak handling

For active scenarios:

- sample aggregate application totals at a fixed 100 ms interval
- record sampled observed maxima
- record OS-provided high-water counters where available
- label sampled maximum as "observed maximum"
- do not sum unrelated historical per-process peaks and call that an aggregate peak

For multi-process candidate architectures, aggregate simultaneous samples and retain per-process rows.

## 13. Candidate-specific runtime diagnostics

These diagnostics explain results; they do not replace headline OS metrics.

### Go

Record where available:

- HeapAlloc: allocated Go heap, including objects not yet reclaimed
- optional marked-live heap snapshot from the corresponding runtime metric, clearly labeled as the previous GC's marked-live snapshot
- HeapSys
- HeapInuse
- GC cycle count
- cumulative GC pause time
- goroutine count
- GOMAXPROCS
- GOGC
- GOMEMLIMIT
- any other non-default Go runtime controls
- cgo usage and major native allocations if identifiable

Interpretation rules:

- HeapSys is not resident memory.
- HeapAlloc is not synonymous with currently reachable/live heap.
- cumulative GC pause time is not total GC CPU cost; concurrent marking, assists, scavenging, and other runtime work also consume resources.
- GOMEMLIMIT is a soft limit on Go-runtime-managed memory, not a cap on total process memory, cgo/native GUI allocations, or GPU resources.
- Do not subtract Go runtime/GC memory from process memory or add it again on top of OS totals.
- Avoid intrusive runtime sampling during headline runs; use low-intrusion snapshots or separate diagnostic passes when necessary.

### Rust / Zig

If allocator/runtime diagnostics are cheaply available, report them as supplemental data with the same rule: do not subtract them from process totals.

## 14. Build/dependency report

For each implementation record:

- compiler/toolchain version
- exact release-build command
- allocator choice
- runtime/GC settings where applicable
- runtime-safety settings
- LTO settings
- stripping settings
- enabled dependency features
- direct dependencies and purpose
- notable transitive/native dependencies
- required DLL/framework/runtime files
- stripped executable size
- packaged/runtime payload size if meaningful
- clean build time
- incremental no-op build time
- incremental one-file-change build time

Do not compare a sanitizer/debug allocator build against a normal release build.

Build time is informative, not the primary product metric.

## 15. Engineering-cost report

Record factual evidence:

- handwritten LOC by language
- platform-specific LOC
- unsafe/FFI/native bridge LOC where meaningful
- direct dependency count
- project-owned native infrastructure
- patches/forks/workarounds
- known input/window limitations
- debugging friction encountered
- pinned toolchain/version constraints
- helper-process inventory

LOC and dependency counts are descriptive, not scores.

Generated bindings do not count as handwritten LOC.

A mature library doing substantial work is a legitimate engineering benefit, not unfair outsourcing.

## 16. Real-provider compatibility gate

Before final selection, every surviving candidate runs one untimed Codex app-server smoke test:

- pinned app-server version
- stdio launch
- initialization
- one read-only interaction
- streamed output
- interrupt or normal completion
- clean process/pipe teardown

Do not use model/network latency as comparative benchmark data.

Do not exclude candidate adapter memory merely because the external Codex process itself is excluded.

Devin ACP is not required before final selection unless a specific uncovered ACP obligation is identified.

## 17. Agent/source efficiency

A separate normative protocol measures source-code/token economy and agent implementation friction:

- docs/AGENT_SOURCE_EFFICIENCY.md

This comparison is required for the Windows report after all correctness-eligible candidates are implemented.

At minimum record:

- handwritten source bytes
- nonblank/noncomment LOC
- frozen-tokenizer source token count
- source tokens by major subsystem
- first-complete versus correctness-ready token totals
- focused correction token churn
- structural stack changes
- build/debug friction
- native/FFI bridge surface

The primary LLM-context proxy uses the tokenizer frozen by that protocol. It is a model-context proxy, not a universal property of the language.

Do not let token counting change candidate architecture, formatting, or correctness requirements.

## 18. Result table

Fill only after candidate correctness eligibility is known.

| Metric | Rust | Zig | Go | Notes |
|---|---:|---:|---:|---|
| Correctness eligibility | TBD | TBD | TBD | PASS only if all required cases pass |
| Required UNTESTED cases | TBD | TBD | TBD | |
| Fresh startup median | TBD | TBD | TBD | warm OS cache |
| Fresh startup range | TBD | TBD | TBD | |
| Post-reboot first launch median/range | TBD | TBD | TBD | raw values retained |
| First composer activation median | TBD | TBD | TBD | |
| Warm hotkey p95 | TBD | TBD | TBD | |
| Fresh mascot private working set | TBD | TBD | TBD | |
| Warm mascot private working set | TBD | TBD | TBD | after use |
| Warm mascot private commit | TBD | TBD | TBD | |
| Composer private working set | TBD | TBD | TBD | |
| Composer private commit | TBD | TBD | TBD | |
| Streaming observed peak PWS | TBD | TBD | TBD | |
| Streaming observed peak commit | TBD | TBD | TBD | |
| Post-open/close batch delta | TBD | TBD | TBD | warmed baseline |
| Post-stream batch delta | TBD | TBD | TBD | warmed baseline |
| Post-cancel batch delta | TBD | TBD | TBD | warmed baseline |
| Idle one-core CPU % | TBD | TBD | TBD | |
| Idle redraw/present count | TBD | TBD | TBD | |
| Wakeup diagnostic | TBD | TBD | TBD | source named |
| Threads | TBD | TBD | TBD | |
| Kernel handles | TBD | TBD | TBD | |
| USER objects | TBD | TBD | TBD | |
| GDI objects | TBD | TBD | TBD | |
| Live child cleanup | TBD | TBD | TBD | |
| Stripped executable size | TBD | TBD | TBD | |
| Runtime payload size | TBD | TBD | TBD | |
| Handwritten LOC | TBD | TBD | TBD | |
| Platform-specific LOC | TBD | TBD | TBD | |
| Project-owned workarounds | TBD | TBD | TBD | |
| Handwritten source tokens | TBD | TBD | TBD | frozen tokenizer |
| Focused-correction token churn | TBD | TBD | TBD | candidate-owned source |
| Structural stack changes | TBD | TBD | TBD | |

Do not reduce the decision to a single weighted score. Runtime results and agent/source-efficiency results must remain separately visible before any overall engineering conclusion.
