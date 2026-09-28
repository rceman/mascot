# Agent Handoff — Complete Source/Agent Efficiency Evidence

Task ID: MASCOT-SOURCE-EFFICIENCY-COMPLETE-001

## Objective

Complete the missing Phase 11 deliverables for the finished Windows Rust/Zig/Go comparison without changing candidate product behavior and without rerunning the long runtime benchmark campaign unless a source-metric requirement genuinely cannot be satisfied otherwise.

Read:

- docs/AGENT_SOURCE_EFFICIENCY.md
- docs/BENCHMARK_PROTOCOL.md
- docs/AGENT_HANDOFF_WINDOWS_COMPARISON.md
- docs/BENCHMARK_RESULTS.md

Use the already committed final correctness-ready candidate sources and preserved Git history.

## Required outputs

Create:

    benchmark/results/windows/source-efficiency.md
    benchmark/results/windows/source-efficiency.json

Update:

    docs/BENCHMARK_RESULTS.md

with the complete source/agent-efficiency table and references to the detailed files.

## Required source/token evidence

For Rust, Zig, and Go report all obtainable required fields from docs/AGENT_SOURCE_EFFICIENCY.md, including:

- handwritten source bytes
- handwritten characters
- nonblank/noncomment LOC
- source file count
- median/largest source file
- source tokens using frozen tiktoken/o200k_base
- tokens/LOC
- tokens/file
- subsystem token counts
- normalized same-function token counts where boundaries are defensible
- first-complete source tokens
- final correctness-ready source tokens
- correction token additions/removals/churn
- correction commits
- structural stack/dependency changes
- native/FFI bridge boundaries
- platform-specific source files
- project-owned workarounds
- dependency counts
- candidate-specific build/config size

If a metric cannot be reconstructed reliably, mark it explicitly UNAVAILABLE with the reason. Do not invent data.

## Build timing evidence

Measure on the same Windows machine and final source state:

- clean release build
- incremental no-op build
- incremental one-file-change build

Record exact command, toolchain, cache state assumptions, sample count, median, and range.

Do not deliberately change source architecture or build flags to improve these numbers.

## Test execution timing evidence

Also measure the development-feedback side, separately from the long benchmark campaign.

For each candidate, where the corresponding command exists, record:

- candidate unit/self-test execution time
- provider regression execution time
- smoke-test execution time
- full acceptance-suite execution time excluding the long runtime benchmark
- Codex compatibility-gate execution time
- one representative edit -> incremental build -> targeted test loop time

Timing rules:

- use the final correctness-ready source/binary configuration
- same machine/power mode for all candidates
- use the same fixture version
- at least 5 repetitions for short tests when practical
- report median and range, not only one sample
- distinguish cold/first-run from warm repeated results when material
- report shared harness/provider overhead separately where measurable
- do not include benchmark-003 or other long performance campaigns in "test execution time"

The purpose is to estimate how quickly an AI coding agent can iterate:

    edit -> build -> test -> observe failure/success -> next edit

Do not infer model token usage from elapsed time.

## History and fairness

Do not squash or rewrite the existing candidate history used for correction/rework evidence.

Do not modify candidate implementations merely to improve source-token/build/test-time metrics.

If a measurement exposes a real correctness defect, report it before making any source correction.

## Final report

Return:

1. final branch and HEAD
2. source-efficiency artifact paths
3. complete Rust/Zig/Go source/token table
4. build-time table
5. test-execution-time table
6. edit-build-test feedback-loop table
7. unavailable fields and reasons
8. files/commits changed
9. clean-worktree confirmation

End with exactly:

    SOURCE_EFFICIENCY_COMPLETE

or:

    SOURCE_EFFICIENCY_BLOCKED: <reason>
