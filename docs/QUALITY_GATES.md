# Quality gates

Status: repository-wide gate contract.

This gate model adapts the useful separation from the GPT Tunnel Gateway project to Mascot: fast worker checks, canonical deterministic correctness, explicit visual QA, and explicit native performance.

The important rule is that different gates answer different questions. A fast focused test is not a substitute for a canonical full test; an alpha-coverage check is not a substitute for visual QA; deterministic tests are not a substitute for native runtime performance measurement.

## 1. Canonical command surface

The target project-owned interface is:

    mascotctl gate format
    mascotctl gate check
    mascotctl gate test
    mascotctl gate qa-static
    mascotctl gate qa-animation
    mascotctl gate perf
    mascotctl gate task

During the current bootstrap, an equivalent repository-owned script may temporarily front these commands, but the final preferred interface is `mascotctl`.

`gate task` is the canonical exact-candidate completion gate for the current task profile. It orchestrates the required deterministic gates without silently skipping expensive lanes.

Gate output must be concise for agents and support machine-readable JSON.

Recommended:

    mascotctl gate task --json <path>

## 2. Gate: format

Purpose: formatting only.

Expected checks include all relevant changed source/tooling files, for example:

- `cargo fmt --check`;
- Python formatting/syntax policy if Python remains in durable tooling;
- deterministic JSON/data formatting if the repository defines one.

Properties:

- deterministic;
- fast;
- no network;
- no rendering;
- no performance measurement.

Target warm runtime: <= 1 second where practical.

## 3. Gate: check

Purpose: cheap structural/static correctness.

Expected checks:

- `cargo check` for applicable workspace targets;
- rig/clip schema validation;
- required project files/specs present;
- no stale generated/canonical-format conflict;
- hard-cut migration policy checks;
- no forbidden compatibility shim for superseded internal formats;
- no obvious committed temporary/debug artifacts;
- narrow static architecture invariants that can be checked cheaply.

For the current clip migration, this gate must verify the final state:

    canonical clip authoring: mascot-clips/0.3
    runtime supported authoring format: mascot-clips/0.3
    runtime 0.2 compatibility: none

Documentation/evidence may mention 0.2 for historical comparison; runtime/source compatibility code may not.

Target warm runtime: a few seconds, preferably <= 5 seconds.

## 4. Gate: test

Purpose: canonical deterministic code correctness.

Run the full deterministic Rust test corpus for the materially final candidate.

Rules:

- no network-dependent/live UI automation hidden inside this gate;
- no machine-speed pass/fail thresholds;
- no performance SLO assertions based on host load;
- no visual-quality claims based solely on unit tests;
- test the compact authoring parser/expansion and rig/animation invariants;
- test runtime state and event-driven stop behavior where deterministic.

This is the equivalent of GTW's canonical task correctness lane.

During implementation, agents should use focused tests. Before completion, run this canonical full test gate once on the materially final candidate and rerun only if materially invalidated.

## 5. Gate: qa-static

Purpose: deterministic static rig deformation QA.

Run:

- canonical rest comparison;
- all declared safe-range stress poses;
- joint coverage/guard checks;
- automatic artifact detector;
- normal contact sheet;
- zoomed joint evidence;
- mirrored stress cases.

The artifact detector must inspect:

- gaps/background leaks;
- thin dark slivers;
- black wedges/spikes;
- doubled internal outlines;
- detached components;
- contour topology anomalies;
- unexpected shading discontinuities;
- exposed raw cut edges;
- z-order/coverage anomalies.

Pass policy:

- zero unresolved ERROR;
- every WARN reviewed;
- suppressions must be narrow, explicit and committed as data;
- declared safe ranges must match visually clean ranges.

## 6. Gate: qa-animation

Purpose: dynamic frame-by-frame visual QA.

For every required animation clip:

- render deterministic frame sequence;
- analyze every frame;
- run joint-local artifact detection;
- generate normal MP4;
- generate enlarged/zoom MP4;
- generate contact sheet/spritesheet;
- generate zoom contact sheet;
- extract suspect frames;
- emit QA JSON with per-frame/per-joint findings.

