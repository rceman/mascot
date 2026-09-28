# Mascot rig + native Rust animation runtime specification v0.2

Status: implementation specification for the next Windows experiment.

This supersedes the rejected first-pass layered-rig approach as the target architecture.

## 1. Objective

Create a production-oriented 2D skeletal mascot rig plus a minimal native Rust/Windows animation lab.

The task is successful when the mascot can perform useful small rotations and simple clips without:

- transparent joint holes
- visible seams
- detached limbs
- black wedge artifacts
- doubled external outlines
- broken z-order
- visible background leaks

Building code is not sufficient. Visual deformation quality is a primary acceptance criterion.

The canonical visual reference is the approved repository asset:

    assets/mascot.png

## 2. Lessons from rejected rig v0.1

A first decomposition experiment proved that a flattened PNG can be split and recomposed exactly in the default pose, but it also exposed the wrong abstraction for animation.

Observed failure mode:

- body-part PNGs carried finished black contours
- joints had insufficient hidden overlap/reconstructed geometry
- small rotations exposed gaps and black wedges
- independently outlined pieces produced doubled contours

Therefore:

- exact default-pose recomposition alone is not enough
- visible canonical artwork should be preserved where possible
- hidden geometry must be reconstructed for motion
- final external silhouette outlining must happen after transformed fills are composited
- internal line art and external silhouette outline must be treated separately

Do not preserve the v0.1 decomposition merely because it existed. Redesign boundaries where necessary.

## 3. Art decomposition

Desired logical attachments, at minimum:

    root
      hips
        body/chest
          neck
            head
              ear_far
              ear_near
              eye_left
              eye_right
              eyelid/blink support
        tail
        leg_far
        leg_near
        arm_far
        arm_near
        laptop
          laptop_screen

The runtime skeleton may use richer bone chains than the initial sprite decomposition.

Recommended future-capable skeleton:

    root
    └── hips
        ├── body
        │   └── chest
        │       └── neck
        │           └── head
        │               ├── ear_far
        │               ├── ear_near
        │               ├── eye_left
        │               └── eye_right
        ├── tail
        ├── leg_far_upper
        │   └── leg_far_lower
        │       └── foot_far
        ├── leg_near_upper
        │   └── leg_near_lower
        │       └── foot_near
        ├── arm_far_upper
        │   └── arm_far_lower
        │       └── paw_far
        └── arm_near_upper
            └── arm_near_lower
                └── paw_near

Laptop may be anchored to hips/body or represented as an independent sibling with explicit constraints in the default pose. Keep the model simple in v0.2.

### Asset granularity

Initial raster attachments may be coarser than the bone hierarchy.

For example:

- one rigid leg_near sprite can initially follow leg_near_upper
- lower-leg/knee/foot bones can exist in the schema before weighted mesh deformation is implemented

The goal is to avoid an incompatible skeleton redesign when walking is added later.

## 4. Hidden geometry reconstruction

The flattened canonical PNG does not contain pixels hidden beneath overlapping parts.

Any part that can rotate away from an overlap must be extended underneath its neighbor.

Examples:

- neck/head overlap
- body beneath arms
- shoulders beneath arms
- leg roots beneath body
- tail root beneath body
- arms behind/in front of laptop
- laptop portions hidden by paws

Rules:

1. Preserve visible source pixels from assets/mascot.png wherever practical.
2. Reconstruct/redraw only missing hidden geometry or art that must be separated.
3. Hidden extensions should overlap generously enough for the declared safe rotation range.
4. Do not add finished external black borders to hidden joint extensions.
5. Do not optimize hidden geometry for one stress pose only; it must behave across the declared range.

The exact art tooling is not prescribed. Image editing/generation may assist hidden reconstruction, but visible canonical art should not be casually regenerated.

## 5. Outline model

This is a core architectural decision.

### 5.1 Fill composition

Render colored mascot fill attachments first, without treating every attachment as an individually finished black-sticker silhouette.

Conceptually:

    transformed fill layers
        -> offscreen mascot alpha/color composition

### 5.2 External outline

Generate one external outline from the final composited mascot alpha/silhouette.

Conceptually:

    final_alpha
      -> dilate/expand alpha
      -> subtract/or mask original alpha
      -> render black outline beneath final color

The implementation can use Direct2D effects, an offscreen bitmap pass, a shader, or another lightweight native technique. Choose the simplest robust native method.

