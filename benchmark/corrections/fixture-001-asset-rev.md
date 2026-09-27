# Fixture revision — fixture-001 (windows-v1.0.1 -> windows-v1.0.2)

## Trigger

User supplied the approved mascot image
(`C:\Users\therceman\Downloads\mascot.png`, PNG RGBA 8-bit, 1254x1254 px,
SHA-256 `2c6e6e90aea8d3283912a46c885a763b588e526740ef9fb5a6d8ba0068ba9b37`)
and directed it be committed as `assets/mascot.png`, replacing the
PROVISIONAL generated asset. Asset identity is frozen, so the fixture was
revised to `windows-v1.0.2` rather than mutated in place.

## Changes

- `assets/mascot.png` added; `assets/provisional-mascot.png` removed.
- `benchmark/manifest/fixture.json`: `version` -> `windows-v1.0.2`;
  `asset` block -> `status: APPROVED`, `path: ../assets/mascot.png`,
  `pixel_width/height: 1254`, `logical_*_dip: 64`, new `sha256`;
  `files_sha256` updated (Go-style `\u003e` escaping preserved).
- `benchmark/fixture.go`, `benchmark/main.go`: user-edited generator —
  requires the approved asset to already exist, no longer calls
  `drawMascot`, hashes the approved file, emits APPROVED metadata.
- `benchmark/FIXTURE_FREEZE.md` updated; `benchmark/harness/
  audit_fixture.py`, `verify_acceptance.py`, `aggregate_results.py` now
  report v1.0.2; the audit decodes the asset at the manifest path instead
  of a hardcoded provisional path.

## Candidate changes (all three)

Candidates previously assumed a 128x128 source image. They now read pixel
dimensions from the manifest/decoded asset while keeping the 64x64 DIP
logical size; source-alpha hit-mask policy unchanged (`alpha > 0`,
no expansion, texel mapping at current effective DPI). The asset remains
square, so window geometry is unchanged.

## Re-validation on windows-v1.0.2

- `raw/fixture-audit-002` — independent audit PASS (pins re-restored after
  environment drift; `requirements-source.txt` applied).
- Rust: `rust-smoke-010`, `rust-provider-003`, `rust-acceptance-010` PASS
  (W2/W7 then deferred), `rust-acceptance-hidpi2` full PASS.
- Zig: `zig-smoke-002`, `zig-provider-002`, `zig-acceptance-005`,
  `zig-acceptance-hidpi` full PASS.
- Go: `go-smoke-002`, `go-provider-002`, `go-acceptance-003`,
  `go-acceptance-hidpi` full PASS.

Failed/aborted runs are preserved: `rust-smoke-009` (pre-rev binary on the
new manifest), `zig-acceptance-004`, `go-acceptance-002`,
`rust-acceptance-hidpi` (harness fixed: W2 second launch must run after the
primary instance releases the global hotkey; `run_hidpi_launch` scoping bug
fixed).
