# Mascot Native UI Foundation v0.1 — SWE-2 Task Specification

**Branch:** `agent/native-ui-foundation-v0.1-swe2`  
**Base snapshot:** `f6e8e6ebfbfb8d8abe2edbdf77893e0c9e43b3a5`  
**Platform for this task:** Windows first  
**Language:** Rust  
**Milestone:** M1A native visual playground

## 1. Objective

Build the first product-oriented native UI foundation for Mascot.

The task is intentionally parallel to ongoing animation/rig work.

Deliver a Windows-native visual playground that demonstrates the real product interaction language:

- approved mascot/rest image;
- compact floating composer;
- response expansion;
- native text editor;
- small native icon system;
- light/dark themes;
- left/right placement;
- deterministic visual evidence;
- event-driven idle behavior.

This is not the benchmark UI and should not polish the benchmark composer.

## 2. Isolation from sibling animation work

This branch was cut from the current Planner/rig branch snapshot so it has the current repository policies and workspace structure.

However, this task MUST remain logically isolated from the animation implementation.

Do not modify unless strictly necessary:

- `crates/mascot-animation/`;
- rig data/assets;
- animation clips;
- rig visual QA logic;
- animation evidence;
- animation task docs.

If a reusable low-level renderer improvement is necessary, keep it surgical and document why.

Do not merge the sibling animation branch or wait for it to finish. Use the existing mascot/rest artifact as a static visual anchor.

## 3. Preferred repository shape

Preferred additions:

    crates/
      mascot-ui/
      mascot-icons/
      mascot-ui-win32/        # if platform glue warrants a separate owner

    apps/
      mascot-ui-lab/

Exact layout may adapt after inspecting the existing workspace.

Ownership intent:

- `mascot-ui`: tokens, layout/state/scene concepts that do not require Win32;
- `mascot-icons`: small typed icon vocabulary/path data;
- Win32 crate/module: Direct2D/DirectWrite/native editor/window integration;
- `mascot-ui-lab`: developer playground and deterministic state capture.

Do not create a generic framework larger than the required surface.

## 4. Required visual implementation

Implement the visual states defined in:

- `docs/MASCOT_NATIVE_UI_DESIGN_SYSTEM_V0.1.md`

The lab must make it easy to inspect all required states without code edits.

A simple developer state switcher/hotkeys are acceptable.

The final product UI should visually read as:

- shadcn-inspired restraint;
- Devin-like monochrome/neutral product feel;
- original Mascot composition;
- native, not HTML/CSS-looking;
- compact and polished.

Do not copy website branding or exact external layouts.

## 5. Composer

The composer is the most important control in this task.

Requirements:

- native editable text path;
- visually embedded in the custom bubble;
- caret;
- selection;
- keyboard navigation;
- copy/paste;
- Unicode;
- IME/dead-key compatibility;
- multiline growth with bounds;
- explicit Enter vs Shift+Enter behavior;
- focus treatment;
- no visible default Win32 chrome that breaks the product look.

Do not fake the editor with painted text.

## 6. Response surface

Implement a deterministic mocked response state.

Requirements:

- compact readable body text;
- bounded width;
- sensible wrapping;
- copy icon/action;
- follow-up composer;
- no history architecture;
- no Markdown engine in v0.1;
- no provider process required.

Plain text is sufficient.

## 7. Icon system

Implement the minimal typed icon system from the design doc.

Requirements:

- compact vector path representation;
- no runtime SVG/XML parser;
- no icon font;
- no JS/runtime package;
- native Direct2D path rendering;
- consistent stroke sizing/alignment;
- license/provenance documented.

The icon source vocabulary should be visually compatible with the shadcn direction.

## 8. Theme/tokens

Implement a small token model for:

- neutral colors;
- text hierarchy;
- border;
- hover/pressed;
- focus;
- radii;
- spacing;
- icon size;
- control size;
- shadow.

Do not implement CSS variables or a general theme language.

Light/dark switching must be available in the lab.

## 9. Native rendering constraints

Use the existing Rust/native direction.

Preferred Windows technologies:

- Win32;
- Direct2D;
- DirectWrite;
- DirectComposition where useful;
- native RichEdit/editor.

No:

- Electron;
- Chromium;
- WebView;
- Tauri frontend;
- Node;
- egui;
- iced;
- Slint;
- Qt;
- game engine;
- wgpu.

