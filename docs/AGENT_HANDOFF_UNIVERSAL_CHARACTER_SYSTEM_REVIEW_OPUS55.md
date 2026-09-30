# Agent Handoff — Universal Character System Final Architecture Review (Opus 5.5)

Task ID: MASCOT-CHAR-REVIEW-007

## Goal

Perform one final independent architecture review before Planner/user freeze the implementation plan.

Do not implement production changes.

The Planner has consolidated the current direction into:

    docs/MASCOT_UNIVERSAL_CHARACTER_SYSTEM_SYNTHESIS_V0.4.md

Your job is to review it critically against:

- repository code;
- completed research;
- completed universal architecture work;
- Slice-1 implementation;
- paused rig findings;
- the goal of a reusable character animation framework.

Do not assume the Planner synthesis is correct.

## Repository / branch / workspace

Repository:

    git@github.com:rceman/mascot.git

Branch:

    agent/universal-character-final-review-opus55

This branch begins from the current universal guide/planner state and includes the completed Slice-1 implementation ancestry.

Use a NEW isolated native Windows workspace:

    W:\devin_folder\mascot-character-review

Do not modify:

    W:\devin_folder\mascot-rig-v02

That old workspace contains intentionally preserved paused WIN-002 work.

Native Windows Git + SSH is authoritative.

Do not use CI.

## Bootstrap

    cd W:\devin_folder
    git clone git@github.com:rceman/mascot.git mascot-character-review
    cd mascot-character-review
    git fetch origin
    git checkout agent/universal-character-final-review-opus55
    git pull --ff-only origin agent/universal-character-final-review-opus55

## Read first

Read in full:

1. `AGENTS.md`
2. `docs/QUALITY_GATES.md`
3. `docs/MASCOT_UNIVERSAL_CHARACTER_SYSTEM_SYNTHESIS_V0.4.md`
4. `docs/MASCOT_UNIVERSAL_CHARACTER_PIPELINE_V0.1.md`
5. `docs/MASCOT_UNIVERSAL_CHARACTER_ARCHITECTURE_V0.1.md`
6. `docs/MASCOT_UNIVERSAL_CHARACTER_DECISIONS_V0.2.md`
7. `docs/MASCOT_UNIVERSAL_CHARACTER_GUIDE_DECISIONS_V0.3.md`
8. `docs/MASCOT_RIG_ART_CONSTRUCTION_RESEARCH_V0.1.md`
9. `docs/MASCOT_RIG_ART_CONSTRUCTION_SPEC_V0.1.md`
10. `docs/MASCOT_RIG_RUNTIME_SPEC_V0.2.md`
11. relevant current Rust/profile/tooling/source-validation code.

Also inspect the completed Slice-1 implementation from:

    MASCOT-CHAR-PIPE-005

and current `biped-3q-v1` profile/tooling.

## Primary direction to review

The current preferred simplification is:

    ONE approved assembled 1:1 CharacterSource
        +
    RigProfile
        ->
    agent/tooling builds the rig representation

Image generation creates character art.

Image generation does NOT define:

- bones;
- cuts;
- meshes;
- weights;
- exploded sheets;
- animation frames.

A bone does NOT imply a cut.

A region may be one continuous weighted mesh across several bones.

Use the least fragmented representation that passes the required articulation and visual QA.

## Critical example

For an organic arm, test whether:

    shoulder -> upper_arm -> lower_arm -> hand

can deform one continuous textured mesh cleanly.

If yes, do NOT split the arm at the elbow merely because the skeleton has an elbow bone.

For a robot, the same profile may instead select rigid hinge attachments.

The runtime must remain generic.

## Required review behavior

You must challenge the Planner synthesis.

For every major proposal classify it as:

    ACCEPT
    ACCEPT WITH CHANGE
    REJECT
    DEFER

and explain why.

Do not protect prior work because it already exists.

Do not reject useful Slice-1 tooling merely because it is no longer mandatory.

## Key question: current board/dummy work

The previous direction spent substantial effort on a combined generation board.

