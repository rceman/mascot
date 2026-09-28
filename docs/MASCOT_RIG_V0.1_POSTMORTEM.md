# Rejected rig v0.1 — experiment notes

Status: rejected as an animation architecture; retained as design evidence.

## What was attempted

The approved flattened mascot was decomposed into raster layers with explicit origins/pivots and then recomposed on the canonical canvas.

The experiment separated approximately:

- body
- head
- eyes
- visible/placeholder ears
- visible/placeholder arms
- near/far legs
- tail
- laptop shell
- laptop screen

It also defined an initial future-looking bone hierarchy and rotation origins.

## What worked

The rest/default pose could be recomposed exactly from the extracted raster layers in the experiment:

    different pixels: 0
    max RGBA delta: 0

This demonstrated that exact canvas registration and explicit layer coordinates are feasible.

The experiment also reinforced useful requirements:

- independent legs are needed, not only for seated idle motion but later walking
- both ears should be independently articulated
- head/body should be independently transformable
- eyes should support independent motion/blink
- exact pivots/origins and parent relationships must be machine-readable
- whole-character horizontal mirror should be a root-level transform
- shadows should not be baked into the mascot art

## Why it was rejected

The small-rotation stress render exposed visible failures:

- seams at joints
- insufficient hidden geometry under overlapping parts
- black wedges caused by moving already-outlined pieces
- doubled external outlines
- joint regions that looked acceptable only in the exact rest pose

The central problem was architectural: each moving raster piece behaved too much like a finished sticker with its own external outline.

Exact rest-pose equality therefore did not imply animation readiness.

## v0.2 correction

The next experiment must:

1. preserve canonical visible art where practical
2. reconstruct concealed geometry under joints
3. use generous overlap beneath neighboring parts
4. composite transformed fill artwork first
5. derive one final external silhouette outline from the composed alpha
6. keep internal line art distinct from the final outer outline
7. render shadows at runtime
8. run deterministic small-rotation stress renders as a primary acceptance gate

## Art/runtime separation

Artwork should contain the character's visual material and deliberate internal details.

Runtime should own effects that depend on the current composed pose:

- external silhouette outline
- contact/drop shadow
- future bubble shadow
- optional screen glow
- whole-rig mirror

## Walking implication

The rig should expose a skeleton capable of later:

    hips
      -> upper leg
      -> lower leg / knee
      -> foot

even if v0.2 still attaches a rigid whole-leg sprite initially.

This avoids rebuilding the animation model when walking or weighted mesh deformation is added.

## Conclusion

Do not attempt to salvage the rejected v0.1 layer boundaries merely to preserve prior work.

Its useful output is the failure evidence and the architectural constraints now captured in docs/MASCOT_RIG_RUNTIME_SPEC_V0.2.md.