The exact method is less important than these observable properties:

- continuous outer silhouette
- no doubled border at internal moving joints
- stable thickness at normal display scale
- correct behavior after horizontal mirror
- correct behavior under small rotations/scales

### 5.3 Internal line art

Internal black details are distinct from the final external silhouette.

Examples:

- mouth/muzzle line
- whisker marks
- nose details
- ear interior
- intentional limb crease
- laptop logo/details

These may remain raster/vector overlays or belong to specific attachments.

Do not let an outer-outline algorithm erase intentional internal line art.

## 6. Shadows

No baked shadow asset.

Runtime-render:

- mascot contact/drop shadow if used
- bubble shadow later
- optional laptop-screen activity glow later

Shadow should derive from current transformed geometry/silhouette so it remains correct during:

- flip
- scale
- head/body motion
- future walking

For the animation lab, implement a minimal runtime shadow toggle suitable for validating the pipeline.

## 7. Coordinate and pivot contract

Every attachment/bone must have explicit machine-readable coordinates.

rig.json must include at least:

- format/version
- source canvas width/height
- canonical source asset
- bone id
- parent id
- rest local translation
- rest local rotation
- rest local scale
- pivot/origin
- attachment id/path
- attachment local origin
- z-order
- default visibility
- safe initial rotation range
- mirror policy if special handling is needed
- optional joint guard definition

Coordinate spaces must be documented.

Suggested convention:

- canonical canvas coordinates: pixels, origin top-left
- bone local coordinates: parent-local
- rotations: degrees or radians, explicitly stated
- positive rotation direction: explicitly stated
- normalized or pixel pivots: choose one and use consistently

Produce a pivot/bone debug view to visually verify every origin.

## 8. Horizontal mirror

The character should not need duplicate left/right art.

Support root-level horizontal mirror.

Requirements:

- all attachments mirror coherently
- pivots/bones remain correct
- external outline remains correct
- shadow remains correct
- z-order remains semantically correct
- hit geometry can later use the same root transform

Future product rule: mascot generally faces inward/toward the bubble.

## 9. Minimal animation runtime

Implement as a small reusable Rust crate, not code embedded only in the lab window.

Suggested responsibilities:

### Bone

    id
    parent
    rest transform
    animated local transform
    world transform

Transform supports:

- translation
- rotation
- scale

### Attachment

v0.2:

- SpriteAttachment

Fields should include:

- image/texture reference
- owning bone
- local origin/offset
- z
- opacity

Design the attachment enum/data model so a future MeshAttachment can be added without rewriting the animation system.

### Animation clip

Support:

- named clip
- duration
- per-bone/property tracks
- keyframes
- simple easing
- optional looping where explicitly needed

Properties initially:

- x/y translation
- rotation
- x/y scale
- opacity

Do not implement a full editor, IK solver or Spine clone in this task.

### Player

Support:

- play
- stop
- reset/rest
- time advance
- clip completion
- static state detection

When no clip/interaction is active, the runtime must not require a permanent frame loop.

## 10. Initial animation clips

Required:

- blink
- double blink
- look_left
- look_right
- small_head_tilt
- ear_twitch
- tail_flick
- posture_adjust
- stretch

Optional only after core quality is good:

- tiny laptop adjust
- rough walk-cycle experiment

Do not prioritize walking over seam-safe articulation.

## 11. Idle director direction

The full product IdleDirector is not required in v0.2, but runtime design must support it.

Intended model:

- one-shot wakeup/timer
- weighted action selection
- randomized cooldown
- repetition guards
- contextual suppression
- play short clip
- return to static/no-frame state

Avoid a permanently running 60 Hz idle loop.

## 12. Native Windows implementation constraints

Target now: Windows only.

Language:

- Rust

Preferred native graphics architecture:

- Win32 host/window
- Direct2D
- DirectComposition where useful
- WIC for image decode if needed

Do not use:

- Electron
- Chromium
- WebView
- Tauri frontend
- egui
- iced
- Slint
- Skia
- Unity/Godot/other game engine
- a browser-based editor/runtime

The point of this experiment is a small native runtime suitable for an ultra-light always-present desktop mascot.

## 13. Animation Lab

Create a Windows-only developer application.

Suggested location:

    apps/mascot-animation-lab/
    crates/mascot-animation/
    crates/mascot-render-win32/

