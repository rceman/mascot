# Benchmark Protocol v0.2

This protocol keeps the Rust and Zig comparison repeatable and prevents artificial wins from different accounting or lifecycle choices.

## 1. General rules

- Benchmark release/optimized builds only.
- Do not attach debugger/profiler during headline measurements.
- Run native Windows executables for Stage A; do not benchmark via WSLg.
- Benchmark both candidates from equivalent Windows-native deployment locations.
- Record OS build, CPU, RAM, power mode, display topology/scaling, refresh rate, compiler/toolchain versions, and commit SHA.
- Run both candidates on the same machine under the same display configuration.
- Alternate candidate order in balanced A/B blocks.
- Report raw per-run data and distributions; do not report only a composite score.
- The common provider fixture and measurement tools are excluded from application totals. Candidate-specific helpers are not excluded.

## 2. Application-owned process inventory

Before every benchmark run, record all application-owned processes with:

- PID
- role
- parent PID
- whether counted in aggregate application metrics

Only these may be excluded:

- the byte-identical shared mock-provider fixture
- measurement/instrumentation processes

Any candidate-specific worker, helper, renderer service, bridge, or broker is part of that candidate's application cost.

## 3. Required Windows metrics

Mandatory where the OS exposes them:

- private working set
- private bytes / private commit
- total working set
- process/thread count
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

The existing 20/50/80 MiB target bands refer to **aggregate application-owned private working set on Windows**.

Private commit and active peaks are mandatory companion metrics and must never be collapsed into the same column.

## 4. Timing definitions

### 4.1 Fresh-process startup with warm OS caches

Start time:

- external benchmark harness issues the process launch request

End time:

- first mascot frame is visibly presented

Run at least 30 times per candidate initially.

Report:

- every raw sample
- median
- range
- p95 only if the sample count and distribution make it useful

### 4.2 Post-reboot first launch

This is distinct from ordinary fresh-process startup.

Run at least 3 post-reboot observations per candidate initially.

Alternate which candidate receives the actual first launch after boot.

Report raw values and median/range. Do not present a small-sample p95 as statistically strong.

### 4.3 First composer activation

In a newly launched process that has not opened the composer yet:

global hotkey -> composer visibly presented and ready for input.

Measure separately from subsequent warm activations.

### 4.4 Warm activation

With the process settled and the composer path already exercised:

global hotkey -> composer visibly presented and ready for input.

Run at least 100 activations distributed across at least 3 process lifetimes.

Report median, p95, range, and per-process distribution.

### 4.5 Stream presentation

For each logical response chunk, record:

1. mock emission timestamp
2. complete logical-frame receipt timestamp before JSON decoding
3. first presentation containing that chunk

Multiple chunks may share one presentation.

Do not require one redraw per chunk.

Do not label a redraw request, queue submission, or presentation API return as "visible" unless the observation method actually establishes visibility.

### 4.6 Clock and observation contract

For Stage A, use a documented Windows high-resolution clock strategy compatible with QPC-derived timestamps.

Do not assume language-specific monotonic-clock epochs are interchangeable across processes.

Use one of:

- a common external measurement owner, or
- shared QPC-derived timestamp representation with documented conversion

Document the visual/presentation observation method and its timing resolution.

If only application-side submission is available for a diagnostic run, label it as submission latency, not visible-presentation latency.

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

Purpose: startup footprint before text/provider paths have been exercised.

### R1 — first composer open

- from fresh process, open composer for the first time
- measure active peak
- settle 60 s
- collect 30 s steady-state window

Purpose: expose lazy font/input/toolkit initialization.

### R2 — warm composer open

- composer path already exercised
- open composer
- settle
- collect

### R3 — warm mascot-only after use

- exercise composer and provider path
- hide composer
- sample approximately 1 s, 10 s, and 60 s after hide
- collect 30 s steady-state window after 60 s

Purpose: capture what an all-day resident mascot actually retains.

