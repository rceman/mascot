# Windows fixture freeze

Status: **READY FOR CANDIDATE IMPLEMENTATION**. This is not candidate correctness approval or permission to publish unqualified headline timing.

## Repository identity

- Task: `MASCOT-WIN-COMPARE-001`.
- Branch: `agent/windows-rust-zig-go-comparison-v1`.
- Updated planning base: `2f55ae825848e183a7840032782e37cbc54e641d`.
- The implementation branch originally started at `8d30360e88d11945361ab1f12ac7fec35ad1aa17`, then fast-forwarded to the updated planning head before implementation.
- Frozen source/artifact commit: `378046dd624be57019b2947b377e5dde2043bbbe`.
- Freeze-record commit: the commit introducing this file; resolve with `git log -1 --format=%H -- benchmark/FIXTURE_FREEZE.md`. A separate record commit avoids a self-referential commit hash.
- Fixture/manifest version: `windows-v1.0.2` (amended; see "Version history").
- Text fixture version: `text-v1.0.0`.
- Candidate control interface: `candidate-control-v1`.

All build and validation execution was native Windows. WSL was used only to stage/synchronize and commit repository files. No candidate implementation existed at this freeze.

## Frozen identities

| Artifact | SHA-256 |
|---|---|
| `manifest/fixture.json` | `cfdee605c3864e2cf50ed1adcf21dba96daa3a4bb4bbae1d340518ebbc0fc625` |
| `bin/fixture.exe` | `e6a5b4d20f1c92fbb7bd48778be692b91970acc72007d6399736ef1025d5160d` |
| `fixtures/response.txt` | `afab2753b91d30eb9297b7afe77340f078076fa41579d68afbd5de93fb8aeb9e` |
| `fixtures/chunks.json` | `146898dfbc8f875cab8a15b714d79bd9be0e4841f7de840952dc36a2edabfdc1` |
| `fixtures/history.json` | `1f67e4a1eeb74087efe16f5f1d8ec7fa28af31a8e95e08b68caa0abe7e168c05` |
| `fixtures/text.json` | `9ff1439b559aa5f26389b76576f7a2c3a9f3a975b50c83835986790863746639` |
| `decoder-vectors/vectors.json` | `58f288aef52729587a42171374f152d3fdaf045b75e8104b1d60fafc9654507d` |
| `harness/control-v1.json` | `c278ede7159d8bbf3faa8ebbd8f2c820f7f9bb4d5c66bf3e5f44d124700f7ac4` |
| `requirements-source.txt` | `015f22b13fabd548033c52de1515312d9483a2796cf5d9eebabc7b7216893f8a` |
| `../assets/mascot.png` | `2c6e6e90aea8d3283912a46c885a763b588e526740ef9fb5a6d8ba0068ba9b37` |

The manifest freezes provider path/arguments/cwd, an explicit eight-key environment without inheritance, 100 normal chunks, the chunk-49 cancellation barrier, 1,000 ms cancellation and 2,000 ms shutdown/reap deadlines, physical write plans, exact frame sizes, terminal events, and recovery rules. The approved asset is 1254 by 1254 RGBA pixels at 64 by 64 logical pixels, with source alpha greater than zero as the hit mask and no expansion.

UI dimensions, font/fallback policy, fixed history, F1-F10 strings/actions, keyboard shortcuts, IME identity, no permitted native variations, bounded input/response sizes, common control hooks, the six balanced order permutations and required sample counts are frozen in the manifest and referenced files.

## Environment preflight

Evidence: `results/windows/preflight/environment-001.json` and `results/windows/preflight/ime-009/{observations.json,review.json,01-preedit.png,02-committed.png,03-before-cancel.png,04-cancelled.png}`.

```text
Japanese IME available: YES
preedit/composition visible: YES
commit works: YES
cancel works: YES
```

The probe used actual scan-code `nihonn` input through Microsoft Japanese IME, not injected Japanese Unicode. Visible preedit and committed text were both exactly `にほん`; cancellation restored `baseline` exactly. Native events and all four screenshots were reviewed. This was a stock single-line Win32 EDIT environment probe, not a candidate composer test. Earlier failed/partial attempts remain unchanged and are described in `review.json`; the RichTextBox probe's extra newline is not allowed in a candidate.

