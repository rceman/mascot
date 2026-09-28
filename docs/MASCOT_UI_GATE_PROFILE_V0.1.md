# Mascot Native UI v0.1 — Universal Gate Profile

**Status:** task specialization  
**Canonical taxonomy:** `docs/QUALITY_GATES.md` Gates 1-20

This file does not create new gates. It defines the UI-specific evidence expected for the canonical Universal Gates.

## Gate 1 — Requirements & Goal Completeness

PASS requires every requirement in:

- `docs/MASCOT_NATIVE_UI_DESIGN_SYSTEM_V0.1.md`;
- `docs/MASCOT_NATIVE_UI_FOUNDATION_V0.1_SPEC.md`;
- `docs/AGENT_HANDOFF_NATIVE_UI_SWE2.md`

to map to implementation/evidence or an explicitly approved N/A.

## Gate 2 — Assumptions, Ambiguity & Approval Boundaries

Do not silently decide material product semantics.

Escalate if blocked by a decision about:

- fundamental bubble interaction;
- product navigation;
- persistent history/settings;
- whether multiple windows are product-visible;
- native text behavior that would change user semantics.

Minor visual tuning within the approved design language is owned by SWE-2.

## Gate 3 — Existing Behavior & Contract Preservation

The task must not regress unrelated:

- rig/animation runtime;
- benchmark foundation;
- existing native platform behavior;
- workspace builds/tests.

Static UI work must not rewrite sibling animation semantics.

## Gate 4 — Scope Discipline

Changed files must trace to UI foundation, tests, evidence or required documentation.

FAIL for unrelated rig work, provider work, benchmark cleanup or broad workspace refactoring.

## Gate 5 — Minimal Correct Design

PASS requires a narrow project-owned UI layer, not a general widget framework.

Explicit FAIL examples:

- CSS engine;
- flexbox clone without demonstrated need;
- generic retained-mode UI framework;
- speculative renderer backend architecture;
- wgpu added "for later";
- large design-system machinery for four controls.

## Gate 6 — Architecture & Ownership

Expected separation:

- UI semantics/tokens/layout -> UI crate;
- icons -> typed icon owner;
- Win32 rendering/editor/window glue -> Windows owner;
- lab state/capture -> app/tooling owner;
- rig/animation remains owned by sibling crates.

Product runtime must not depend on dev-only screenshot/video tooling.

## Gate 7 — Duplication & Reuse

Reuse existing safe low-level native/render capability where ownership fits.

Do not duplicate device creation/image decode helpers unnecessarily.

Do not force reuse when it would turn the rig renderer into a generic God crate.

## Gate 8 — No Fallback / Shim / Legacy Path

Do not keep both benchmark UI and product UI behind speculative compatibility routing.

The benchmark application may remain as historical/benchmark code, but the new product UI foundation must have one clear canonical implementation path.

No fallback to browser/WebView UI.

## Gate 9 — Deterministic Semantics & Stable Contracts

The visual contract for M1A is shadcn-first and the canonical icon family is Lucide. A different visual/icon system requires explicit Planner approval.

Document and test:

- DIP coordinate semantics;
- theme token meanings;
- control state meanings;
- icon IDs;
- composer sizing rules;
- Enter/Shift+Enter behavior;
- focus/hover/pressed semantics.

## Gate 10 — Authority / Specs / Obsolescence

Before completion re-read:

- `AGENTS.md`;
- `docs/QUALITY_GATES.md`;
- UI design/spec/handoff;
- `docs/MASCOT_PRODUCT_UI_ANIMATION_DIRECTION.md`.

The benchmark composer is not product visual authority.

## Gate 11 — Failure Safety

Failed initialization/capture/resource creation must fail clearly.

No partial screenshot/contact-sheet generation may be reported as successful evidence.

Native resource lifetime must not leak across repeated show/hide cycles.

## Gate 12 — Boundedness / Agent Output

Bound:

- composer growth;
- response fixture size;
- capture count;
- icon set;
- retained layout/render caches;
- logs/tool output.

Normal UI-lab output should be concise.

## Gate 13 — Dependency Necessity

Every new crate/tool must have a concrete purpose.

FAIL for browser engines, large UI frameworks, wgpu or runtime SVG/XML parsing.

Dev-only capture/image tooling must remain outside production runtime dependencies.

## Gate 14 — Performance / Redundant Work

PASS requires:

- no permanent idle redraw;
- no avoidable polling loop;
- no repeated icon/SVG parsing at runtime;
- no unnecessary texture re-upload on every frame;
- no unexplained large memory regression;
- measured native Windows idle/show behavior.

Profile any surprising slow/open path.

## Gate 15 — Infrastructure Boundary

UI/layout logic must not know:

- screenshot evidence directory internals;
- Git checkout/sync mechanics;
- dev video encoding;
- machine-specific paths.

## Gate 16 — Verification Sufficiency

Required proof includes:

- deterministic focused tests;
- actual native Windows execution;
- native editor/input verification;
- final light/dark screenshots;
- DPI screenshots;
- actual visual review;
- performance measurement.

A passing unit-test suite alone is insufficient.

## Gate 17 — Recovery Proportionality

If an early UI abstraction becomes cumbersome, replace it rather than layering adapters around it.

If a rendering approach structurally conflicts with native editor/DPI/transparency goals, bounded restart is preferred over preserving sunk cost.

## Gate 18 — Verification Immutability / Artifact Identity

Generate final screenshots/performance evidence only after the candidate is materially frozen.

Record exact HEAD and relevant environment identity.

Any later visual/layout/render change invalidates affected evidence.

## Gate 19 — Completion Integrity

Before COMPLETE inspect:

- actual UI at normal scale;
- zoomed alignment where needed;
- native editor behavior;
- light/dark;
- all required DPI variants;
- final evidence;
- final diff from the authoritative Windows Git clone;
- clean/pushed Windows task branch.

Known visible defects may not be hidden behind "functional" status.

## Gate 20 — Systemic Scope Completeness

Automatically applies to claims such as:

- no browser/WebView/framework dependency;
- no wgpu;
- no continuous idle redraw;
- all required icons originate from the canonical Lucide source and use native vector path data;
- all required states have evidence;
- all required DPI variants are validated;
- no runtime SVG parser;
- no UI-specific scratch-only review workflow remains.

Use independent inventory/search methods and show zero unexplained violations.

## Final report

Report Gates 1-20 as:

    PASS / FAIL / N/A

with concise evidence/reference.

N/A requires a changed-cone rationale.

Do not invent additional numbered gates.
