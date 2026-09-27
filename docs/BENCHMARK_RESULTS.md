# Benchmark Results — Windows Stage A

Fixture: `windows-v1.0.1` (commit-frozen). All evidence under `benchmark/results/windows/raw/`.

## Correctness eligibility

| Case | Rust | Zig | Go |
|---|---|---|---|
| T1 | PASS | - | PASS |
| T2 | PASS | - | PASS |
| T3 | PASS | - | PASS |
| T4 | PASS | - | PASS |
| T5 | PASS | - | PASS |
| T6 | PASS | - | PASS |
| T7 | PASS | - | PASS |
| T8 | PASS | - | PASS |
| T9 | PASS | - | PASS |
| T10 | PASS | - | PASS |
| T11 | PASS | - | PASS |
| T12 | PASS | - | PASS |
| T13 | PASS | - | PASS |
| T14 | PASS | - | PASS |
| W1 | PASS | - | PASS |
| W2 | UNTESTED | - | UNTESTED |
| W3 | PASS | - | PASS |
| W4 | PASS | - | PASS |
| W5 | PASS | - | PASS |
| W6 | PASS | - | PASS |
| W7 | UNTESTED | - | UNTESTED |
| W8 | PASS | - | PASS |
| W9 | PASS | - | PASS |

Provider/lifecycle (regression status per candidate):

| Candidate | Provider regression | Smoke |
|---|---|---|
| rust | PASS_PROVIDER_REGRESSION_ONLY | PASS_SMOKE_ONLY |
| zig | - | PASS_SMOKE_ONLY |
| go | PASS_PROVIDER_REGRESSION_ONLY | PASS_SMOKE_ONLY |

## Source/token efficiency (tiktoken/o200k_base)

| Metric | Rust | Zig | Go |
|---|---:|---:|---:|
| Final source tokens | 32933 | - | - |
| Nonblank/noncomment LOC | 4355 | - | - |
| Source files | [{'file': 'src/config.rs', 'bytes': 7184, 'chars': 7184, 'loc_nonblank_noncomment': 213, 'tokens': 1830, 'tokens_code_only': 1830, 'subsystem': 'other_candidate_logic', 'functions': 7, 'ffi_boundary_lines': 2}, {'file': 'src/control.rs', 'bytes': 7600, 'chars': 7600, 'loc_nonblank_noncomment': 223, 'tokens': 1649, 'tokens_code_only': 1649, 'subsystem': 'benchmark_candidate_hooks', 'functions': 8, 'ffi_boundary_lines': 6}, {'file': 'src/framing.rs', 'bytes': 5137, 'chars': 5137, 'loc_nonblank_noncomment': 179, 'tokens': 1141, 'tokens_code_only': 1141, 'subsystem': 'other_candidate_logic', 'functions': 2, 'ffi_boundary_lines': 0}, {'file': 'src/main.rs', 'bytes': 8392, 'chars': 8392, 'loc_nonblank_noncomment': 238, 'tokens': 1988, 'tokens_code_only': 1988, 'subsystem': 'other_candidate_logic', 'functions': 4, 'ffi_boundary_lines': 11}, {'file': 'src/platform.rs', 'bytes': 54163, 'chars': 54159, 'loc_nonblank_noncomment': 1506, 'tokens': 11462, 'tokens_code_only': 11462, 'subsystem': 'windowing_platform_glue', 'functions': 15, 'ffi_boundary_lines': 40}, {'file': 'src/provider.rs', 'bytes': 54889, 'chars': 54889, 'loc_nonblank_noncomment': 1566, 'tokens': 11403, 'tokens_code_only': 11403, 'subsystem': 'provider_process_io', 'functions': 19, 'ffi_boundary_lines': 7}, {'file': 'src/queue.rs', 'bytes': 4567, 'chars': 4567, 'loc_nonblank_noncomment': 143, 'tokens': 1087, 'tokens_code_only': 1087, 'subsystem': 'other_candidate_logic', 'functions': 2, 'ffi_boundary_lines': 0}, {'file': 'src/text.rs', 'bytes': 8813, 'chars': 8813, 'loc_nonblank_noncomment': 287, 'tokens': 2373, 'tokens_code_only': 2373, 'subsystem': 'text_ime_rendering', 'functions': 14, 'ffi_boundary_lines': 36}] | - | - |
| First-complete tokens | 28463 | - | - |
| Correction tokens added | 14430 | - | - |
| Correction tokens removed | 9983 | - | - |

## Benchmark metrics (see raw dirs for distributions)

| Metric | Rust | Zig | Go |
|---|---:|---:|---:|
| Fresh startup median ms | - | - | - |
| First activation ms | - | - | - |
| Warm activation median ms | - | - | - |
| Fresh mascot PWS (bytes) | - | - | - |
| Warm mascot PWS (bytes) | - | - | - |
| Streaming observed peak PWS | - | - | - |
| Idle CPU % of one core | - | - | - |
| R6 composer paints delta | - | - | - |

Raw per-run samples are in each candidate's result.json; this report deliberately avoids a composite score and does not declare a cross-platform language winner.