The shared display lab has a physical 3840 by 2160 display (normally at the user's 125% scale; raised to 150% only for the W2/W7 high-DPI acceptance window, then restored) and a virtual 1024 by 768 display at 100%, using signed Amyuni USB Mobile Monitor driver 2.0.0.1. Preflight records Windows 11 Home 26200.9457, i9-9900K with 8 cores/16 logical processors, RAM, Balanced power, language profiles, Rust/Cargo 1.94.0, Zig 0.15.2, Go 1.25.3, Python 3.12.4, Visual Studio Build Tools 17.14.37027.9 and SDK 10.0.26100.0. No automatic reboot or security-policy change was performed. Source-tool dependency versions are recorded in the independent audit and pinned lock file.

## Validation and evidence

Native commands are documented in `README.md`. Executed successfully:

- `go test ./...`: provider bound/UTF-8/reuse tests and harness native launch, explicit environment, clock, null-metric schema and immutable-run tests.
- Release build using Go 1.25.3, `-trimpath -buildvcs=false -ldflags="-s -w -buildid="`.
- `fixture.exe validate ROOT` with the final manifest and binary.
- Independent Python `harness/audit_fixture.py`, using the pinned tokenizer environment, checking hashes, schema, exact text/UTF-16 selections, all decoder vectors, asset dimensions/alpha format, schedule and recorded IME preflight.

Final provider validation: `results/windows/raw/fixture-validation-20260927T124009.286679400Z.json`.
Independent audit: `results/windows/raw/fixture-audit-001.json`.

| Shared check | Result |
|---|---|
| Artifact hashes and identical provider binary | PASS |
| Sanitized child environment and effective GOMAXPROCS | PASS |
| Normal 100-chunk stream and exact response reconstruction | PASS |
| Physical fragmentation/coalescing and split multibyte UTF-8 | PASS |
| Client request/response | PASS |
| Cancel after seq49, correct terminal, no seq50/complete, same-child reuse | PASS |
| 262,144-byte concurrent stderr pressure and exact content | PASS |
| 256-frame backpressure case | PASS |
| 65,536-byte maximum-valid frame | PASS |
| 65,537-byte oversized invalidation, reap and explicit fresh session | PASS |
| Unexpected exit23 and explicit fresh session | PASS |
| Shutdown acknowledgement and child reaping | PASS |
| All six direct decoder vectors in Go and independent Python | PASS |
| Tokenizer 0.12.0/o200k_base available with exact installed dependency pins | PASS |

The earlier `windows-v1.0.0` validation was an unfrozen development run. Its manifest is preserved as `results/windows/raw/unfrozen-manifest-20260927T124001.486603100Z.json`; it is not this freeze. The final version adds the fixed control schema/dependency identities and explicit ordering/cold-launch counts. No candidate metrics existed to invalidate.

Raw preflight JSON retains its original Windows BOM/CRLF bytes through `.gitattributes`; it is deliberately not whitespace-normalized. Source whitespace checks passed separately. All raw evidence and the provider executable are committed, not reconstructed summaries.

## Scope limits and readiness

There is no current fixture or environment blocker to architecture notes and candidate implementation. Rust, Zig and Go still need every required window/text/provider acceptance gate with their own evidence; none is marked PASS by this record.

The common native process/QPC foundation and result/inventory/resource schemas are implemented. The visible-presentation observer is **interface-only pending qualification**, as allowed by the fixture handoff before candidate windows exist. No startup, hotkey or stream-to-visible headline result may use application callbacks alone. Before measurement, qualify one external observation procedure for all candidates, record resolution/uncertainty and freeze/version any affected measurement configuration. Resource sampling implementation and full UI acceptance automation likewise require qualification against real candidates.

Post-reboot measurements require genuine boots and user coordination; no reboot is automatic. Missing or infeasible observations must remain explicitly unavailable, not be manufactured from ordinary launches.

Any later change to fixture semantics, assets, text/actions, provider executable/environment, timeouts or decoder vectors requires a new fixture version and rerunning affected candidate checks/results. Preserve old raw run IDs. Do not regenerate or silently edit this frozen version to fit a candidate.

## Version history

- `windows-v1.0.1` (freeze commits `378046dd` / `49d20a2`): initial freeze with a task-approved PROVISIONAL generated mascot (`../assets/provisional-mascot.png`, 128x128).
- `windows-v1.0.2`: the user supplied and approved the real mascot image. Asset identity only changed: `../assets/mascot.png`, 1254x1254 RGBA, SHA-256 `2c6e6e90aea8d3283912a46c885a763b588e526740ef9fb5a6d8ba0068ba9b37`, status `APPROVED`, same 64x64 logical DIP window and the same alpha>0 texel hit-mask policy. Protocol, text fixture, scenarios, deadlines, environment and schedule are unchanged. Because asset bytes and version moved, all candidate asset decode/hit-test/resample paths were made dimension-generic and every candidate gate (smoke, provider regression, acceptance) was re-run against v1.0.2. Earlier v1.0.1 raw runs remain preserved under their original run IDs and are superseded by the v1.0.2 evidence.
