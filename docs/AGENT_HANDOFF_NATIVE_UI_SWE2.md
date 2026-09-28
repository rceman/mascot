# Agent Handoff — Native UI Foundation v0.1 (SWE-2)

## Mission

Implement the Windows-native Mascot UI foundation on the dedicated branch:

    agent/native-ui-foundation-v0.1-swe2

This is an isolated M1A product-UI task running in parallel with animation/rig work.

Do not merge sibling branches.

## Read first

Read these files in full before editing:

1. `AGENTS.md`
2. `docs/QUALITY_GATES.md`
3. `docs/MASCOT_NATIVE_UI_DESIGN_SYSTEM_V0.1.md`
4. `docs/MASCOT_NATIVE_UI_FOUNDATION_V0.1_SPEC.md`
5. `docs/MASCOT_UI_GATE_PROFILE_V0.1.md`
6. `docs/MASCOT_PRODUCT_UI_ANIMATION_DIRECTION.md`
7. `docs/BENCHMARK_RESULTS.md`

The UI design/spec/profile in this branch are authoritative for this task.

## Branch / baseline

Expected branch:

    agent/native-ui-foundation-v0.1-swe2

Expected Planner documentation baseline will be the branch HEAD after these docs are committed.

Before implementation:

- fetch origin;
- verify branch;
- verify clean/understood worktree;
- record starting HEAD.

Do not reset away Planner documentation commits.

## Product direction

Build a small native black/white/neutral UI inspired by the restraint of shadcn-style components and the clean monochrome feel of Devin-like product surfaces.

Do not copy external branding or website layout.

The product should look like a native Mascot shell, not a browser page.

Primary composition:

                mascot
          +----------------------+
          | Ask anything...    ↑ |
          +----------------------+

Expanded:

                mascot
          +------------------------+
          | mocked response...     |
          |                    copy|
          +------------------------+
          | Ask follow-up...     ↑ |
          +------------------------+

Mascot visually perches on the bubble.

## Native implementation

Use Rust and native Windows APIs.

Preferred:

- Win32;
- Direct2D;
- DirectWrite;
- DirectComposition where useful;
- native RichEdit/editor.

Do not add:

- Electron;
- Chromium;
- WebView;
- Tauri frontend;
- Node;
- egui;
- iced;
- Slint;
- Qt;
- a game engine;
- wgpu;
- runtime SVG/XML parser.

Use a small project-owned UI layer only for the primitives actually required.

## Icons

Implement a small typed icon vocabulary using compact native vector paths.

Use shadcn-compatible/Lucide-style visual vocabulary where licensing permits.

No runtime SVG parser and no icon font.

Document icon provenance/license.

## Scope isolation

Do NOT redesign or repair the mascot rig.

Do NOT change animation clips, pivots, hidden geometry or animation QA.

Use the current mascot/rest image as a static anchor.

Keep changes out of sibling animation-owned code unless a tiny low-level reuse change is strictly required and justified.

## Visual iteration

Do not stop at "it renders".

Iterate on:

- bubble proportions;
- mascot overlap;
- radius;
- border weight;
- typography;
- icon alignment;
- editor inset;
- hover/focus/pressed states;
- response spacing;
- light/dark balance;
- DPI behavior.

Generate deterministic final contact sheets and inspect them.

## Native editor

The composer must be a real native editable control/stack.

Verify:

- caret;
- selection;
- copy/paste;
- multiline;
- keyboard navigation;
- Unicode;
- IME/dead keys;
- Enter/Shift+Enter;
- focus.

Do not fake editable text with DirectWrite glyph painting.

## Performance

Static UI must stop producing frames.

No permanent 60 Hz loop.

Measure native Windows:

- idle frame activity;
- CPU;
- memory;
- threads;
- handles;
- first show;
- warm show.

Investigate material regressions.

## Evidence

Commit final evidence under:

    benchmark/results/windows/native-ui-v0.1/

Do not commit noisy exploratory output.

Required final sheets and report are defined in the task spec.

## Universal Gates

Use the canonical Universal Gates 1-20 only.

`docs/MASCOT_UI_GATE_PROFILE_V0.1.md` defines the UI-specific evidence.

The final report must list Gate 1 through Gate 20 as PASS/FAIL/N/A with concise evidence.

No second numbered gate taxonomy.

## Git / platform workflow

For native Windows validation, preserve the repository's established workflow:

- WSL checkout is authoritative Git state;
- native Windows is used for build/run/render/performance validation;
- do not copy/replace `.git`;
- sync intended source/evidence only;
- review status/diff in WSL;
- commit/push from WSL;
- leave authoritative WSL worktree clean.

If the exact WSL checkout path is not available, ask only for that path before performing the WSL/Windows sync workflow.

Do not add CI.

## Completion

Return:

1. starting HEAD;
2. final branch + HEAD;
3. architecture/layout;
4. crates/modules added;
5. native rendering stack;
6. native editor implementation;
7. icon pipeline + license/provenance;
8. required UI state status;
9. DPI results;
10. light/dark evidence paths;
11. performance table;
12. known limitations;
13. Universal Gates 1-20 PASS/FAIL/N/A;
14. final Git clean/push confirmation.

End with exactly one of:

    MASCOT_NATIVE_UI_V01_COMPLETE

or

    MASCOT_NATIVE_UI_V01_BLOCKED: <reason>
