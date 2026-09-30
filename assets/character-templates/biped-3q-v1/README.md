# biped-3q-v1

Canonical rig profile for a compact three-quarter biped character
(`mascot-rig-profile/1`): 23 roles, 41 landmarks (R/J/F/T/S classes),
9 sockets, 10 chains, 31 pose rules, 6 proportions, 3 clearance rules,
normalized [0,1] coordinates (top-left origin, y down), facing +x
(screen-right) and clockwise-positive signed angles. Sides are
character-relative: the character's right side is the near side (drawn in
front, on image-left in this pose); mirroring on screen never relabels sides.

## Current pose (revision 1)

Profile revision 1 is the current dummy: a relaxed, right-facing 3/4 A-pose.
Upper arms 45° from vertical, forearms about 30°, hands about 25°, elbows and
knees slightly bent, legs apart, feet flat and pointing +x. Measured on the
rendered dummy: arm–torso clearance ≈0.044 near / ≈0.049 far, leg–leg ≈0.092.
Revision 1 supersedes the earlier prototype dummy geometry (revision 0);
`profile.json` is the canonical geometry, and any older coordinate or angle
listed elsewhere is superseded by it. The profile `status` stays `prototype`
until the dummy geometry is approved.

The generated mascot T-pose in `assets/research/neutral-bind-pose/` is
research-only and non-canonical. It is not a pose authority and is not used by
this profile, its guides or its fixtures.

## Fixture landmarks

Synthetic fixture specs inherit every landmark from this profile's dummy.
A spec's `landmarks` map contains overrides only, and a character-specific
override must be a true deviation from the dummy (a point that actually
differs). Do not copy dummy coordinates into a spec. `annotate_exclude` leaves
ids out of the generated annotation. Override ids not in the profile, or a spec
whose profile id/revision differs from the loaded profile, are errors.

Files:

- `profile.json` — the authored profile (canonical geometry/values).
- `prompt-template.txt` — generation prompt scaffold, `{brief}` placeholder.
- `guide/` — committed deterministic renders produced by `profile guide`:
  `dummy-generation.png` (1024² pose authority, no annotations),
  `dummy-annotated.png` (2048² with zones, class-coloured dots, labels),
  `guide.json` (receipt with profile + output sha256).

Commands (from repo root, `mascotctl`):

    mascotctl profile check assets/character-templates/biped-3q-v1/profile.json
    mascotctl profile guide assets/character-templates/biped-3q-v1/profile.json
    mascotctl character board --profile assets/character-templates/biped-3q-v1/profile.json \
        --style <style.png> --out <dir> --brief "<text>"
    mascotctl character synth <spec.json> --profile assets/character-templates/biped-3q-v1/profile.json --out <dir>
    mascotctl character validate <source.json>... [--profile <profile.json>] [--report-dir <dir>]

Synthetic fixture specs live under `crates/mascot-character/fixtures/biped-3q-v1/`.
