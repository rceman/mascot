# Mascot rig v0.2 + native Windows animation runtime — architecture

Status: experiment deliverable for `docs/MASCOT_RIG_RUNTIME_SPEC_V0.2.md`.
Evidence: `benchmark/results/windows/animation-rig-v0.2/README.md`.

## Layout

```
Cargo.toml                         workspace (edition 2024, exact-pinned deps; `rust/` v0.1 app excluded)
crates/mascot-animation/           platform-neutral runtime: rig model, skeleton, attachments, clips, player, idle director
crates/mascot-render-win32/        Direct2D renderer (fills, runtime outline, runtime shadow) + swap-chain window target
  src/bin/stress.rs                deterministic pose-stress / joint-guard / rest-diff tool (`mascot-rig-stress`)
apps/mascot-animation-lab/         Win32 developer lab (native controls + Direct2D canvas), perf + capture automation
assets/mascot/rig-v0.2/            generated rig: rig.json, clips.json, parts/*.png
tools/rig_art/                     art decomposition pipeline (Python/numpy/scipy/Pillow), authored as data
```

No Electron, WebView, Tauri frontend, egui, iced, Slint, Skia or game engine. Rendering is Win32 +
D3D11 device + Direct2D 1.1 device context + DXGI flip-model swap chain (`windows` crate 0.62.2).

## 1. Art decomposition (`tools/rig_art`)

`decomposition.py` is the *authored* data (bones, pivots, safe ranges, part ownership polygons, z order,
hidden-geometry extension shapes, line-ownership overrides); `build_rig_v02.py` is the algorithm.
Regenerate with:

```
python -m venv scratch/venv && scratch/venv/Scripts/pip install numpy scipy pillow
scratch/venv/Scripts/python tools/rig_art/build_rig_v02.py [--debug scratch/dbg]
```

Pipeline, all in canonical canvas pixels (1254×1254):

1. **Silhouette / external stroke.** S = alpha ≥ 0.5. Pixels within `OUTLINE_RADIUS` (34.5 px, measured
   median of the canonical outer stroke: 35.4 px) of the background are the canonical *external* stroke and
   are **not stored** in any attachment; the runtime regenerates it. Stroke pieces the radius cannot
   reproduce (pointed tail tip, head-top step, whisker tips) are kept as line art of their owning part.
2. **Colour ownership.** Visible colour pixels are assigned to parts through priority-ordered polygons,
   restricted by per-part palette classes (so a lid polygon cannot steal arm orange). Anti-aliased
   black↔colour ramps are un-mixed against the nearest pure colour, so fills store flat colour and lines
   store coverage only.
3. **Internal line art.** Each internal black stroke is classified by its two nearest colour owners: a
   *single* stroke belongs to the higher part and the lower part fills beneath it; a *merged* double
   stroke (two contours fused, e.g. chin + laptop lid top) is split so each part carries a normal-width
   contour along its own edge. Explicit overrides handle semantics the geometry cannot infer (eye ovals
   belong to the eye bones so blink = eye `scale_y`; whiskers belong to the head; the ear's inner "C"
   belongs to the ear).
4. **Hidden geometry.** Authored extension shapes continue each part underneath the parts that cover it
   (arm under the lid, body under the head/ear/chin, thighs, tail root, far leg). Extensions are clipped to
   the region covered at rest, so they never change the rest pose. Each part also gets a *hidden contour*
   band along its concealed boundary, except where that boundary is an artificial ownership cut
   (`no_contour` zones), which would otherwise show as black wedges when exposed.
5. **Reconstruction of unseen parts.** `ear_far` is a mirrored 0.9× copy of the near ear placed just
   fully occluded behind the head; `arm_far` is a procedural capsule under the lid. Both are flagged
   `reconstructed: true`.
6. **Feathering.** Cut edges of an upper part that meet a lower part of the same colour with no line
   between them (neck, shoulder, hip, ear root) ramp alpha to 0 over 14 px, matched against the composite
   beneath, so motion shows no hard seam and rest is unchanged. Never within the silhouette margin.
