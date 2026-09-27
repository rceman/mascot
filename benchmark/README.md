# Shared Windows comparison fixture

This directory is shared experimental infrastructure, not the Go candidate. Candidate code must remain independent under `rust/`, `zig/`, and `go/`.

## Implementation and identities

The provider, fixture generator, direct-decoder validator, native QPC clock and process-launch foundation use Go 1.25.3 and its standard library. This provides one reproducible native Windows executable without a browser or additional runtime DLL package. Its process is excluded only because all candidates launch the identical executable with the identical explicit environment.

PowerShell 5 / C# Windows Forms are used for environment preflight and screen evidence. The single-line EDIT IME probe proves that the installed input method works; it does not grant any candidate a text-correctness pass. The earlier multiline RichTextBox probe's extra newline is recorded as a failed exact-text check, not an approved native variation.

Python 3.12.4 independently audits the generated fixture and hosts the mandated `tiktoken/o200k_base` source metric. `requirements-source.txt` pins tiktoken 0.12.0 and all its installed dependencies. These tools are shared measurement infrastructure and excluded from candidate source counts.

`manifest/fixture.json` is the language-neutral consumer contract. JSON was chosen because all three candidates can parse it without custom schema syntax or interpolation. It records artifact SHA-256 identities, absolute native deployment paths, provider environment, frame limits, scenario semantics, UI sizes, text fixtures, the balanced order and measurement repetitions. `harness/control-v1.json` fixes the common candidate automation seam and direct-decoder mode.

The shared mascot is **PROVISIONAL**, 128 by 128 RGBA pixels displayed at 64 by 64 logical pixels. The hit mask is source alpha greater than zero, with no expansion. The Windows task explicitly permits this single temporary asset; all candidates must use its frozen bytes.

## Native build and validation

Working directory: `W:\devin_folder\mascot\benchmark`.

```powershell
go test ./...
go build -trimpath -buildvcs=false -ldflags="-s -w -buildid=" -o bin/fixture.exe .
.\bin\fixture.exe validate 'W:\devin_folder\mascot\benchmark'
& 'W:\devin_folder\tools\mascot-tokenizer\Scripts\python.exe' .\harness\audit_fixture.py 'W:\devin_folder\mascot\benchmark' 'results\windows\raw\fixture-audit-NEW-RUN-ID.json'
```

Use a new audit output path every time. Do not overwrite evidence. After freeze, rebuilding must reproduce the recorded executable SHA; do not replace the frozen provider with an unverified build.

Initial `materialize ROOT` creates payloads, vectors, the PNG and manifest. `refresh-unfrozen ROOT` is a development-only command that preserves the previous manifest and refuses to run once `FIXTURE_FREEZE.md` exists. It never changes an existing mascot to different bytes. Do not use regeneration as a way around the post-freeze versioning rule.

## Provider launch

Use exactly the executable, arguments, working directory and environment map in the manifest. Replace the child environment instead of adding values to the candidate's inherited environment. In particular, the shared provider has `GOMAXPROCS=1`, `GOGC=100`, `GOMEMLIMIT=off`, empty `GODEBUG`, and `GOTRACEBACK=none`. Candidate-only settings cannot influence it. No full inherited environment or secrets are recorded.

Ordinary requests reuse a persistent child. The optional request `scenario` selects a manifest case; an omitted scenario means `normal`. IDs are positive signed 64-bit integers. Responses carry request IDs and zero-based chunk sequences. `emit_qpc` is a fixed-width decimal string, not a floating-point timestamp. Its common frequency is supplied by `start`. Physical frame writes and direct decoder fragments are separate tests.

Cancellation after accepted chunk 49 is cooperative and deterministic. The child pauses at the barrier, acknowledges with exactly one `cancelled` frame, and serves the next ordinary request. Oversized output and unexpected exit invalidate the session; only a new explicit request may create a new one. Stderr must always be drained.

## Harness foundation and observer qualification

`harness/foundation.go` provides sanitized native launch, QPC, typed raw-event/resource/process-inventory records, and exclusive run-directory creation. Missing metrics use null with a reason, not zero. Callers must continuously drain both process output pipes and finish draining before calling `Command.Wait`; this avoids closing unread pipes through `os/exec`.

The manifest fixes all six permutations of candidate order. For each scenario, repetition r uses block r modulo six and executes the candidates sequentially. It also fixes the startup/activation counts, 60-second settle plus 30-second collection windows, 100-millisecond active sampling, 10-minute focused idle, and three batches of 100 stability operations for both fixed and bounded varying content.

The initial freeze intentionally provides an **observer interface**, not a claim of visible-presentation qualification. The fixture handoff permits qualification only after real candidate windows exist. Before headline timing, independently observe actual screen/compositor output, verify input readiness separately, document uncertainty and correlate chunk content with first visible output. Application callbacks, redraw requests and presentation API returns alone are not headline timing endpoints. Record and version any fixture-affecting qualification change for every candidate.

No candidate metrics have been collected by fixture validation. Full acceptance, mixed-DPI transitions, resource collection, runtime diagnostics and observer qualification still have to run against each real application.

## Environment and Git workflow

Preflight evidence is under `results/windows/preflight/`. The lab uses Windows 11 build 26200.9457, a physical 3840 by 2160 display at 200% and an Amyuni usbmmidd 2.0.0.1 virtual 1024 by 768 display at 100%. Native display-mode enumeration reports integral refresh rates of 96 and 60 Hz respectively; these are not precision refresh measurements. Microsoft Japanese IME is installed and the passing environment probe is `ime-009`, including its separate visual review.

Installed tool paths and versions are in `environment-001.json`; the independent audit additionally records the installed tokenizer package versions. No automatic reboot or security-policy change is authorized. Post-reboot samples require an actual boot and cannot be substituted with ordinary process restarts.

Develop and build natively in `W:\devin_folder\mascot`. The authoritative Git repository is WSL Ubuntu `~/git/mascot`, on `agent/windows-rust-zig-go-comparison-v1` only. Synchronize source/evidence back before logical commits; preserve unrelated files and normalize source mode bits. WSL is for repository operations, never for candidate builds, tests or benchmark execution. Do not use GitHub Actions/CI.
