# Astra Review Prompt

Review the current planning branch as a hostile architecture reviewer before implementation begins.

Read all of these first:

- README.md
- docs/PROTOTYPE_PLAN.md
- docs/BENCHMARK_PROTOCOL.md
- docs/ACCEPTANCE_MATRIX.md
- docs/MOCK_PROVIDER_CONTRACT.md
- rust/README.md
- zig/README.md

## Context

We are comparing Rust and Zig as foundations for an ultra-light native desktop AI-agent shell.

The eventual product may expose a floating mascot/bubble, text/voice interaction, desktop context, external agent providers such as Codex app-server and Devin ACP, browser/computer-use integration, cloud sync, and much later RepoSuite structured memory.

For this benchmark we intentionally want the smallest vertical slice that exposes dangerous assumptions without turning into two mini IDEs.

Do not redesign the whole future product unless the benchmark still cannot answer the intended language/stack question.

## Review goals

Critique the revised plan for:

1. fairness between Rust and Zig
2. hidden bias toward one ecosystem
3. whether the shared correctness contract is operational enough
4. whether the mock provider contract is sufficiently frozen
5. missing platform risks
6. memory/accounting methodology
7. latency/presentation methodology
8. idle CPU/redraw/wakeup methodology
9. resource-growth/leak methodology
10. text-input/Unicode/IME blind spots
11. transparency/hit-testing/windowing blind spots
12. dependency choices that could invalidate the comparison
13. scenarios that add noise without decision value
14. acceptance thresholds that are unrealistic or misleading
15. whether Windows screening + mandatory macOS validation is sufficient
16. whether the Linux feasibility gate is strong enough
17. whether the untimed Codex compatibility gate is sufficient before final selection
18. whether anything still allows a candidate to obtain an artificial advantage

## Important constraints

- No Electron, Chromium, WebView, Tauri frontend, or Node runtime.
- Low RAM and low interaction latency are first-class product requirements.
- Platform-native shims are allowed.
- Pure-language dependency stacks are not required.
- Dependency symmetry is not required.
- CPU/GPU/native-widget/custom-rendered approaches are all allowed if they meet the same correctness/accounting contract.
- Do not prefer Rust merely because its ecosystem is larger.
- Do not prefer Zig merely because its theoretical runtime floor is smaller.
- A few MiB of memory difference is not automatically worth substantially more infrastructure.
- The prototype must remain small enough that two implementations are affordable.
- Distinguish blockers to a valid experiment from optional product improvements.
- Do not require additional product subsystems without demonstrating decision value.
- An UNTESTED required capability does not count as passing.

## Desired output

### A. Blocking issues

Only issues that must be fixed before implementation.

For each:

- affected file/section
- why it matters
- exact correction

If there are no blockers, explicitly say so.

### B. Non-blocking improvements

Useful changes that improve the experiment but are not required before coding.

### C. Fairness audit

State whether the revised experiment now provides a fair Rust-vs-Zig comparison.

Identify any remaining artificial-win paths.

### D. Benchmark audit

Review only measurements that could realistically change the language decision:

- memory
- commit/resident accounting
- peaks
- latency
- presentation timing
- CPU/redraw/wakeups
- repetitions/order
- stability/resource growth

### E. Scope audit

Identify anything still too large, too small, missing, or unnecessary.

### F. Platform-decision audit

Answer explicitly:

- Is Windows appropriate for screening?
- Is mandatory macOS validation sufficient before final selection?
- Is the Linux/X11/Wayland feasibility gate sufficient?

### G. Provider-integration audit

Answer explicitly:

- Is the revised structured mock sufficient for comparative benchmarking?
- Is the untimed Codex app-server gate sufficient before final language selection?
- Is there any decision-critical reason to add Devin ACP now?

### H. Approved minimum prototype

Give the exact minimum scope you would approve.

### I. Proposed document changes

Give only concrete edits still required or strongly recommended.

### J. Final review status

End with exactly one of:

- PLAN_APPROVED
- PLAN_APPROVED_WITH_NON_BLOCKING_CHANGES
- PLAN_NEEDS_REVISION

Do not choose Rust or Zig in this review.