7. **Output.** One `fill` and one `line` RGBA sprite per part (line sprites are black + coverage), tightly
   cropped, plus `rig.json`. The build self-checks: rest re-composite vs source, uncovered interior,
   clip-loss per extension.

Deliberately *not* done: redrawing the character, baked outlines per part, baked shadows.

## 2. `rig.json` (format `mascot-rig` 0.2.0)

- `coordinate_system`: canvas = source pixels, origin top-left, +x right, +y down; rotation degrees,
  positive = clockwise on screen; `world = parent_world · T(x,y) · R(rot) · S(sx,sy)`; bone origin ==
  rotation pivot; attachment `origin` = sprite top-left in the owning bone's local pixels; pivots are
  absolute pixels.
- `bones[]`: `id`, `parent`, `rest` transform (parent-local), `rest_world_pivot` (canvas),
  `safe_rotation_deg`, `note`. 25 bones. Topologically validated at load (out-of-order files accepted).
- `attachments[]`: `id`, `part`, `bone`, `role` (`fill` | `line`), `z`, `kind` (`sprite` | `mesh`), sprite
  `image`/`size`/`origin`/`canvas_rect`, `reconstructed`. `kind: mesh` (vertices, uvs, triangles, per-vertex
  bone weights) is already parsed and skinned by the runtime (unit-tested) but unused by v0.2 assets.
- `outline`: `radius_canvas_px`, model description, measured canonical stroke statistics.
- `shadow`: runtime-only.
- `joint_guards[]`: circles in a bone's local space that must stay covered while the listed bones sweep
  their ranges (`min_coverage` 0.995).
- `reconstructed`, `build_stats`: provenance and self-check numbers.

Skeleton (parent → children): `root → hips → {body → chest → {neck → head → {ear_near, ear_far, eye_left,
eye_right}, arm_near_upper → arm_near_lower → paw_near, arm_far_upper → arm_far_lower → paw_far},
tail, leg_near_upper → leg_near_lower → foot_near, leg_far_upper → leg_far_lower → foot_far,
laptop → laptop_screen}`. Both ears and both legs are independent; each leg has hip/knee/ankle bones
(walk-ready). The v0.2 art is rigid per part, so lower-arm/paw/knee/ankle bones have a zero safe range and
exist for future mesh attachments without changing the bone model.

`clips.json` (format `mascot-clips/0.2`): named one-shot clips of keyed tracks (`x`, `y`, `rotation`,
`scale_x`, `scale_y` per bone; `opacity` per part), per-key easing. Non-looping clips are validated to
start and end at rest. Clips: blink, double_blink, look_left, look_right, small_head_tilt,
ear_twitch, tail_flick, posture_adjust, stretch. All stay inside the declared safe ranges
(integration-tested against the shipped assets).

## 3. Runtime (`crates/mascot-animation`, no platform dependencies)

- `math::Affine` 2D affine; `skeleton::Skeleton` evaluates world transforms in topological order from a
  `Pose` (per-bone offsets from rest) and a `RootPlacement` (position, scale, **mirror**). Mirroring is a
  whole-rig `scale_x = -1` about the root pivot, so every attachment, the outline and the shadow mirror
  together.
- `attachment::Attachment` = `Sprite` (rigid, one bone) | `Mesh` (weighted, linear-blend skinned).
  Renderers consume a `DrawList` of world-space items; adding meshes later means adding a mesh draw path,
  not replacing bones/clips.
- `clip` / `player::Player`: additive clip evaluation, per-clip time, completion events; `is_static()`
  when nothing plays.
- `idle::IdleDirector`: deterministic (seeded) scheduler of occasional idle clips with cooldowns and
  no immediate repeats. It returns *the next wake-up delay*; it never ticks.

## 4. Renderer (`crates/mascot-render-win32`)

Per frame (only when something changed):

1. **Colour composite** → offscreen: fill and line sprites in z order, transformed by bone world ×
   view (linear interpolation, premultiplied).
