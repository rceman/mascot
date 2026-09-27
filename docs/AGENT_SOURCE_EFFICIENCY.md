# Agent/Source Efficiency Protocol v0.1

This protocol measures a separate question from runtime performance:

> For the same accepted product behavior, which candidate is more economical and reliable for an AI coding agent to understand, generate, modify, and maintain?

It is intentionally **not** reduced to one score.

Runtime performance and agent/source efficiency are reported side by side.

## 1. Scope

Measure only candidate-owned handwritten implementation material for:

- rust/
- zig/
- go/

Exclude:

- shared benchmark/
- shared assets/
- vendored dependencies
- generated bindings/code
- compiler/build outputs
- lockfiles
- raw benchmark results
- shared normative docs
- third-party source copied unchanged

Report exclusions explicitly.

## 2. Source-size metrics

For each correctness-eligible candidate, record:

- handwritten UTF-8 source bytes
- handwritten source characters
- nonblank/noncomment LOC
- handwritten source file count
- median source file size
- largest handwritten source file
- candidate-specific build/config bytes
- candidate-specific architecture/integration doc bytes separately

These are descriptive metrics, not scores.

## 3. LLM source-token metric

Freeze one primary tokenizer in the benchmark manifest:

    tokenizer: tiktoken/o200k_base

Use the exact same tokenizer implementation/version for Rust, Zig, and Go.

For every included handwritten source file:

1. read normalized UTF-8 file content exactly as committed
2. do not minify or reformat for token counting
3. encode with the frozen tokenizer
4. sum token counts

Report:

- total handwritten source tokens
- source tokens per nonblank/noncomment LOC
- source tokens per source file
- tokens by subsystem:
  - windowing/platform glue
  - text/IME/rendering
  - provider/process I/O
  - benchmark/candidate hooks
  - other candidate-owned logic

This token count is a **proxy for LLM context cost under the frozen tokenizer**. Do not claim it is identical to every current or future model's tokenizer.

If the tokenizer package cannot be installed or reproduced, do not substitute another tokenizer silently. Record the blocker or version a deliberate manifest change.

## 4. Normalized same-function comparison

Because all candidates implement the same acceptance matrix, whole-candidate token totals are directly useful.

Also identify a small set of homologous responsibilities and report their isolated handwritten token counts where boundaries are clear:

- create/show/hide transparent mascot window
- hit testing + dragging
- global hotkey
- composer input/IME bridge
- response rendering
- provider process launch
- framed stdout parser
- stderr drain
- cooperative cancellation
- clean child shutdown

Do not distort architecture merely to force one-to-one file/function symmetry.

If one language delegates a responsibility to a mature dependency, count only candidate-owned source but record the dependency as the implementation mechanism. Delegation to a mature library is a legitimate engineering advantage, not hidden zero-cost magic.

## 5. Agent rework / correction metrics

The same agent performs all three implementations under the same handoff.

Record factual per-candidate implementation/rework data where available:

- commits from first candidate implementation to correctness-ready state
- focused correction commits
- source tokens added in the first complete implementation snapshot
- final source tokens
- token delta between first complete snapshot and correctness-ready final state
- changed-source token volume during focused correction pass
- number of distinct build/test correction loops, if reliably logged
- number of structural stack/dependency changes
- number of candidate-specific benchmark exceptions requested: must be zero for final eligibility

Do not invent model-side token usage if the agent/runtime does not expose it.

If the agent platform exposes prompt/completion/cache token usage per implementation phase, record it as optional raw evidence, clearly separated from source-code token counts.

## 6. Build/debug friction

For each candidate record:

- clean build command
- incremental build command
- clean build time
- incremental one-file-change build time
- compiler/linker diagnostic quality notes based on concrete failures encountered
- native bridge/FFI setup steps
- dependency-resolution issues
- platform-specific debugging issues
- number of project-maintained workarounds

Narrative observations must cite concrete incidents from the implementation log/commits.

## 7. Readability / maintainability evidence

Do not assign subjective beauty scores.

Report concrete structural proxies:

- maximum function size
- median function size
- maximum nesting depth if a common tool can measure it reliably
- candidate-owned public/internal API surface count where meaningful
- number of unsafe/FFI/native bridge boundaries
- number of platform-specific source files
- dependency count
- project-owned adapter/shim LOC and source tokens

These are descriptive. They do not automatically make one language "better."

## 8. Token-churn rules

Token churn must be derived from committed candidate-owned source states.

Do not count:

- formatter-only changes when they can be isolated
- generated-code changes
- shared fixture changes
- benchmark result files
- documentation outside the candidate implementation

If a candidate changes stack after a structural failure, report both:

- abandoned candidate-owned token volume
- final candidate-owned token volume

This is important agent-effort evidence and must not be erased by squashing history.

## 9. Fairness rules

- Use the same agent/model configuration for all three candidate implementation phases when operationally possible.
- Use the same approved handoff and frozen fixture.
- Do not give one candidate extra design hints unavailable to the others after implementation begins.
- Candidate order must be recorded because the agent may learn from earlier implementations.
- To reduce learning-order bias, the final report must explicitly discuss whether later candidates benefited from already-solved product/benchmark questions.
- Do not intentionally write verbose or compressed code to influence token counts.
- Use normal idiomatic formatting and the candidate's chosen realistic architecture.
- Do not count comments/docs as source tokens unless they are required in the shipped candidate source; report code-only and code+inline-comment totals separately if practical.

## 10. Output table

Add a separate table to the Windows comparison report:

| Agent/source metric | Rust | Zig | Go |
|---|---:|---:|---:|
| Handwritten source bytes | TBD | TBD | TBD |
| Nonblank/noncomment LOC | TBD | TBD | TBD |
| Handwritten source tokens (o200k_base) | TBD | TBD | TBD |
| Tokens / LOC | TBD | TBD | TBD |
| Candidate source files | TBD | TBD | TBD |
| Platform-glue source tokens | TBD | TBD | TBD |
| Text/IME/rendering source tokens | TBD | TBD | TBD |
| Provider/process-I/O source tokens | TBD | TBD | TBD |
| First complete snapshot tokens | TBD | TBD | TBD |
| Final correctness-ready tokens | TBD | TBD | TBD |
| Focused-correction token churn | TBD | TBD | TBD |
| Focused correction commits | TBD | TBD | TBD |
| Structural stack changes | TBD | TBD | TBD |
| Clean build time | TBD | TBD | TBD |
| Incremental build time | TBD | TBD | TBD |
| Native/FFI bridge boundaries | TBD | TBD | TBD |
| Project-owned workarounds | TBD | TBD | TBD |

Do not collapse this table into a weighted "agent friendliness score" before inspecting raw evidence.

## 11. Interpretation

Useful conclusions may include factual statements such as:

- one candidate required materially fewer handwritten LLM tokens for the same accepted behavior
- one candidate needed substantially more correction churn
- one candidate delegated complex text/platform behavior to mature libraries with little project-owned glue
- one candidate had lower source token count but materially worse build/debug friction

Do not equate "fewest tokens" with "best language" automatically.

The final product-language decision considers runtime behavior, correctness, platform viability, engineering ownership, and agent/source efficiency together.