Exact layout may adapt to the existing repository structure if there is a cleaner fit.

The lab must allow:

- load/display rig
- reset to rest pose
- mirror horizontally
- toggle external outline
- toggle runtime shadow
- toggle bone debug overlay
- toggle pivot markers
- toggle attachment bounds
- select a major bone/attachment
- interactively rotate major parts
- play required clips

Major interactive controls:

- head
- each ear
- each arm
- each leg
- tail
- laptop if useful

The lab is developer tooling, not product UI. It may use simple native controls for sliders/buttons if that keeps implementation small.

## 14. Deterministic pose stress suite

Automate deterministic renders for at least:

Rest:
- canonical rest
- runtime rest

Head:
- -10
- -5
- +5
- +10 degrees

Each ear:
- useful small negative rotation
- useful small positive rotation

Each arm:
- -15
- -10
- +10
- +15 degrees

Each leg:
- -15
- -10
- +10
- +15 degrees

Tail:
- -20
- -10
- +10
- +20 degrees

Mixed:
- several representative mixed poses

Mirror:
- mirrored rest
- at least one mirrored mixed pose

Render individual outputs plus a contact sheet.

## 15. Joint guard tests

Where practical define joint guard regions that are expected to remain visually covered across a safe rotation range.

Machine-readable guard may specify:

- joint id
- region
- allowed pose/range
- expected minimum alpha/coverage

Automated validation should detect obvious transparent holes/background leaks in those guards.

Automated alpha tests are supplementary. Human visual review of the stress sheet remains required.

## 16. Rest-pose comparison

Produce:

- canonical source
- runtime rest render
- alpha diff
- RGBA/color diff
- documented intentional differences

Unlike rejected v0.1, exact pixel equality is not the only goal because runtime-generated outer outline can legitimately change antialiasing.

The important standard is:

- visually faithful to the canonical mascot at intended display size
- no gratuitous redraw/style drift
- any deliberate visible difference documented
- animation correctness not sacrificed to game an exact diff metric

## 17. Performance requirements

Measure the animation lab, with the mascot visible.

At minimum report:

- idle memory
- idle CPU
- threads
- handles where useful
- active-animation CPU
- frame cadence during active clip
- behavior after animation completes

Required qualitative property:

    static -> no continuous render/update loop

A timer used only to wake for an animation or explicit UI interaction is acceptable.

## 18. Source/workflow constraint for this task

The authoritative Git checkout remains in WSL.

Implementation/build/test execution happens on native Windows.

The user will provide the exact WSL repository path before execution.

Required workflow once the path is known:

1. Verify WSL checkout branch/head/worktree.
2. Create a disposable/working Windows copy of the source without treating it as the Git authority.
3. Implement in the Windows copy.
4. Build/test/run/render on native Windows.
5. Before committing, sync only intended source/assets/evidence back to the authoritative WSL checkout.
6. Review git status, diff and generated evidence from WSL.
7. Commit and push from the WSL checkout.
8. Never replace/copy the .git directory from the Windows working copy.
9. Do not overwrite unrelated WSL changes.
10. Leave both workspaces in an understandable state; WSL must be clean at completion.

The exact copy/sync mechanism is implementation-defined but must preserve file metadata sufficiently for this repository and must be documented.

## 19. Required deliverables

At minimum:

- rig v0.2 assets
- rig.json
- native Rust animation runtime crate
- Windows renderer
- Windows animation lab
- required animation clips
- pose-stress individual renders
- pose-stress contact sheet
- pivot/bone visualization
- rest comparison/diff evidence
- joint guard results
- performance report
- architecture/readme
- known limitations

## 20. Acceptance criteria

The task is accepted only if all of the following are true:

1. Rest pose is visually faithful to the approved mascot.
2. Useful small head/ear/arm/leg/tail rotations do not show obvious seams, holes, detached pieces, double external outlines or black wedges.
3. External outline is generated after composition rather than independently baked around every moving part.
4. Shadows are runtime effects.
5. Pivots/origins and coordinate spaces are explicit in rig.json.
6. Rig can be mirrored as a whole.
7. Runtime is designed to accept mesh attachments later.
8. Leg skeleton is compatible with future walking.
9. Static mascot does not require a continuous render loop.
10. Windows lab builds/runs/tests natively.
11. Stress evidence is committed.
12. WSL authoritative checkout is clean and pushed at the end.