2. **Fill silhouette** → offscreen: fill sprites only (line art excluded from the outline mask).
3. **Black silhouette**: colour-matrix effect, alpha sharpened (2a − 0.5).
4. **Dilation**: MAX-blend stamping of the black silhouette on a ring of radius R/2, twice
   (ring ⊕ ring = disc R), i.e. a true morphological dilation of the *composed* silhouette.
5. **Target**: background → runtime shadow (contact: silhouette squashed to the floor line + blur; or
   drop: offset blur; both from the composed silhouette, following the mirror) → outline → colour.

Because the outline is regenerated from the composed silhouette every frame, overlapping moving parts
never produce doubled external outlines, and a part that rotates outward gets outlined correctly.

`SwapChainTarget` wraps a DXGI flip-sequential swap chain on an HWND. `DeviceKind::Hardware` is used by the
lab; `DeviceKind::Warp` (software) makes stress renders deterministic across machines.

## 5. Animation lab (`apps/mascot-animation-lab`)

Win32 window with a native control panel (trackbars for head, each ear, each arm, each leg, tail, laptop
screen; a selected-bone slider; clip buttons; checkboxes for mirror, outline, shadow + style, bone overlay,
pivots, attachment bounds, idle director) and a Direct2D canvas child window.

Event-driven by construction:

- The canvas renders only in `WM_PAINT`, which is only requested (`InvalidateRect`) by control changes or
  by an animation frame.
- While a clip plays, each paint advances the player by the measured frame time and invalidates the
  canvas again. `Present(1)` paces this to the display refresh. When the player becomes static, the
  paint stops invalidating. There is no render or update loop while static.
- The idle director arms a single one-shot timer for its next action (seconds away); no polling.
- `--perf OUT.json` measures idle/active/after windows in-process; `--capture DIR` takes deterministic
  screenshots via the same renderer.

## 6. Build, test, run (native Windows)

```
cargo build --release --workspace
cargo test --release --workspace
target/release/mascot-rig-stress.exe --out benchmark/results/windows/animation-rig-v0.2
target/release/mascot-animation-lab.exe                       # interactive
target/release/mascot-animation-lab.exe --perf perf.json      # measured run, exits by itself
target/release/mascot-animation-lab.exe --capture shots/      # screenshots, exits by itself
```

In Git Bash, pose arguments to `mascot-rig-stress --probe` use `::` as the separator because a lone `/`
is rewritten into a path by MSYS.

## 7. Source/workflow used for this task

The WSL checkout is the Git authority. The source was copied (without `.git`) to a native Windows
working copy, implemented/built/tested/rendered there, then only intended files (sources, generated
assets, evidence, docs) were copied back into the WSL checkout with `cp` over `\\wsl$` paths, reviewed
with `git status` / `git diff` in WSL, and committed/pushed from WSL. `target/` and `scratch/` are
ignored and never synced.

## 8. Known limitations

See the evidence README for the visually reviewed state. In summary:

- Parts are rigid raster sprites. Bending (elbow, knee, spine) needs the mesh path, which exists in the
  runtime but has no authored meshes yet.
- The seams are clean only inside the declared safe ranges; beyond them hidden geometry runs out by design.
- `ear_far` and `arm_far` are reconstructions (never visible at rest); they are only lightly exposed by the
  safe ranges.
- Where the head's chin rests on the laptop lid, large head rotations visibly *change the contact*
  (the chin lifts off the lid or overlaps it). The contour stays clean, but this is a pose change, not
  an invisible seam.
- The runtime outline is a uniform-radius dilation; the canonical stroke varies slightly (p10 34 px, p90
  46 px), which is the main source of rest-diff pixels (mostly anti-aliasing along the outer contour).
- Shading gradients inside parts (dark-orange side shading) are carried by the part that owns them, so a
  rotated part shows a small shading discontinuity against a static neighbour.
- The lab UI is developer tooling (plain Win32 controls), not product UI.
