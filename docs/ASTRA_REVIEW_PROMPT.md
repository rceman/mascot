# Astra Final Review Prompt

Perform a final hostile architecture review of the current planning branch before any candidate implementation begins.

Read all of these first:

- README.md
- docs/PROTOTYPE_PLAN.md
- docs/BENCHMARK_PROTOCOL.md
- docs/ACCEPTANCE_MATRIX.md
- docs/TEXT_FIXTURES.md
- docs/MOCK_PROVIDER_CONTRACT.md
- rust/README.md
- zig/README.md
- go/README.md

## Context

The benchmark now compares **Rust, Zig, and Go** as foundations for an ultra-light native desktop AI-agent shell.

The eventual product may expose a floating mascot/bubble, text/voice interaction, desktop context, external agent providers such as Codex app-server and Devin ACP, browser/computer-use integration, cloud sync, and much later RepoSuite structured memory.

For this benchmark we intentionally want the smallest vertical slice that exposes dangerous assumptions without turning into three mini IDEs.

The previous review returned PLAN_NEEDS_REVISION because:

1. text correctness could still pass with byte preservation but incorrect rendering/editing
2. canonical cancellation and exceptional provider-session recovery were not fully frozen

The current revision attempts to close both blockers and also adds **Go as a third candidate**.

Do not redesign the whole future product unless the benchmark still cannot answer the intended language/stack question.

## Review goals

Critique the current plan for:

1. whether the two previous blockers are now fully resolved
2. fairness among Rust, Zig, and Go
3. whether adding Go introduces any new artificial-win path
4. whether Go runtime/GC costs are accounted correctly without unfairly penalizing or excusing Go
5. whether shared text/visual fixtures are operational enough for composer and response rendering
6. whether the cancellation barrier and exceptional-session recovery are now deterministic
7. whether the fixture freeze point is early and strict enough
8. whether the common observer/timing methodology is comparable across all three candidates
9. whether process/resource accounting includes lazy helpers and language runtimes correctly
10. whether Windows screening + mandatory macOS validation remains sufficient
11. whether the Linux/X11/Wayland feasibility gate remains sufficient
12. whether the untimed Codex compatibility gate remains sufficient
13. whether any benchmark scenario adds noise without decision value
14. whether any missing test could realistically change the final language decision
15. whether the experiment is now stable enough to hand to an implementation agent

## Important constraints

- No Electron, Chromium, WebView, Tauri frontend, or Node runtime.
- Low RAM and low interaction latency are first-class product requirements.
- Platform-native shims are allowed.
- Pure-language dependency stacks are not required.
- Dependency symmetry is not required.
- CPU/GPU/native-widget/custom-rendered approaches are all allowed if they meet the same correctness/accounting contract.
- cgo is allowed for Go when justified.
- Go runtime/GC memory and CPU remain part of application-owned cost.
- Rust/Zig allocator/runtime details also remain part of application-owned cost.
- Do not prefer Rust merely because its ecosystem is larger.
- Do not prefer Zig merely because its theoretical runtime floor is smaller.
- Do not prefer Go merely because orchestration/concurrency may be simpler.
- Do not penalize Go merely for having a GC if measured product-level behavior remains competitive.
- A few MiB of memory difference is not automatically worth substantially more infrastructure.
- The prototype must remain small enough that three implementations are affordable.
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

### B. Previous-blocker verification

State explicitly whether each previous blocker is now resolved:

- text visual/editing oracle
- cancellation/exceptional provider lifecycle

### C. Three-candidate fairness audit

State whether Rust, Zig, and Go now receive a technically fair comparison.

Identify any remaining artificial-win path, including runtime/GC accounting.

### D. Benchmark audit

Review only measurements that could realistically change the language decision:

- resident/commit/peak memory
- runtime/GC diagnostics
- startup/first-use/warm latency
- presentation timing
- CPU/redraw/wakeups
- repetitions/order
- stability/resource growth
- process/helper accounting

### E. Scope audit

Identify anything still too large, too small, missing, or unnecessary.

### F. Platform-decision audit

Answer explicitly:

- Is Windows appropriate for screening all three candidates?
- Is mandatory macOS validation sufficient before final selection?
- Is the Linux/X11/Wayland feasibility gate sufficient?

### G. Provider-integration audit

Answer explicitly:

- Is the revised structured mock sufficient for comparative benchmarking?
- Is the untimed Codex app-server gate sufficient before final language selection?
- Is there any decision-critical reason to add Devin ACP now?

### H. Go-candidate audit

Answer explicitly:

- Is Go a useful third candidate for this exact product?
- Are the current Go-specific diagnostics sufficient?
- Is any additional Go-only benchmark necessary for a fair decision?
- Does adding Go materially distort or over-expand the experiment?

Do not choose the final language yet.

### I. Approved minimum prototype

Give the exact minimum scope you would approve for all three candidates.

### J. Proposed document changes

Give only concrete edits still required or strongly recommended.

### K. Final review status

End with exactly one of:

- PLAN_APPROVED
- PLAN_APPROVED_WITH_NON_BLOCKING_CHANGES
- PLAN_NEEDS_REVISION

Do not choose Rust, Zig, or Go in this review.
