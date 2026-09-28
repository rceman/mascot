# Mascot agent authoring format and tool-performance contract

Status: mandatory design correction for the agent-facing animation workflow.

The current runtime clip schema is intentionally explicit, but the shipped `assets/mascot/rig-v0.2/clips.json` is too verbose for repeated agent authoring. It duplicates targets, property names, key object field names and symmetric tracks. That wastes context/tokens and increases editing error rate.

The agent-facing authoring representation and the runtime-normalized representation do not need to be identical.

## 1. Design principle

Use:

    compact authoring data
        -> validate/expand in memory (or compile as a dev artifact)
        -> normalized runtime structures

Do not force an agent to repeatedly emit verbose normalized JSON.

The authoring format must optimize for:

- low token count
- low repetition
- easy diffs
- easy mechanical editing
- obvious semantics
- deterministic expansion
- strong validation
- no hidden magic that makes animation behavior hard to reason about

Do not replace readable data with a cryptic binary or opaque DSL merely to save bytes.

## 2. Required compact clip format

Introduce a compact authoring format (recommended format id: `mascot-clips/0.3`) with at least:

- named target groups
- compact key tuples
- short but readable clip fields
- defaults for common easing
- target.property addressing
- optional reusable curves/key sequences where they materially reduce repetition

Recommended shape:

    {
      "format": "mascot-clips/0.3",
      "defaults": {
        "ease": "in_out_sine"
      },
      "groups": {
        "eyes": ["eye_left", "eye_right"]
      },
      "clips": {
        "blink": {
          "duration": 0.26,
          "tracks": {
            "@eyes.scale_y": [
              [0.00, 1.00, "in_quad"],
              [0.08, 0.08, "linear"],
              [0.12, 0.08, "out_quad"],
              [0.26, 1.00]
            ]
          }
        }
      }
    }

This should expand deterministically to the same logical runtime tracks as the current verbose form.

## 3. Target groups

Allow named groups such as:

    "groups": {
      "eyes": ["eye_left", "eye_right"],
      "ears": ["ear_near", "ear_far"],
      "arms": ["arm_near_upper", "arm_far_upper"],
      "legs": ["leg_near_upper", "leg_far_upper"]
    }

A grouped track:

    "@eyes.scale_y": [...]

means: apply the same track to every member.

Do not require duplicate left/right tracks when their curve is identical.

Groups must validate:

- referenced ids exist;
- no recursive group cycles;
- property is valid for all targets.

## 4. Compact key tuples

Use tuples instead of repeated objects.

Preferred:

    [time, value]
    [time, value, ease]

rather than:

    {"t": 0.08, "v": 0.08, "ease": "linear"}

Default easing should be defined once and applied when omitted.

Document clearly whether easing belongs to the segment starting at a key or ending at a key. Keep the existing runtime semantics unless there is a strong reason to change them.

## 5. Track addressing

Prefer a concise, readable key:

    "<target-or-group>.<property>"

Examples:

    "head.rotation"
    "@eyes.x"
    "@eyes.scale_y"
    "tail.rotation"
    "root.y"

The parser should split on the final/property delimiter in a deterministic, validated way.

Do not abbreviate properties to single letters if doing so materially hurts readability. Token efficiency should remove structural repetition, not semantic clarity.

## 6. Reusable curves / shared key sequences

If the same curve is used repeatedly, allow a named reusable curve.

Example:

    "curves": {
      "blink_once": [
        [0.00, 1.00, "in_quad"],
        [0.08, 0.08, "linear"],
        [0.12, 0.08, "out_quad"],
        [0.26, 1.00]
      ]
    }

and:

    "@eyes.scale_y": {"curve": "blink_once"}

Only add this if it keeps the format simple. Do not build a general programming language.

## 7. Symmetry / derivation

Avoid blindly duplicating complete clips when a safe deterministic derivation exists.

A limited mirror/derive feature is acceptable for clearly symmetric cases, but only if the transformation rules are explicit and testable.

For example, a future:

    "look_right": {
      "derive": "look_left",
      "mirror_motion": true
    }

must have precisely defined behavior for:

- sign of x translation
- sign of rotation
- near/far target swapping
- any asymmetric target exceptions

Do not implement this until it is less error-prone than authoring the explicit compact tracks.

Target groups and compact tuples are mandatory; clip derivation is optional.

## 8. Runtime compatibility

The Rust animation runtime may:

- parse the compact format directly into normalized `Clip` / `Track` structures; or
- use `mascotctl compile clips` to expand it.

Do not require a generated normalized JSON file to be committed unless it has a clear runtime/deployment purpose.

Prefer one canonical authoring source of truth.

If a normalized representation is generated, it must be deterministic and reproducible.

## 9. Migration requirement

Migrate the existing required clips from verbose `mascot-clips/0.2` authoring to the compact format without changing their intended motion unless a visual-QA correction explicitly requires it.

Add tests proving that:

