# Agent handoff — Universal Dummy/Board Refinement (Opus 5.5)

Task ID: MASCOT-CHAR-GUIDE-006

## Goal

Refine the `biped-3q-v1` deterministic dummy/combined-board system before we use it to generate the first real CharacterSource.

Slice 1 passed technically. This task is a **visual/profile refinement**, not a new runtime feature phase.

## Repository / branch / workspace

Repository:

    git@github.com:rceman/mascot.git

Branch:

    agent/universal-character-guide-refine-v0.1-opus55

This branch starts from completed Slice 1:

    43c8bd65d144e9503f2662aab49dacb773164518

Use a new isolated native Windows workspace:

    W:\devin_folder\mascot-character-guide-v01

Do not work inside the paused rig workspace.

Native Windows Git + SSH is authoritative.

Do not use CI.

## Bootstrap

    cd W:\devin_folder
    git clone git@github.com:rceman/mascot.git mascot-character-guide-v01
    cd mascot-character-guide-v01
    git fetch origin
    git checkout agent/universal-character-guide-refine-v0.1-opus55
    git pull --ff-only origin agent/universal-character-guide-refine-v0.1-opus55

If the directory already exists, inspect it rather than deleting/resetting unknown state.

## Read first

Read in full:

1. `AGENTS.md`
2. `docs/QUALITY_GATES.md`
3. `docs/MASCOT_UNIVERSAL_CHARACTER_PIPELINE_V0.1.md`
4. `docs/MASCOT_UNIVERSAL_CHARACTER_ARCHITECTURE_V0.1.md`
5. `docs/MASCOT_UNIVERSAL_CHARACTER_DECISIONS_V0.2.md`
6. `docs/MASCOT_UNIVERSAL_CHARACTER_GUIDE_DECISIONS_V0.3.md`
7. current `biped-3q-v1/profile.json`
8. current guide/board implementation and tests.

Planner Decisions v0.3 are authoritative for this task.

## User-facing invariant

Do not change the workflow:

    ONE combined reference board image
        +
    ONE desired-character prompt
        ->
    ONE 1:1 CharacterSource

The combined board still contains:

    LEFT  = dummy pose/rig authority
    RIGHT = style/identity/rendering authority

## Required refinement

### 1. Remove the face wedge from the generation dummy

The current generation dummy has a protruding triangle on the head.

Remove it from `dummy-generation.png`.

The guide must not imply:

- beak;
- snout;
- nose shape;
- muzzle.

Do not replace it with another species-like facial protrusion.

If facing metadata is useful visually, keep that information in the annotated guide/profile rather than turning it into generated character geometry.

### 2. Make the 3/4 view read more clearly

The current dummy is valid but can read too frontal through the torso.

Strengthen generic right-facing 3/4 depth cues.

Prefer:

- near/far z and fill distinction already present;
- modest near/far width differences;
- generic shoulder/hip depth presentation;
- limb overlap/placement that remains profile-driven.

Do NOT redesign the pose into a side view or front view.

Do NOT add identity-specific parts.

Avoid changing normalized landmarks unless needed.

If changing a landmark is required, explain exactly:

- old value;
- new value;
- why rendering-only changes were insufficient;
- validator/test impact.

### 3. Re-check clearances

After the visual refinement, re-measure:

- near arm ↔ torso;
- far arm ↔ torso;
- leg ↔ leg.

Prototype warning thresholds currently approved:

    arm warn = 0.03
    leg warn = 0.06

Failure thresholds remain as in Slice 1.

Do not relax thresholds merely to make the guide pass.

### 4. Correct bend semantics

Validator authority must be geometric/signed and derived from profile/dummy geometry.

Remove/avoid assumptions that both projected elbows have the same intuitive screen bend direction.

Add tests proving the dummy and mirrored/wrong-pose fixtures classify correctly.

### 5. Keep landmark authority single-source

Current `source.json` hash-bound landmark annotation is accepted for now.

Do not duplicate the same canonical coordinates into a new recipe file.

Document how a future character recipe references the approved source annotation without becoming a second landmark authority.

### 6. Appendage zones remain advisory

Do not add hard zone validation.

The current tail zone overlap is known.

If you adjust the tail zone for clearer annotated visualization, keep it explicitly advisory and document the change.

### 7. Regenerate artifacts

Regenerate and inspect:

    dummy-generation.png
    dummy-annotated.png
    guide.json

and the combined board using:

    assets/mascot.png

as the style-reference input.

Regenerate bounded evidence.

The board must still have:

    LEFT  = exact generation guide
    RIGHT = deterministic fit/pad style reference

with no crop/stretch.

## Visual acceptance

Inspect the actual images, not only hashes/tests.

The revised generation dummy should satisfy:

- recognizably right-facing 3/4;
- no species identity;
- no beak/snout/muzzle implication;
- compact stylized biped;
- clear near/far depth;
- all core limbs visible;
- arms separated from torso;
- legs separated;
- hands/feet readable;
- no props;
- no directional shading;
- no labels/text.

The board should make it obvious that the left panel is pose geometry and the right panel is style identity.

## Tests / evidence

Preserve all Slice-1 positive/negative validation tests.

Add/adjust tests for:

- no protruding face polygon in generation-guide geometry;
- deterministic guide hashes;
- deterministic board;
- dummy still validates;
- mascot-like fixture still passes;
- robot fixture still passes;
- wrong-facing/mirrored fixture still fails for geometric reasons;
- no character-specific branch.

If exact image golden hashes change, update them only after inspecting the regenerated artifacts.

## Do not proceed to real character generation

This agent does not need to generate the final robot/mascot image.

The next Planner/user step will use the revised combined board with ChatGPT image generation.

Stop after guide/profile refinement and evidence.

## Final report

Return:

1. starting HEAD;
2. final HEAD;
3. profile changes;
4. landmark changes, if any;
5. generation-guide visual changes;
6. annotated-guide changes;
7. combined-board output/hash;
8. measured clearances;
9. bend-semantics correction;
10. positive fixture results;
11. negative fixture results;
12. dependency changes;
13. files/commits;
14. Gates 1–20;
15. native Windows clean/push status.

End with exactly:

    MASCOT_CHAR_GUIDE_006_COMPLETE

or:

    MASCOT_CHAR_GUIDE_006_BLOCKED: <reason>
