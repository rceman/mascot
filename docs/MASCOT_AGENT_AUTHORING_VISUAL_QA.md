# Mascot agent authoring + visual QA toolchain

Status: mandatory corrective addendum for MASCOT-RIG-WIN-002.

The first completion at commit `5013d602f11d4f4e0ba1e325ada156256a6e0b2c` is useful progress, but it is not the final visual-QA completion for the rig/runtime task.

Two issues must be corrected:

1. recurring authoring/inspection work was left as ignored `scratch/` scripts instead of reusable repository tooling;
2. the committed evidence contains static pose images/contact sheets, but not the complete animation video + frame-by-frame zoom inspection package needed to validate motion artifacts.

The known visual defects reported at head ±10° and leg ±15° also mean the task is not yet visually finished inside the currently claimed safe ranges.

## 1. Promote recurring work into durable tooling

Do not keep useful repeated workflows only in ignored `scratch/`.

The capabilities currently represented by helpers such as:

- zoom comparisons
- tinted diagnostic rows
- pose montages
- rebuild -> stress summary

must be promoted into repository-owned, documented tooling.

Prefer a stable Rust CLI named `mascotctl` where practical. A small committed Python helper is acceptable for image-analysis/compositing tasks that are materially simpler in Python, but it must be:

- committed
- documented
- deterministic
- reusable
- callable from one stable CLI/workflow

Do not make future agents reconstruct the same script from scratch.

## 2. Required authoring CLI capabilities

Exact syntax may vary, but the durable toolchain must provide equivalent capabilities to:

    mascotctl rig validate
    mascotctl rig inspect

    mascotctl pose render <pose>
    mascotctl pose stress
    mascotctl pose stress --bone <bone>

    mascotctl sweep rotation <bone> --from <deg> --to <deg> --step <deg>
    mascotctl sweep pivot <bone> ...

    mascotctl clip render <clip>
    mascotctl clip frames <clip>
    mascotctl clip video <clip>

    mascotctl inspect joints <clip-or-pose-set>
    mascotctl montage <artifact-set>
    mascotctl compare rest

    mascotctl bench idle
    mascotctl bench animation <clip>

Prefer concise human output and optional machine-readable JSON.

## 3. Data-driven tuning

Rig and clip tuning should not require Rust recompilation when only data changes.

