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

## 8. Automatic visual artifact detector

The framework must not rely only on human/agent inspection. It should automatically detect and rank frames that are likely to contain rendering/rigging glitches.

This is especially important for defects that appear for only one or two frames and are easy to miss at normal playback speed.

The detector does not need to prove that an image is artistically perfect. Its job is to identify suspicious local changes with high recall and send those frames/joints to the agent for focused inspection.

### 8.1 Joint-local regions of interest

Define a QA region for every articulated connection, at minimum:

- neck/head
- ear roots
- shoulders
- arm/laptop contacts where applicable
- hips
- knees/feet when articulated
- tail root
- laptop/paw overlaps

Store these regions in rig/QA data so tooling can automatically crop the same physical joint across all poses/frames.

Prefer regions defined relative to bones/joints rather than fixed screen coordinates where practical.

### 8.2 Expected-coverage masks

For each joint, maintain an expected overlap/coverage relationship between the connected parts.

The detector should flag frames when a region that should be continuously covered suddenly contains:

- transparent pixels/background leaks;
- a narrow uncovered channel between parent and child;
- disconnected alpha islands;
- an unexpected hole;
- a sudden reduction in overlap area.

This is stronger than the existing simple guard test: evaluate the actual shape/topology of coverage, not only aggregate percentage.

### 8.3 Thin dark sliver / black-wedge detection

Detect the class of artifact described by the user: a thin dark feature suddenly appearing between connected body parts, for example between head and torso during rotation.

Within each joint ROI:

1. identify very dark pixels / outline-like pixels;
2. compare their connected components and geometry against the rest/reference neighborhood;
3. flag newly appearing thin elongated components, wedges, spikes or channels that were not present in the rest pose or adjacent frames;
4. score them higher when they are surrounded by body-color pixels and lie on an internal joint where no intentional line should exist.

Useful morphology/signals include:

- component width/thickness;
- aspect ratio;
- skeleton length;
- acute triangular/wedge shape;
- sudden component appearance/disappearance;
- distance from the expected final external silhouette;
- whether the dark component lies inside the composed mascot rather than on its exterior boundary.

Do not classify every black line as a defect: intentional internal line art must be represented in an allowed/reference mask so it can be excluded.

### 8.4 Unexpected internal-outline detection

Because the final external outline is generated from the composed silhouette, a new black line entirely inside the mascot around a moving joint is usually suspicious unless it is intentional line art.

Build an internal-dark-pixel mask after excluding:

- the final external outline band;
- known internal line-art masks;
- laptop/logo details and other explicit dark artwork.

Flag new dark components appearing in the remaining interior around articulated joints.

This directly targets doubled outlines and hidden per-part contour fragments.

### 8.5 Temporal continuity checks

Compare each frame not only with the rest pose but also with neighboring frames.

For a smooth animation, local image changes should normally evolve smoothly.

Flag:

- a one-frame spike in dark-pixel count;
- sudden alpha-area jumps;
- a component that appears for one or two frames then disappears;
- abrupt local bounding-box changes;
- abrupt outline-length changes;
- sudden centroid jumps;
- sudden local color/shading changes inconsistent with bone motion.

Use thresholds tolerant enough for anti-aliasing and legitimate overlap changes.

### 8.6 Exterior-vs-interior contour topology

The composed mascot should normally have one coherent exterior silhouette.

Track:

- number of exterior connected components;
- number/area of enclosed holes;
- contour length;
- small protrusions/spikes;
- local curvature extremes.

Flag unexpected changes, especially when a new tiny component, hole, spike or notch appears near a joint.

### 8.7 Color/shading continuity

At same-color body joints, compare the colors immediately across the seam.

Flag unusually strong local discontinuities that appear only after rotation, especially:

- narrow dark bands;
- abrupt shade steps;
- exposed reconstruction with a palette/color not matching either neighboring region;
- isolated pixels outside the allowed mascot palette beyond expected antialiasing.

This should be local and joint-aware; do not globally reject intentional shading.

### 8.8 Motion-aware hidden-geometry validation

When a child part rotates away from its rest overlap, verify that newly exposed pixels belong to plausible reconstructed hidden geometry.

The tool should know which attachment becomes exposed at a given joint and can compare the exposed region against:

- allowed palette;
- expected part mask;
- neighboring part colors;
- whether any raw cut edge of the original flattened extraction became visible.

A straight cut line or abrupt rectangular boundary becoming visible should be scored as a likely reconstruction defect.

### 8.9 Multi-signal suspicion score

Do not make the whole QA system depend on one brittle threshold.

Produce a per-frame/per-joint suspicion score from several signals, for example:

- alpha gap;
- new internal dark component;
- thin-sliver geometry;
- detached component;
- contour topology change;
- temporal spike;
- shading discontinuity;
- exposed cut edge.

Store both the combined score and individual reasons.

Example machine-readable result:

    {
      "frame": 31,
      "joint": "neck",
      "score": 0.91,
      "reasons": [
        "new_internal_dark_sliver",
        "temporal_dark_pixel_spike",
        "unexpected_joint_gap"
      ],
      "artifacts": [
        "suspects/neck/frame-0031.png",
        "suspects/neck/frame-0031-x8.png"
      ]
    }

The exact scoring model can be heuristic. It does not need ML.

### 8.10 Automatic zoom/crop evidence for every suspect

For every flagged frame, automatically emit:

- full frame;
- joint crop;
- 4x-8x nearest-neighbor or crisp diagnostic zoom;
- alpha-only view;
- dark-pixel/outline diagnostic view;
- previous/current/next frame triptych;
- optional overlay showing the detector's suspicious component.

This should let the agent understand the failure immediately without writing another crop script.

### 8.11 Severity and completion behavior

Classify at least:

- ERROR: definite hole/detachment/background leak or severe contour failure;
- WARN: likely dark sliver/double outline/shading artifact requiring inspection;
- INFO: unusual but probably legitimate change.

A clip must not receive a clean visual-QA verdict while ERROR findings remain.

WARN findings must be explicitly reviewed. The agent must either:

- fix the rig/art/runtime and rerun QA; or
- mark the finding as intentional/false-positive with a narrow machine-readable suppression tied to that specific joint/art feature.

Do not suppress an entire detector globally to make the report green.

### 8.12 Baseline/suppression model

Intentional static details must not generate repeated noise.

Allow narrow baselines/suppressions based on:

- canonical rest artwork;
- explicit internal-line masks;
- named joint/art feature;
- bounded ROI and reason.

Every suppression must be committed as data and documented.

The default should remain fail/open-to-review for newly appearing artifacts.

### 8.13 Target CLI behavior

The preferred one-command QA flow:

    mascotctl qa clip stretch

should include the automatic artifact detector and finish with a concise summary such as:

    72 frames rendered
    8 joints analyzed
    0 ERROR
    2 WARN
      neck frame 31: dark sliver score 0.91
      shoulder_near frame 44: shading discontinuity score 0.67
    evidence: ...

And:

    mascotctl qa stress

should run the same detector across all static safe-range poses.

The key design goal is that future agents should spend tokens correcting visual defects, not discovering how to detect/crop them.

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
