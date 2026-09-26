# Astra Review Prompt

Please review the planning documents in this repository as a hostile architecture reviewer before implementation begins:

- `README.md`
- `docs/PROTOTYPE_PLAN.md`
- `docs/BENCHMARK_PROTOCOL.md`

Context:

We are comparing Rust and Zig as foundations for an ultra-light native desktop AI-agent shell. The eventual product may expose a floating mascot/bubble, text/voice interaction, desktop context, external agent providers such as Codex app-server and Devin ACP, browser/computer-use integration, cloud sync, and much later RepoSuite structured memory.

For this benchmark, however, we intentionally want the smallest vertical slice that exposes the dangerous assumptions without turning into two mini IDEs.

Please do NOT redesign the whole product unless the current benchmark cannot answer the intended language/stack question.

## Review goals

Critique the plan for:

1. fairness between Rust and Zig
2. hidden bias toward one ecosystem
3. missing platform risks
4. incorrect or weak memory metrics
5. incorrect latency methodology
6. text-input/Unicode/IME blind spots
7. transparency/windowing blind spots
8. dependency choices that could invalidate the comparison
9. benchmark scenarios that are too synthetic
10. benchmark scenarios that add noise without decision value
11. acceptance thresholds that are unrealistic or misleading
12. process/resource measurements we should add
13. stop conditions we should tighten
14. any reason Windows-first could produce a misleading language decision
15. whether mock-provider streaming is enough for this stage or whether one real provider integration is required before choosing a language

## Important constraints

- No Electron, Chromium, WebView, Tauri frontend, or Node runtime.
- Low RAM and low interaction latency are first-class product requirements.
- We are willing to use platform-native shims.
- We do not require a pure-language dependency stack.
- Do not prefer Rust merely because its ecosystem is larger.
- Do not prefer Zig merely because its theoretical runtime floor is smaller.
- A few MiB of memory difference is not automatically worth substantially more infrastructure.
- The prototype should remain small enough that two implementations are affordable.

## Desired output

Please return:

### A. Blocking issues

Only issues that should be fixed before implementation.

### B. Recommended plan edits

Concrete changes, preferably referencing exact sections.

### C. Benchmark fairness audit

State whether the proposed comparison would produce a technically meaningful Rust-vs-Zig result, and why.

### D. Missing measurements/tests

Only measurements that could realistically change the language decision.

### E. Proposed final prototype scope

Give the minimum scope you would approve after your review.

Do not choose the final language yet. The point of this phase is to validate the experiment, not to repeat the earlier language recommendation.