If the existing `mascot-render-win32` exposes useful low-level capability, reuse it only when ownership remains clean. Do not turn a rig-specific crate into a God renderer.

## 10. Window behavior

The UI lab should use transparent/borderless native presentation suitable for the product direction.

Required:

- bubble and mascot appear as one coherent composition;
- transparent exterior does not look like a rectangular app window;
- no standard title bar;
- move/resize behavior does not reveal large opaque backing surfaces;
- no continuous idle repaint.

Final production click-through/hit-test sophistication can be deferred if it would distract from M1A, but the chosen structure must not block it.

## 11. Layout and DPI

Layout must use device-independent intent.

Validate required states at:

- 100%;
- 125%;
- 150%;
- 200%.

Required evidence includes DPI contact sheets.

Check:

- border sharpness;
- icon alignment;
- text baseline;
- editor inset;
- mascot overlap/perch;
- hit-target geometry;
- response wrapping.

## 12. Visual iteration process

SWE-2 is expected to iterate visually rather than stop at the first functional layout.

Use deterministic capture tooling.

Preferred loop:

    implement/tune
      -> capture all states
      -> build light/dark contact sheets
      -> inspect at normal size and zoom
      -> correct spacing/alignment/weight
      -> repeat

Promote repeated capture/contact-sheet work into repository-owned tooling or lab commands. Do not leave essential review workflow only in ignored scratch scripts.

## 13. Performance

Static UI must not have a permanent frame loop.

Measure on native Windows:

- static-frame activity;
- idle CPU;
- working/private working set;
- threads;
- handles;
- first show;
- warm show;
- light/dark state switch;
- open/close cycle.

Compare qualitatively against the existing Rust native foundation.

Any unexpected large regression requires profiling before completion.

## 14. Tests

Add focused tests where semantics can be deterministic without UI automation, including as applicable:

- token/layout math;
- bounded composer sizing;
- icon registry;
- theme completeness;
- state transition model;
- DPI conversion/layout;
- hit geometry;
- no invalid/duplicate icon IDs.

Do not replace visual inspection with unit tests.

## 15. Evidence

Commit final evidence under:

    benchmark/results/windows/native-ui-v0.1/

Required:

- README with environment/build/run instructions;
- light contact sheet;
- dark contact sheet;
- DPI contact sheet;
- representative individual screenshots;
- performance JSON/table;
- visual review notes;
- known limitations.

Optional:

- short MP4 showing open/type/expand/theme switch;
- icon-alignment crop sheet.

Evidence must correspond to the final candidate.

## 16. Explicit non-scope

Do not implement:

- real Devin ACP;
- real Codex/provider integration;
- daemon/agentd;
- history database;
- settings architecture;
- plugin system;
- notifications framework;
- screenshot capture;
- microphone/STT/TTS;
- browser automation;
- updater/installer;
- full animation integration;
- walking/rig work;
- wgpu;
- macOS implementation;
- Linux implementation.

## 17. Authoritative native Windows Git workflow

For this task, native Windows Git is authoritative.

The machine already has working Git + SSH access to GitHub.

Use a dedicated clone under the existing Windows `devin_folder`, for example:

    W:\\devin_folder\\mascot-ui-v01

Clone/fetch directly from:

    git@github.com:rceman/mascot.git

The agent MUST:

- use the Windows clone as the only working Git checkout for this task;
- fetch origin;
- checkout `agent/native-ui-foundation-v0.1-swe2`;
- verify the required Planner baseline/current branch state before editing;
- implement/build/run/capture/profile natively on Windows;
- keep normal Git history in this Windows clone;
- review `git status` and the complete diff before commit;
- commit and push directly from Windows over the configured SSH remote;
- finish with the task branch pushed and the Windows worktree clean.

Do NOT copy source back and forth through WSL for this task.

Do NOT create a second authoritative checkout or sync pipeline.

Do not commit build outputs or machine-local state such as:

- `target/`;
- compiler/build caches;
- temporary screenshots not selected as evidence;
- scratch files;
- local tool environments;
- unrelated machine-specific state.

## 18. Completion

The task is complete only when:

- the native UI lab builds/runs on Windows;
- required visual states exist;
- light/dark evidence is committed;
- DPI evidence is committed;
- native editor behavior is verified;
- icons are native vector paths with no runtime SVG dependency;
- static UI has no continuous redraw;
- performance is measured;
- all applicable Universal Gates 1-20 pass;
- final branch is pushed and clean.

Do not merge into another branch as part of this task.