- group expansion is correct;
- tuple parsing is correct;
- omitted ease uses the declared/default easing;
- clip validation still enforces rest endpoints and safe ranges;
- compact blink expands to both eyes correctly;
- invalid group/target/property combinations fail clearly.

## 10. Agent-oriented CLI

The CLI should make common inspection/editing tasks concise.

Examples:

    mascotctl clip show blink
    mascotctl clip show blink --expanded
    mascotctl clip validate
    mascotctl clip render blink
    mascotctl qa clip blink

`clip show` should default to the compact authoring view, not dump verbose normalized structures.

Use `--expanded` only when debugging the runtime expansion.

## 11. Avoid shell/Python inspection boilerplate

A command such as:

    cd ... && python -c "import json; ..."

should not be the normal way for an agent to inspect a clip.

Provide dedicated commands that emit only the relevant data.

The tooling should support narrow output so the agent can inspect one clip/track/joint without loading the entire authoring file into context.

Examples:

    mascotctl clip show blink
    mascotctl clip track blink @eyes.scale_y
    mascotctl rig bone head
    mascotctl rig joint neck

## 12. Tool performance is part of the contract

The authoring/QA framework exists to reduce iteration cost. A correct tool that takes minutes for routine local work is still a workflow failure.

Measure and optimize the common commands on the native Windows development machine.

### 12.1 Fast-path budgets

For a warm development environment, target approximately:

- `mascotctl rig validate`: <= 0.25 s
- `mascotctl clip validate`: <= 0.25 s
- `mascotctl clip show <name>`: <= 0.10 s
- render one static pose: <= 0.5 s
- render one short clip to frames (without video encoding): <= 2 s where practical
- build one contact sheet from existing frames: <= 1 s
- artifact analysis of one short clip: <= 2 s where practical
- complete `qa clip <short-clip>` excluding external video encoding: target <= 5 s
- complete static stress suite: target <= 10 s

These are engineering targets, not excuses to skip correctness. If a specific operation legitimately exceeds them, profile it and document why.

A routine authoring command taking ~30-120 seconds is considered a performance defect unless the work is inherently expensive and unavoidable.

### 12.2 Video encoding

Video encoding may be slower than pure render/analysis, but it must not dominate interactive iteration unnecessarily.

Requirements:

- render frames once and reuse them for video/contact-sheet/analysis;
- do not rerender the clip separately for every artifact;
- use an appropriate fast development ffmpeg preset;
- allow `qa clip --no-video` or equivalent for the fastest tuning loop;
- allow final evidence generation to add MP4 after the clip passes analysis.

Target final MP4 generation for a short clip: a few seconds, not minutes, on the development machine.

### 12.3 Incremental/cache behavior

Cache expensive deterministic intermediates by content hash or equivalent where useful:

- decoded source textures;
- expanded clip data;
- rendered frame sequence for unchanged rig+clip+renderer inputs;
- analysis masks;
- contact-sheet inputs.

Changing only QA presentation should not force art decomposition or a Rust rebuild.

Changing only clip data should not force Cargo compilation.

Changing only one clip should not rerender all clips.

### 12.4 Avoid process-startup bottlenecks

Do not create a pipeline that launches Python/ffmpeg/Rust dozens or hundreds of times per frame.

Prefer:

- one renderer process per requested job;
- batch frame rendering;
- batch analysis;
- one ffmpeg invocation per video.

Repeated process startup is measurable overhead and makes agent workflows noisy.

### 12.5 Parallelism

Parallelize independent work where it is safe and useful:

- independent clip QA;
- static pose renders;
- per-joint analysis after frames are available.

Do not introduce nondeterministic output ordering.

### 12.6 Profiling requirement

Add a lightweight timing report, for example:

    mascotctl qa clip stretch --timings

Output:

    parse/validate       8 ms
    frame render       820 ms
    artifact analysis  340 ms
    contact sheets     110 ms
    video encode      1.8 s
    total             3.1 s

The final task report must include timings for the main authoring commands and identify the slowest stages.

If any routine command exceeds 10 seconds, profile before declaring completion.

If any routine command exceeds 30 seconds, treat it as a blocker/performance bug unless explicitly justified.

## 13. Token-efficiency acceptance criterion

The compact authoring format should materially reduce representation size compared with the current verbose `mascot-clips/0.2` file.

Measure at least:

- bytes
- approximate token count
- repeated-field count / structural duplication

Report the before/after result for the shipped clip library.

Do not optimize solely for the smallest possible token count. The target is a substantial reduction while keeping the format obvious enough for agents and humans to edit safely.

## 14. Completion condition

This authoring-format correction is complete only when:

- compact clip authoring is implemented and documented;
- existing clips are migrated;
- runtime semantics are preserved or intentional motion changes are documented;
- group expansion and compact keys are tested;
- dedicated CLI inspection replaces ad-hoc Python for normal workflows;
- common commands are benchmarked;
- no routine local authoring operation has an unexplained large bottleneck;
- timing output is available;
- before/after authoring token-size evidence is recorded.