The Planner now believes that board should be optional authoring convenience, not a required rigging contract.

Review:

- RigProfile;
- dummy renderer;
- board composer;
- CharacterSource manifest;
- source validator;
- synthetic fixtures;

and classify each:

    KEEP
    ADAPT
    OPTIONAL
    RETIRE

## Required technical review

Inspect actual code and answer:

- how current `Attachment::Mesh` / `skin_mesh` can be reused;
- what is missing from draw-list/render path;
- whether CPU skinning remains appropriate initially;
- how one textured region can span multiple bones;
- how line layers should share/deform with mesh geometry;
- how external outline should consume mixed sprite/mesh fills;
- how hidden-art completion should be represented;
- how the tool/agent should iterate meshes/weights;
- what measurable failure thresholds are needed;
- what must remain visual-review-only;
- how a robot rigid-part character fits the same system.

## Animation / physics review

Preserve the no-sprite-sheet core architecture.

Review the staged evaluation model:

    animation intent
        -> constraints / IK targets
        -> physical pose solver
        <-> contacts / joint constraints
        -> final bone pose
        -> skinning
        -> rendering

Do not implement physics.

State what interfaces/data must be stable now so later physics does not force a CharacterPack rewrite.

## Required output

Create:

    docs/MASCOT_UNIVERSAL_CHARACTER_SYSTEM_REVIEW_V0.1.md

Follow the required section list from the Planner synthesis.

The report must end with:

### Recommended implementation sequence

Give a concrete ordered phase plan.

For each phase include:

- purpose;
- prerequisites;
- code/art scope;
- test/evidence;
- stop condition;
- what it proves;
- what it deliberately does NOT do.

Then include:

### Recommended first implementation task

Write the exact bounded task you think should run immediately after Planner/user approve the final plan.

Do not execute it.

## No implementation

Allowed:

- docs;
- read-only source inspection;
- read-only local measurements;
- small throwaway scratch analysis outside committed product tree if necessary.

Not allowed:

- production Rust changes;
- renderer implementation;
- profile behavior changes;
- source/rig asset changes;
- mesh implementation;
- clip migration;
- shading;
- IK;
- physics.

Commit/push only the review document and any strictly necessary review-only documentation correction.

Then STOP.

## Final report

Return:

1. starting HEAD;
2. final HEAD;
3. overall verdict;
4. strongest accepted Planner ideas;
5. strongest rejected/modified Planner ideas;
6. current-code salvage summary;
7. one-source CharacterSource verdict;
8. representation-selection verdict;
9. renderer prerequisite verdict;
10. animation/physics staging verdict;
11. Slice-1 board/dummy verdict;
12. recommended implementation phases;
13. recommended first task;
14. open Planner/user decisions;
15. files changed;
16. confirmation no implementation occurred;
17. Gates 1–20 PASS/FAIL/N/A;
18. native Windows clean/push confirmation.

End with exactly:

    MASCOT_CHAR_REVIEW_007_COMPLETE

or:

    MASCOT_CHAR_REVIEW_007_BLOCKED: <reason>


## Additional mandatory review: A-pose / source-fit question

The user and Opus had a focused pre-review discussion after the Planner synthesis was written.

Read Section 29 of:

    docs/MASCOT_UNIVERSAL_CHARACTER_SYSTEM_SYNTHESIS_V0.4.md

You MUST explicitly review:

1. relaxed A-pose vs T-pose for the first 2D weighted-mesh biped profile;
2. why the existing generated T-pose reference is not currently a production bind/source;
3. whether profile/view consistency matters more than the exact T-vs-A label;
4. whether a CharacterSource must exactly overlay dummy landmarks, or whether per-character landmark fitting within profile envelopes is the correct universal model;
5. whether the current mascot really needs a separate seated art set, or whether that should remain evidence-driven after neutral-rig sweeps.

Do not silently accept the preliminary Opus answer from the conversation.
Re-evaluate it against the actual code, retargeting goals, and universality requirement.