### R4 — normal streaming

- execute canonical mock response
- sample at fixed 100 ms interval
- retain observed active maxima
- also record OS high-water values where exposed

### R5 — cancellation

- execute canonical cancellation case
- capture active peak
- sample post-operation at 1 s, 10 s, 60 s

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

A one-time warm-up increase is acceptable if later equivalent batches plateau.

Unexplained monotonic growth requires investigation.

Do not force working-set trimming or benchmark-only cache purges.

## 8. Mock provider fixture

The fixture contract is normative:

- docs/MOCK_PROVIDER_CONTRACT.md

Both candidates use the same executable and frozen manifest.

The benchmark must include:

- normal deterministic streaming
- fragmented/coalesced physical reads
- UTF-8 boundary split
- provider-initiated client request
- cooperative cancellation
- concurrent stderr pressure
- backpressure case
- maximum valid frame
- oversized frame
- unexpected child exit
- explicit shutdown/cleanup

The mock remains the comparative performance workload.

## 9. UI fixture

Use equivalent:

- mascot source image
- logical mascot dimensions
- composer dimensions
- font size
- response-area dimensions
- four-message fixed prior-history fixture
- animation policy
- monitor/scaling configuration
- focus/caret state for each measurement

Do not make one candidate visually or behaviorally simpler to improve its numbers.

## 10. Text correctness

Correctness eligibility is defined by:

- docs/ACCEPTANCE_MATRIX.md

Required cases must be PASS.

UNTESTED is not passing.

Use the same named Windows CJK IME for both candidates.

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

## 12. Peak handling

For active scenarios:

- sample application totals at a fixed 100 ms interval
- record sampled observed maxima
- record OS-provided high-water counters where available
- label sampled maximum as "observed maximum"
- do not sum unrelated historical per-process peaks and call that an aggregate peak

For multi-process candidate architectures, aggregate simultaneous samples and also retain per-process rows.

## 13. Build/dependency report

For each implementation record:

- compiler/toolchain version
- exact release-build command
- allocator choice
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

## 14. Engineering-cost report

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

## 15. Real-provider compatibility gate

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

## 16. Result table

Fill only after candidate correctness eligibility is known.

| Metric | Rust | Zig | Notes |
|---|---:|---:|---|
| Correctness eligibility | TBD | TBD | PASS only if all required cases pass |
| Required UNTESTED cases | TBD | TBD | |
| Fresh startup median | TBD | TBD | warm OS cache |
| Fresh startup range | TBD | TBD | |
| Post-reboot first launch median/range | TBD | TBD | raw values retained |
| First composer activation median | TBD | TBD | |
| Warm hotkey p95 | TBD | TBD | |
| Fresh mascot private working set | TBD | TBD | |
| Warm mascot private working set | TBD | TBD | after use |
| Warm mascot private commit | TBD | TBD | |
| Composer private working set | TBD | TBD | |
| Composer private commit | TBD | TBD | |
| Streaming observed peak PWS | TBD | TBD | |
| Streaming observed peak commit | TBD | TBD | |
| Post-open/close batch delta | TBD | TBD | warmed baseline |
| Post-stream batch delta | TBD | TBD | warmed baseline |
| Post-cancel batch delta | TBD | TBD | warmed baseline |
| Idle one-core CPU % | TBD | TBD | |
| Idle redraw/present count | TBD | TBD | |
| Wakeup diagnostic | TBD | TBD | source named |
| Threads | TBD | TBD | |
| Kernel handles | TBD | TBD | |
| USER objects | TBD | TBD | |
| GDI objects | TBD | TBD | |
| Live child cleanup | TBD | TBD | |
| Stripped executable size | TBD | TBD | |
| Runtime payload size | TBD | TBD | |
| Handwritten LOC | TBD | TBD | |
| Platform-specific LOC | TBD | TBD | |
| Project-owned workarounds | TBD | TBD | |

Do not reduce the decision to a single weighted score.