Required clips for the current rig task:

- blink;
- double blink;
- look left;
- look right;
- small head tilt;
- ear twitch;
- tail flick;
- posture adjust;
- stretch.

Pass policy:

- zero unresolved ERROR;
- every WARN explicitly reviewed/fixed/suppressed narrowly;
- evidence regenerated after the final relevant art/rig/runtime change.

Raw full frame sequences may remain reproducible build artifacts rather than Git content if final videos/sheets/suspect evidence and deterministic reproduction commands are committed.

## 7. Gate: perf

Purpose: explicit native Windows iteration/runtime performance measurement.

This is a separate lane because host load affects timings.

Record:

- Windows version;
- CPU/GPU identity where practical;
- Rust/tool versions;
- command timings;
- lab idle memory/CPU/threads/handles where applicable;
- active-animation resource behavior.

Authoring-tool targets are defined in `docs/MASCOT_AGENT_AUTHORING_FORMAT.md`.

Important policy:

- routine operations >10 s require profiling;
- routine operations >30 s are a performance defect/blocker unless explicitly justified;
- routine 30-120 s authoring commands are not acceptable merely because they eventually succeed;
- video encoding should reuse already-rendered frames;
- data-only edits must not force Cargo rebuild;
- changing one clip must not rerender every clip.

Performance thresholds do not belong in `gate test`.

## 8. Gate: task

Purpose: exact-candidate completion gate.

For the current rig/runtime task, `gate task` should execute or verify fresh success for:

    format
    check
    test
    qa-static
    qa-animation
    perf

It should produce one concise summary and one machine-readable receipt.

Recommended receipt fields:

- repository HEAD/tree identity;
- gate profile/version;
- platform/environment identity for platform-sensitive lanes;
- each gate status;
- duration;
- evidence paths;
- WARN/ERROR counts for visual QA;
- slow-stage summary.

Do not reuse a receipt after the candidate tree or relevant input data changes.

## 9. Fast worker loop

During implementation, prefer the narrowest useful command.

Examples:

    mascotctl clip validate
    mascotctl clip show blink
    mascotctl qa clip blink --no-video
    mascotctl qa stress --bone head
    cargo test -p mascot-animation <focused-test>

Do not run `gate task` after every small pivot/keyframe tweak.

Recommended loop:

    edit
      -> narrow validate/test/render
      -> inspect automatic suspects
      -> fix
      -> repeat

Then on materially final candidate:

    mascotctl gate task

## 10. No ad-hoc substitution

The following are not valid claims of a canonical gate pass:

- "cargo check passed, therefore tests pass";
- "joint guards pass, therefore visual QA passes";
- "video looks fine at normal playback, therefore frame QA passes";
- "I ran a custom Python script similar to the gate";
- "the previous gate receipt was green before the last rig change".

If a canonical gate cannot run, report it as blocked.

## 11. Gate implementation quality

The gate runner itself is production-quality development infrastructure.

Requirements:

- deterministic command ordering;
- concise output;
- machine-readable results;
- no unnecessary process-per-frame design;
- reuse intermediate renders;
- no hidden network dependency;
- no CI requirement;
- clear non-zero exit status on failure;
- timings per stage;
- actionable failure paths.

## 12. Anti-shim gate

The repository should include an explicit static hard-cut check.

It should reject superseded internal compatibility logic in source/runtime data, while allowing historical mentions in designated documentation/evidence.

For the current clip migration, reject runtime/source patterns implementing:

- `mascot-clips/0.2` parsing;
- fallback from 0.3 to 0.2;
- old/new dual-read;
- old/new dual-write;
- legacy clip field aliases.

Do not implement this as a naive repository-wide string ban because historical docs/evidence intentionally mention the old format.

## 13. Future gate profiles

As Mascot grows, gate profiles may be added rather than making every gate universally expensive.

Likely future profiles:

- animation/runtime task;
- Windows shell/UI task;
- macOS shell/UI task;
- provider/agentd task;
- release task.

Profiles may choose applicable platform lanes, but they must not silently weaken the task's acceptance contract.
