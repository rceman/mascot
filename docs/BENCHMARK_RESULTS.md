# Benchmark Results — Windows Stage A

Fixture: `windows-v1.0.2` (commit-frozen). All evidence under `benchmark/results/windows/raw/`.

## Correctness eligibility

| Case | Rust | Zig | Go |
|---|---|---|---|
| T1 | PASS | PASS | PASS |
| T2 | PASS | PASS | PASS |
| T3 | PASS | PASS | PASS |
| T4 | PASS | PASS | PASS |
| T5 | PASS | PASS | PASS |
| T6 | PASS | PASS | PASS |
| T7 | PASS | PASS | PASS |
| T8 | PASS | PASS | PASS |
| T9 | PASS | PASS | PASS |
| T10 | PASS | PASS | PASS |
| T11 | PASS | PASS | PASS |
| T12 | PASS | PASS | PASS |
| T13 | PASS | PASS | PASS |
| T14 | PASS | PASS | PASS |
| W1 | PASS | PASS | PASS |
| W2 | PASS | PASS | PASS |
| W3 | PASS | PASS | PASS |
| W4 | PASS | PASS | PASS |
| W5 | PASS | PASS | PASS |
| W6 | PASS | PASS | PASS |
| W7 | PASS | PASS | PASS |
| W8 | PASS | PASS | PASS |
| W9 | PASS | PASS | PASS |

Provider/lifecycle (regression status per candidate):

| Candidate | Provider regression | Smoke | Codex gate |
|---|---|---|---|
| rust | PASS_PROVIDER_REGRESSION_ONLY | PASS_SMOKE_ONLY | PASS |
| zig | PASS_PROVIDER_REGRESSION_ONLY | PASS_SMOKE_ONLY | PASS |
| go | PASS_PROVIDER_REGRESSION_ONLY | PASS_SMOKE_ONLY | PASS |

## Source/token efficiency (tiktoken/o200k_base)

| Metric | Rust | Zig | Go |
|---|---:|---:|---:|
| Final source tokens | 34801 | 43905 | 36903 |
| Nonblank/noncomment LOC | 4528 | 4123 | 4187 |
| Source files | 9 | 11 | 15 |
| First-complete tokens | 28463 | 41848 | 35378 |
| Correction tokens added | 16520 | 2057 | 1822 |
| Correction tokens removed | 10207 | 0 | 296 |

## Benchmark metrics (see raw dirs for distributions)

| Metric | Rust | Zig | Go |
|---|---:|---:|---:|
| Fresh startup median ms | 93.77 | 94.1 | 93.72 |
| First activation median ms | 858.31 | 877.35 | 846.21 |
| Warm activation median ms | 155.68 | 155.54 | 155.18 |
| Warm activation p95 ms | 162.96 | 160.33 | 159.38 |
| Fresh mascot PWS (bytes) | 7323648 | 9637888 | 16936960 |
| Warm mascot PWS (bytes) | 8466432 | 11395072 | 14331904 |
| Streaming observed peak PWS (bytes) | 8429568 | 11276288 | 13869056 |
| Streaming observed peak commit (bytes) | 9220096 | 99987456 | 26173440 |
| Chunk transport median ms | 0.03 | 0.02 | 0.03 |
| Submit->first visible ms | 189.61 | 190.63 | 195.72 |
| Idle CPU % of one core | 0.0 | 0.0 | 0.11 |
| R6 composer paints delta | 0 | 0 | 0 |
| Stability PWS growth op->bytes | {'open_close': 20480, 'submit_complete': 45056, 'cancel': 36864} | {'open_close': 385024, 'submit_complete': 5091328, 'cancel': 3092480} | {'open_close': 2994176, 'submit_complete': 909312, 'cancel': 36864} |

Notes:

- Stability PWS growth is the per-operation first->last sample delta (private working set, bytes) over 100 ops, worst of 3 batches; positive values indicate per-operation retention, not necessarily leaks (allocator caching included).
- Zig's streaming peak commit (~100 MB) is a pre-reserved address/commit region that is stable and unchanged through the R5 60 s decay window and later states; its resident PWS stays ~11 MB.
- Process inventories per lifetime contain exactly the candidate and the excluded shared provider child; no stray helper processes were observed in the benchmark-003 run.
- The Codex app-server gate is untimed compatibility only (codex 0.80.0 over stdio): initialize + config/read + thread/list + clean teardown; no model/network metrics.
- Devin ACP remains deferred; no uncovered ACP-specific architectural obligation was identified during this stage.
- Post-reboot first launches are **UNTESTED** — they require coordinated genuine boots and no automatic reboot was performed.

Raw per-run samples are in each candidate's result.json; this report deliberately avoids a composite score and does not declare a cross-platform language winner.