Keep pivots, origins, safe ranges, attachment transforms, z-order, visibility, animation keyframes and clip timing in runtime-loaded data such as:

    assets/mascot/rig-v0.2/rig.json
    assets/mascot/rig-v0.2/animations/*.json

Changing this data should normally allow immediate rerender/reinspection.

## 4. Every animation must be rendered to frames and video

For every required clip:

- blink
- double blink
- look left
- look right
- small head tilt
- ear twitch
- tail flick
- posture adjust
- stretch

generate deterministic committed evidence.

At minimum commit:

    benchmark/results/windows/animation-rig-v0.2/animations/<clip>/
      <clip>.mp4
      contact-sheet.png
      zoom-full.mp4
      zoom-contact-sheet.png
      metadata.json
      joints/
        ... relevant zoomed joint evidence ...

If a clip is extremely short, a GIF is optional, but MP4 plus PNG inspection evidence remains preferred.

An external development-only `ffmpeg` encoder is acceptable. It must not become a product/runtime dependency.

## 5. Frame-by-frame QA is mandatory

Do not approve an animation merely because the video looks acceptable at playback speed.

Inspect frames individually.

For every frame, actively search for:

- transparent seams
- one-pixel/two-pixel background leaks
- detached components
- black wedges
- doubled contours
- stray black pixels
- stray fill pixels
- outline thickness discontinuities
- outline collapse at acute angles
- incorrect z-order
- pivot sliding
- clipping
- implausible exposed hidden geometry
- sudden shading discontinuities
- newly exposed body regions that were not reconstructed convincingly

Automated checks should inspect all frames where feasible, not only sampled frames.

Human/agent visual inspection should use contact sheets and zooms.

## 6. Zoom is required for both frames and videos

Normal-size evidence is insufficient for joint QA.

Produce useful magnified views, typically 4x-8x.

For each clip provide:

1. normal-size full-character video;
2. enlarged full-character inspection video;
3. tracked/fixed zoom video(s) around the moving joint(s);
4. zoomed frame/contact-sheet evidence.

Examples:

Head movement:
- neck/head joint
- chin/laptop relationship if the head can approach the laptop

Ear movement:
- ear root

Arm movement:
- shoulder
- elbow/paw if articulated
- arm/laptop overlap

Leg movement:
- hip/body
- knee/foot if articulated
- leg/laptop/body overlap

Tail:
- tail root/body

Stretch/posture:
- full-character enlarged view
- neck
- shoulders
- hips
- tail root
- any laptop-contact area affected

The tool should make these zooms automatically from rig-defined joint regions where possible.

## 7. Suspect-frame extraction

The toolchain should emit a small suspect set automatically.

Useful heuristics include:

- joint guard alpha failure or near-failure
- detached alpha components
- sudden frame-to-frame outline area jump
- unexpected alpha holes
- unexpected bounding-box jump
- high local diff around a joint
- z-order/coverage anomaly

Output suspicious frames to an easy-to-review directory and include their frame numbers in JSON.

These heuristics supplement visual review; they do not replace it.

## 8. Visual defects require art/rig correction, not documentation-only acceptance

If a motion exposes a defect inside the claimed safe range, fix the underlying art/rig.

Examples:

- head turn exposes a bad neck/chin seam -> reconstruct/extend neck/head/body geometry and retest;
- arm rotation exposes a hole -> extend hidden shoulder/arm geometry and retest;
- leg rotation exposes implausible thigh/body geometry -> redraw/reconstruct hidden leg/body overlap and retest;
- tail movement exposes a root gap -> extend tail/body hidden geometry and retest;
- outline produces a wedge/double edge -> fix fill/line split or runtime outline handling and retest.

Do not keep a safe range that visibly fails and call it complete.

If a particular pose is fundamentally incompatible with the current rigid-sprite model, either:

- improve the attachment/deformation model; or
- reduce the declared safe range to a visually clean range and explicitly record the reduced limit.

The declared safe range must mean visually acceptable.

## 9. Current known defects to revisit

The first completion report explicitly listed:

- head ±10°: chin visibly lifts off or overlaps the laptop lid;
- leg ±15°: laptop underside and far thigh become exposed as reconstructed shapes;
- rotated joints: side shading steps slightly;
- far ear and far arm are reconstructions hidden at rest.

Re-run these first with zoomed frame/pose inspection.

Do not assume guard coverage means visual quality is acceptable.

Guard tests only establish coverage. They do not prove that reconstructed art looks correct.

## 10. Evidence retention policy

Commit compact, useful evidence.

Required to commit:

- final MP4 for every required animation clip;
- final normal contact sheet for every clip;
- final zoom contact sheet for every clip;
- relevant per-joint zoom evidence;
- final static stress sheet;
- flagged/suspect frames that informed fixes or remain as documented limitations;
- machine-readable QA metadata/results;
- performance report.

Raw full-resolution PNG frame sequences may be omitted from Git if they are large and are exactly reproducible with a documented command.

If omitted, the committed tooling must reproduce them in one command.

Do not omit the final videos/contact sheets merely because they are generated artifacts; they are review evidence.

## 11. One-command iteration target

Aim for a workflow like:

    mascotctl qa clip stretch

which performs, as appropriate:

    validate rig
      -> render deterministic frames
      -> run all-frame checks
      -> generate normal video
      -> generate zoom video(s)
      -> generate contact sheets
      -> extract suspect frames
      -> write qa.json
      -> print concise verdict

And:

    mascotctl qa stress

for all declared static safe ranges.

The agent should then spend reasoning tokens on actual visual defects, not repeatedly recreating plumbing.

## 12. Completion bar

Do not report `MASCOT_RIG_WIN_002_COMPLETE` again until:

- reusable authoring/QA tooling is committed;
- ignored scratch helpers are no longer the only implementation of recurring workflows;
- all required clips have committed video evidence;
- all required clips have frame-by-frame zoom inspection evidence;
- current known head/leg/shading issues have been reworked or the safe range has been honestly reduced to clean limits;
- static stress and animation QA both pass the visually accepted ranges;
- final WSL commit is pushed and clean.
