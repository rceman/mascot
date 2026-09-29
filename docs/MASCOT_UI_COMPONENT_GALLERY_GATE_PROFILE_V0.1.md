# Mascot UI Component Gallery v0.1 — Universal Gate Profile

**Canonical taxonomy:** `docs/QUALITY_GATES.md` Gates 1-20

This file only specializes evidence for the component-gallery follow-up.

## Gate 1

PASS requires the inventory, gallery workflow, required sheets, size classes, shadcn comparison and final evidence to exist.

## Gate 2

Do not silently promote Tier B/C components into implementation. A new component requires a concrete current product need or Planner approval.

## Gate 3

Existing native UI behavior and evidence commands must remain functional.

## Gate 4

Scope is component inventory, minimal reusable component extraction, gallery tooling and evidence only.

## Gate 5

A small gallery is required; a generic widget framework/Storybook clone is not.

## Gate 6

Production component primitives stay in their owning UI/Win32 crates. Gallery orchestration stays in the lab/dev tooling.

## Gate 7

The gallery must render the real production component code, not duplicate lookalike painters.

## Gate 8

No alternate web-rendered product path, fallback UI, or duplicate component implementation.

## Gate 9

Component names, sizing classes, states and theme semantics must be deterministic.

## Gate 10

Re-read component inventory, UI design system, UI foundation spec and this handoff before completion.

## Gate 11

Failed reference capture/gallery generation must fail clearly; partial sheets must not masquerade as final evidence.

## Gate 12

Bound gallery dimensions, number of states, reference captures and output. Avoid giant unreadable sheets.

## Gate 13

No browser/UI framework/runtime SVG dependency may be added to the product. Dev-only reference capture tooling must remain dev-only and justified.

## Gate 14

Reusable component extraction must not add idle redraw or repeated unnecessary work to the production UI.

## Gate 15

Reference/evidence paths stay out of production UI semantics.

## Gate 16

Actual final sheets must be visually inspected. Unit tests alone are insufficient.

## Gate 17

If a reusable abstraction makes the current simple UI harder, revert/simplify rather than preserving sunk cost.

## Gate 18

Final sheets must identify the exact source HEAD. Regenerate after visual/component changes.

## Gate 19

Inspect every final sheet, inventory status, final diff and clean/pushed branch before COMPLETE.

## Gate 20

Systemic claims requiring exhaustive audit include:

- every Tier A component appears in the gallery;
- every stretchable Tier A component has required width variants;
- every relevant interactive state is represented;
- every direct shadcn analogue has provenance/reference or an explicit reason;
- no Tier B/C component was implemented;
- gallery uses production component code rather than duplicate lookalikes.

Report Gates 1-20 PASS/FAIL/N/A with concise evidence.
