# Agent handoff — Windows mascot rig + animation lab (Opus 5.5)

Task ID: MASCOT-RIG-WIN-002

## Goal

Design and implement the next mascot rig/runtime experiment on native Windows.

This is an art+runtime validation task, not the final product UI.

The canonical mascot image is:

    assets/mascot.png

The previous benchmark established Rust as the product foundation candidate. This task should therefore use Rust and native Windows graphics.

## Read first

Read these documents in full before editing:

- docs/MASCOT_PRODUCT_UI_ANIMATION_DIRECTION.md
- docs/MASCOT_RIG_RUNTIME_SPEC_V0.2.md
- docs/MASCOT_RIG_V0.1_POSTMORTEM.md
- docs/BENCHMARK_RESULTS.md
- rust/ARCHITECTURE.md
- README.md

The first two documents are authoritative for this task.

## Important context

A prior rig decomposition experiment was rejected as an animation solution.

What it proved:

- a flattened PNG can be split and recomposed exactly in the rest pose

What it failed at:

- small rotations exposed seams
- hidden geometry was insufficient
- per-part finished black outlines created doubled contours / wedges

Do not repeat that architecture.

The new design must:

- reconstruct hidden geometry where motion exposes it
- render the final external outline after transformed fill composition
- keep internal decorative line art separate from the external silhouette outline
- render shadows at runtime
- record exact pivots/origins/coordinate systems in rig.json

You may redesign the decomposition if needed.

## Skeleton requirements

At minimum support logical articulation for:

- root
- hips
- body/chest
- neck
- head
- both ears
- both eyes / blink support
- both arms/paws
- both legs/feet
- tail
- laptop
- laptop screen

Leg hierarchy must anticipate later walking.

The v0.2 renderer may still use rigid sprite attachments, but the data/runtime architecture must allow a future weighted MeshAttachment without replacing the bone/clip model.

## Visual acceptance

Generate deterministic pose stress renders.

The accepted range must not show:

- transparent joint holes
- background leaks
- detached limbs
- obvious seams
- doubled outer contours
- black wedge artifacts

Do not merely report that tests passed. Inspect and preserve visual evidence.

## Native runtime constraints

Windows only for this task.

Use Rust and a lightweight native Windows stack.

Preferred direction:

- Win32
- Direct2D
- DirectComposition where useful
- WIC if needed

Do not use Electron, Chromium, WebView, Tauri frontend, egui, iced, Slint, Skia, Unity, Godot or another general-purpose UI/game engine.

Build a small reusable animation crate and a Windows developer animation lab.

## Performance constraint

The static mascot must not require a permanent 60 Hz loop.

Expected model:

    static -> no continuous redraw/update
    active animation -> display-cadence frames
    clip complete -> stop frame production

Measure idle and active resource behavior.

## Git/workspace model

The authoritative Git checkout is in WSL.

The user will provide the exact WSL repository path separately.

Do not start implementation until that path is known.

Once provided:

1. Verify the WSL checkout is on:
       agent/windows-rig-animation-lab-v0.2-opus55
2. Verify expected HEAD and clean/understood worktree.
3. Create a native-Windows working copy for implementation/build/test.
4. Do all builds, runtime testing, screenshot/render generation and performance measurement natively on Windows.
5. Do not use the Windows working copy as the authoritative Git repository.
6. When ready to commit, sync only intended source/assets/evidence back into the WSL checkout.
7. Never copy/replace the .git directory from Windows.
8. Review status and diff from WSL.
9. Commit and push from WSL.
10. Leave the WSL checkout clean and pushed.

Do not overwrite unrelated changes if the WSL checkout is not clean.

## Suggested deliverable layout

Adapt if the existing Rust project structure strongly suggests a better layout, but prefer something close to:

    crates/
      mascot-animation/
      mascot-render-win32/

    apps/
      mascot-animation-lab/

    assets/
      mascot/
        rig-v0.2/
          rig.json
          ...

    benchmark/
      results/
        windows/
          animation-rig-v0.2/
            ...

or another clearly isolated evidence directory.

Do not put temporary build products into Git.

## Required evidence

Commit:

- rig v0.2 assets
- rig.json
- architecture notes
- pivot/bone visualization
- deterministic pose stress images
- contact sheet
- rest comparison/diff
- joint guard validation
- animation lab screenshots or a short capture if useful
- resource measurements
- known limitations

## Required clips

At minimum:

- blink
- double blink
- look left
- look right
- small head tilt
- ear twitch
- tail flick
- posture adjust
- stretch

Walking is optional only after seam-safe basic articulation is demonstrated.

## Stop / escalate conditions

Stop and report rather than papering over the issue if:

- the chosen per-part raster model cannot meet the safe rotation ranges without major visible degradation
- runtime post-composition outlining is not viable in the chosen native renderer without a substantial architecture change
- preserving the canonical style would require regenerating most visible artwork
- hidden geometry cannot be reconstructed credibly from the source without explicit art-direction input
- the renderer requires a continuous idle frame loop to work
- a Windows toolchain/admin prerequisite requires user intervention

A normal iteration/reconstruction pass is expected and is not a stop condition.

## Final report

Return:

1. starting WSL HEAD
2. final branch + HEAD
3. Windows environment/toolchain
4. rig architecture
5. exact asset decomposition
6. bone/pivot coordinate convention
7. outline implementation
8. shadow implementation
9. animation-runtime architecture
10. required clip status
11. pose stress result summary
12. visual defects still present
13. performance/resource table
14. files/commits created
15. sync-back procedure used
16. WSL clean/push confirmation

End with exactly one of:

    MASCOT_RIG_WIN_002_COMPLETE

or

    MASCOT_RIG_WIN_002_BLOCKED: <reason>
