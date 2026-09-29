# Mascot Native UI Component Gallery v0.1 — SWE-2 Follow-up

**Branch:** `agent/native-ui-component-gallery-v0.1-swe2`  
**Base:** completed native UI foundation `4ab080d818d41b855b77590ff65a900c1ec3dc65`  
**Platform:** Windows native Rust  
**Purpose:** make the owned UI component system visually inspectable

## 1. Objective

Add a developer-owned component gallery / preview-sheet workflow so a reviewer can see the complete current Mascot component vocabulary at a glance.

This is the native equivalent of a very small Storybook/component showcase, but it must stay project-specific and deterministic.

Do not turn Mascot into a general UI toolkit.

## 2. Authoritative component scope

Read:

- `docs/MASCOT_UI_COMPONENT_INVENTORY_V0.1.md`

Only Tier A components are implemented/generalized in this task.

Tier B components may be listed as planned but must not be implemented.

Tier C remains out of scope.

## 3. Required gallery command

Add one stable repository-owned command through the existing UI lab, for example:

    mascot-ui-lab components

or an equivalent concise subcommand consistent with the current CLI.

It must deterministically render the full Tier A component gallery.

Also support a deterministic capture mode suitable for evidence, for example:

    mascot-ui-lab components --capture <dir>

Exact CLI spelling may follow the existing app's conventions.

## 4. Preview sheets

Generate and commit at minimum:

    benchmark/results/windows/native-ui-components-v0.1/
      README.md
      component-gallery-light.png
      component-gallery-dark.png
      component-gallery-sizes.png
      component-gallery-shadcn-reference.png
      component-inventory.json
      visual-review.md
      reference/
        PROVENANCE.md
        ...

The core goal is immediate visual inspection.

### Light / dark gallery

Each sheet should show Tier A components grouped by category.

For interactive controls show meaningful state variants side-by-side.

### Size gallery

For stretchable components show:

- minimum practical width;
- current default width;
- wider practical width.

For content-sized components show representative label lengths/supported sizes.

For fixed-size components do not invent stretched variants.

### shadcn comparison gallery

Where a direct shadcn analogue exists, create a comparison row/section:

    component
    shadcn reference
    Mascot native implementation
    intentional deviations

Preferred: include a small captured official shadcn visual reference next to the native component.

Every reference image must have provenance recorded with:

- official URL;
- component name;
- capture date;
- theme/state;
- any relevant version/commit if available.

Reference captures are developer evidence only and must not become a runtime dependency or shipped product asset.

If an exact direct shadcn analogue does not exist (for example the Mascot response composition), label it clearly rather than inventing a fake reference.

## 5. Gallery layout

The sheet itself must be easy to review.

Suggested structure:

    COMPONENT       REFERENCE        DEFAULT        HOVER       FOCUS       DISABLED
    -------------------------------------------------------------------------------
    IconButton      shadcn           native         native      native      native
    Button          shadcn           native         native      native      native
    Tooltip         shadcn           native
    Composer        Input/Textarea    min/default/wide + focused/multiline
    Badge           shadcn           short/medium/long
    ...

Avoid tiny unreadable cells.

Prefer multiple coherent sheets over one giant unreadable bitmap.

## 6. Implementation rules

The gallery must use the actual production/native component painting/layout code.

Do not duplicate a second "fake gallery renderer" that merely resembles the components.

If current hardcoded product rendering prevents reuse, extract only the smallest reusable component primitive necessary.

Examples:

- reusable button paint/layout;
- reusable surface paint;
- reusable tooltip paint;
- reusable text-input framing;
- reusable badge paint.

Do not introduce a general widget framework.

## 7. shadcn-first review

For each Tier A component with a shadcn analogue, review:

- spacing;
- padding;
- height;
- radius;
- border weight;
- neutral colors;
- typography;
- icon size;
- focus ring;
- hover/pressed treatment;
- disabled treatment.

Differences are allowed when caused by:

- native RichEdit behavior;
- Mascot composition;
- DPI/native window requirements;
- accessibility;
- explicit product need.

Record intentional deviations in `visual-review.md`.

## 8. DPI

The component gallery must remain coherent at:

- 100%;
- 125%;
- 150%;
- 200%.

It is sufficient to include a compact DPI evidence strip for representative components rather than duplicating every state four times.

## 9. Existing UI preservation

Do not regress the completed native UI foundation.

The existing:

- composer;
- response;
- icon buttons;
- native editor;
- Lucide pipeline;
- zero-idle-frame behavior;
- existing evidence/selftest/perf behavior

must remain green.

## 10. Verification

Required:

- full workspace tests;
- existing native UI selftest;
- new deterministic component-gallery capture;
- visual inspection of all final sheets;
- light/dark comparison;
- representative DPI comparison;
- source inventory proving Tier B/C components were not accidentally implemented;
- final clean Git state.

## 11. Performance

The developer gallery is not a production runtime feature, but extracting reusable components must not introduce:

- continuous redraw;
- runtime SVG parsing;
- unnecessary allocations per idle frame;
- browser/UI-framework dependencies.

Do not optimize gallery capture at the expense of production clarity.

## 12. Completion

Complete only when:

- Tier A inventory matches implementation;
- gallery command exists;
- final component sheets are committed;
- stretchable components have meaningful size variants;
- fixed-size controls are not artificially stretched;
- shadcn comparison/provenance exists where applicable;
- intentional deviations are documented;
- Tier B/C remain unimplemented;
- existing native UI tests/evidence behavior remains valid;
- Universal Gates 1-20 pass for the changed cone;
- branch is pushed and clean.
