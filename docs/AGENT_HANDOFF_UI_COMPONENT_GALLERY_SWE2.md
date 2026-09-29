# Agent Handoff — Native UI Component Gallery v0.1 (SWE-2)

## Context

The native UI foundation is already complete at:

    4ab080d818d41b855b77590ff65a900c1ec3dc65

The previous task produced product-state contact sheets, but we also need a dedicated **component preview/gallery** so the UI system itself can be reviewed at a glance.

This is a follow-up, not a rewrite of the foundation.

## Branch

Work only on:

    agent/native-ui-component-gallery-v0.1-swe2

Use native Windows Git/SSH as in the previous task.

## Read first

Read in full:

1. `AGENTS.md`
2. `docs/QUALITY_GATES.md`
3. `docs/MASCOT_NATIVE_UI_DESIGN_SYSTEM_V0.1.md`
4. `docs/MASCOT_UI_COMPONENT_INVENTORY_V0.1.md`
5. `docs/MASCOT_UI_COMPONENT_GALLERY_V0.1_SPEC.md`
6. `docs/MASCOT_UI_COMPONENT_GALLERY_GATE_PROFILE_V0.1.md`
7. existing native UI foundation docs/evidence

## Main requirement

Build a deterministic developer component gallery using the actual native component implementation.

We want to open one preview or inspect a few generated sheets and immediately understand:

- which components exist;
- what they look like;
- their states;
- their supported sizes;
- light/dark behavior;
- how closely they follow the shadcn-first reference;
- which deviations are intentional.

## Do not build all shadcn components

The component inventory is authoritative.

Implement/generalize only Tier A.

Tier B = planned, not implemented.

Tier C = out of scope.

Do not expand scope because a shadcn component looks useful.

## Size preview rule

For stretchable components, show meaningful width variants:

- min;
- default;
- wide.

For content-sized controls show representative content lengths.

For fixed-size controls do not create artificial stretched versions.

The same rule will later apply to Checkbox/Radio/Switch if/when they are promoted from Tier B.

## shadcn reference

For each implemented component with a direct shadcn analogue:

- use the official shadcn docs as the reference;
- capture/store a small developer-only reference image where practical;
- place it beside the native component in the comparison sheet;
- record URL/capture provenance;
- document intentional differences.

Do not add a browser/runtime dependency to Mascot.

## Evidence

Commit final evidence under:

    benchmark/results/windows/native-ui-components-v0.1/

At minimum:

- component-gallery-light.png
- component-gallery-dark.png
- component-gallery-sizes.png
- component-gallery-shadcn-reference.png
- component-inventory.json
- visual-review.md
- reference/PROVENANCE.md

The gallery must remain readable; split sheets rather than making one enormous microscopic image.

## Preserve the completed foundation

Do not regress the current native composer, RichEdit behavior, Lucide icon pipeline, DPI behavior, selftest, performance model or zero-idle-frame behavior.

Do not redo rig/animation work.

Do not add CI.

## Completion

Use Universal Gates 1-20 and the gallery gate profile.

Finish on the same branch, push directly from Windows over SSH, leave the worktree clean, and report:

- component inventory implemented/planned/deferred;
- gallery command;
- sheets/evidence;
- shadcn reference provenance;
- intentional visual deviations;
- tests/selftest status;
- final HEAD;
- Gates 1-20 PASS/FAIL/N/A.

End with exactly one of:

    MASCOT_UI_COMPONENT_GALLERY_V01_COMPLETE

or

    MASCOT_UI_COMPONENT_GALLERY_V01_BLOCKED: <reason>
