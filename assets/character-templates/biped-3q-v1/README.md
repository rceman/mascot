# biped-3q-v1

Canonical rig profile for a compact three-quarter biped character
(`mascot-rig-profile/1`): 23 roles, 41 landmarks (R/J/F/T/S classes),
9 sockets, 10 chains, 31 pose rules, 6 proportions, 3 clearance rules,
normalized [0,1] coordinates (top-left origin, y down), facing +x
(screen-right) and clockwise-positive signed angles. Sides are
character-relative: the character's right side is the near side (drawn in
front, on image-left in this pose); mirroring on screen never relabels sides.

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
    mascotctl character synth <spec.json> --out <dir>
    mascotctl character validate <source.json>... [--profile <profile.json>] [--report-dir <dir>]

Synthetic fixture specs live under `crates/mascot-character/fixtures/biped-3q-v1/`.
