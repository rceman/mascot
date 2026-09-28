# AGENTS.md

Repository-wide instructions for AI coding agents.

## 1. Authority

Project-owned specifications, gate definitions and task handoffs are authoritative.

Do not silently weaken an acceptance criterion to make a task pass.

If a requirement cannot be met, report the concrete blocker instead of adding a workaround that changes semantics.

## 2. Internal evolution is hard-cut by default

Internal formats, internal APIs, development tooling and unreleased implementation details are replaced directly unless the task explicitly names a real external compatibility boundary.

Do not add, preserve or invent compatibility machinery by default.

Forbidden without explicit Planner approval:

- backward-compatibility loaders;
- legacy aliases;
- fallback parsing;
- dual-read or dual-write formats;
- deprecated-field support;
- temporary adapters left in production code;
- compatibility feature flags;
- version sniffing for superseded internal formats;
- duplicated canonical representations;
- "keep the old path just in case" branches.

A compatibility boundary is real only when it is explicitly named, for example:

- released/public API;
- persisted user data that must be read;
- external third-party consumer;
- plugin/provider protocol outside this repository;
- supported released product version.

If no such boundary is named, delete obsolete code/data after migration.

One-shot migration tooling is allowed while performing a hard cut, but remove it when the repository has fully migrated unless it has durable value.

## 3. No speculative shims

Do not solve uncertainty with a shim.

When unsure whether compatibility is required:

1. inspect the current task/spec;
2. inspect current repository consumers;
3. if no real external consumer is identified, use a hard cut;
4. if a real boundary exists but policy is unclear, stop and ask.

Compatibility is a cost that must be justified, not a default safety behavior.

## 4. Project-owned gates are mandatory

Use the canonical Mascot Universal Gates 1-20 defined in:

    docs/QUALITY_GATES.md

These twenty gates are the only repository-wide completion/review gate taxonomy. Lower-level commands such as formatting, tests, visual QA and performance checks are evidence mechanisms for the universal gates, not a second numbered gate system.

Do not replace a project-owned verification mechanism with an ad-hoc approximation and then report the corresponding universal gate as passed.

Focused commands are useful during implementation, but completion requires the canonical gates applicable to the task.

A task is not complete when:

- required gates were skipped;
- a gate failed;
- only a partial substitute was run;
- visual QA contains unresolved ERROR findings;
- required WARN findings were not reviewed;
- performance gates reveal an unexplained routine bottleneck;
- generated evidence is stale relative to the final code/art.

## 5. Fast iteration first, canonical verification once

During implementation:

- use focused/affected tests;
- use narrow `mascotctl` inspection commands;
- render only the changed clip/pose where possible;
- reuse cached deterministic intermediates;
- avoid repeatedly running expensive full gates after every small edit.

Before completion, run the canonical project-owned verification required by the materially final candidate and report Gates 1-20 as PASS/FAIL/N/A with concise evidence.

Do not waste agent tokens or machine time by rerunning the same expensive gate without a material change.

## 6. Correctness, visual QA and performance are separate lanes

Do not mix machine-load-sensitive performance thresholds into normal deterministic correctness tests.

Keep distinct:

- formatting/static correctness;
- deterministic unit/integration correctness;
- visual rig/animation QA;
- native Windows runtime/performance measurement.

Performance measurements must report environment identity and timings.

## 7. Agent-facing tooling must be reusable

If a workflow is used repeatedly, promote it into repository-owned tooling.

Do not leave important recurring work only in ignored `scratch/` scripts.

Examples that belong in durable tooling:

- pose/clip rendering;
- zoom/crop inspection;
- contact-sheet generation;
- visual artifact detection;
- frame extraction;
- benchmark/timing reports;
- rig/clip inspection;
- validation/gate orchestration.

The goal is that the next agent uses one documented command rather than rewriting plumbing.

## 8. Authoring data should be token-efficient

Agent-maintained data formats should avoid structural repetition.

Prefer:

- groups;
- compact tuples;
- scoped inspection output;
- one canonical representation;
- deterministic runtime expansion.

Do not make the canonical authoring format verbose merely because the runtime structure is explicit.

## 9. Native Windows task workflow

For tasks whose handoff specifies native Windows validation, follow the task-specific Git authority declared by the handoff.

When native Windows Git/SSH is available and the handoff designates Windows as authoritative:

- clone/fetch the repository directly on Windows using the configured Git/SSH setup;
- work on the exact task branch in the Windows clone;
- build/run/render/profile natively on Windows;
- review `git status` and the complete diff in that same clone;
- commit and push from Windows;
- finish with the task branch pushed and the Windows worktree clean.

Do not introduce an unnecessary WSL copy/sync round-trip when native Windows Git authority is explicitly approved.

Do not use CI as a substitute when the task explicitly requires native local Windows validation.

## 10. Evidence must describe the final candidate

If code, rig data, art, animation data or renderer behavior changes after evidence was generated, regenerate affected evidence.

Do not commit an old contact sheet/video/benchmark result and present it as proof for a newer candidate.

## 11. Completion language

Use COMPLETE only after all applicable Universal Gates 1-20 and required evidence pass. N/A requires a concrete changed-cone rationale.

Known visible defects inside a declared safe range are not a successful visual-QA completion.

If a range cannot be made visually clean with the chosen model, reduce the declared safe range honestly or improve the model.
