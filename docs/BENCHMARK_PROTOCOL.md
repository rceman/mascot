# Benchmark Protocol v0.1

This protocol exists to keep the Rust and Zig prototype comparison honest and repeatable.

## 1. General rules

- Benchmark release/optimized builds only.
- Do not benchmark with debugger/profiler attached unless the scenario explicitly requires it.
- Reboot or otherwise return the machine to a reasonably stable baseline before final comparison runs.
- Record OS build, CPU, RAM, display topology/scaling, compiler/toolchain versions, and commit SHA.
- Run both candidates on the same machine under the same display configuration.
- External provider/mock-child memory is reported separately.
- Report raw measurements; do not report only a composite score.

## 2. Required Windows metrics

For each scenario capture, where practical:

- process private working set
- private bytes / commit
- total working set
- handle count
- thread count
- CPU utilization
- startup duration
- hotkey-to-visible latency
- response-event-to-paint latency
- executable/package size

If GPU resources are used, also record process-associated GPU memory where tooling exposes it.

## 3. Timing definitions

### Cold start

From process launch request to the first fully presented mascot frame.

Run at least 10 times. Report:

- min
- median
- p95
- max

### Warm activation

With the mascot process already settled and idle:

global hotkey event -> composer visibly presented and ready for input.

Run at least 100 times. Report median and p95.

### Stream presentation

Mock provider timestamps each emitted chunk.

The shell records when the corresponding content becomes eligible for or completes presentation.

Use a monotonic clock within each process. If cross-process timestamps are compared, either use one measurement owner or explicitly account for clock source.

## 4. Memory scenarios

Allow the process to settle before steady-state readings.

### M0 — mascot idle

- launch
- wait 60 seconds
- no pointer movement over the mascot
- record memory/CPU/resource counts

### M1 — composer open

- open composer
- populate with representative short history fixture
- wait 60 seconds
- record

### M2 — 100 open/close cycles

- open composer
- close composer
- repeat 100 times
- return to mascot-only state
- wait 60 seconds
- record
- compare to M0

### M3 — 100 streamed responses

Each iteration:

- submit prompt
- receive deterministic mock response
- complete rendering
- clear/discard transient response state according to prototype rules

After 100 iterations:

- return to stable state
- wait 60 seconds
- record

### M4 — 100 cancellation cycles

- start streamed response
- cancel at deterministic chunk
- repeat 100 times
- settle and record

### M5 — long idle

- composer open
- no interaction for 10 minutes
- record CPU/wakeups and before/after memory

## 5. Mock provider fixture

Both implementations must consume the same logical fixture.

Suggested behavior:

- child process over stdin/stdout
- one request line with an ID and UTF-8 prompt
- deterministic response
- 100 chunks
- fixed inter-chunk delay
- stderr emits a small deterministic diagnostic stream
- supports cancellation
- exits with known codes for explicit failure tests

Do not involve network access or a real model.

The wire format may be a tiny line-delimited JSON protocol if both implementations use the exact same fixture.

## 6. UI fixture

Use equivalent:

- mascot source image
- mascot display dimensions
- composer dimensions
- font size
- response content
- history fixture
- animation policy
- monitor/scaling configuration

Do not make one version visually simpler to obtain a memory advantage.

## 7. Text fixture

At minimum exercise:

```text
English: The quick brown fox jumps over the lazy dog.
Latvian: Ārā līst, bet mēs turpinām darbu.
Cyrillic: Проверка ввода и выделения текста.
Combining: é å
Emoji: 👨‍💻 🧑🏽‍🚀 ❤️‍🔥
RTL mixed sample: English العربية English
```

Also manually validate:

- dead keys
- selection
- caret movement
- backspace/delete around multi-codepoint text
- clipboard
- multiline editing
- IME composition where available

If exact behavior differs, document it. Do not silently relax requirements for one candidate.

## 8. Stability checks

Run repeated cycles and record whether these plateau:

- RSS/private memory
- handles
- threads
- native window objects
- renderer resources
- decoded image buffers
- glyph/text caches

A stable cache warming increase is acceptable if it reaches a bounded plateau.

Monotonic growth without an understood bound is a failure requiring investigation.

## 9. Build/dependency report

For each implementation record:

- compiler version
- exact release-build command
- direct dependencies
- notable transitive/native dependencies
- stripped executable size
- packaged size if packaging is tested
- clean build time
- incremental no-op build time
- incremental one-file-change build time

Build time is informative, not the primary product metric.

## 10. Engineering-cost report

The implementation agent must provide factual counts/notes rather than a subjective winner:

- handwritten LOC by language
- platform-specific LOC
- unsafe/FFI/native bridge LOC
- number of direct dependencies
- patches/forks/workarounds
- known text/input limitations
- known windowing limitations
- debugging friction encountered
- toolchain/version constraints

## 11. Result table

Fill this only after both prototypes exist.

| Metric | Rust | Zig | Notes |
|---|---:|---:|---|
| Cold start median | TBD | TBD | |
| Cold start p95 | TBD | TBD | |
| Warm hotkey p95 | TBD | TBD | |
| Mascot idle private memory | TBD | TBD | |
| Composer open private memory | TBD | TBD | |
| Post-100 open/close delta | TBD | TBD | |
| Post-100 stream delta | TBD | TBD | |
| Idle CPU | TBD | TBD | |
| Threads | TBD | TBD | |
| Handles | TBD | TBD | |
| Stripped executable size | TBD | TBD | |
| Handwritten LOC | TBD | TBD | |
| Platform-specific LOC | TBD | TBD | |

Do not reduce the decision to a single weighted score before reviewing the raw data.
