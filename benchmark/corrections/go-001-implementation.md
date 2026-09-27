# Go implementation and corrections — go-001

Scope: first complete Go candidate (go1.25.3, `bin/mascot.exe` with
`gen_syso.go`-generated manifest/.syso resources). First-complete snapshot is
commit `bb1804e` (asset-generic follow-up in `757a22f`).

## Implementation summary

- `main.go` — entry point, app loop, provider coordinator wiring.
- `platform` layer (`ui.go`, `wndproc.go`, `win32.go`) — layered mascot
  window (WIC decode, resample, alpha hit test), RichEdit composer with
  IME/dead-key/candidate-window coverage, global hotkeys, DPI transitions.
- `provider.go` — child lifecycle with explicit environment, stdin/stdout/
  stderr plumbing, persistent-session reuse, cancel/shutdown handling.
- `control.go`, `framing.go`, `queue.go`, `text.go` — bounded control pipe,
  65,536-byte NDJSON decoder, bounded queues, UTF bridging.
- `config.go` — manifest validation incl. provider and asset identity.
- `codex_gate.go` — `--codex-gate` mode: `codex app-server` over stdio,
  JSON-RPC initialize + `config/read` + `thread/list`, unsolicited-
  notification accounting, stdin-close/terminate fallback teardown.

## Corrections found by the shared gates

1. Early acceptance run `go-acceptance-001` showed drag `(0,0)` deltas —
   determined to be manual-cursor interference by the user during the run,
   not a candidate defect; rerun without interference passed
   (`go-acceptance-003`).
2. **Asset generality (windows-v1.0.2).** Decode/resample/hit-mask now read
   `pixel_width`/`pixel_height` from the manifest instead of assuming
   128×128 (see `fixture-001-asset-rev.md`).
3. Gate-mode exit-code distinction: gate failure exits 1, protocol/usage
   errors keep exit 64.

## Evidence

| Run dir | Result |
|---|---|
| `raw/go-smoke-002` | PASS on windows-v1.0.2 |
| `raw/go-provider-002` | PASS_PROVIDER_REGRESSION_ONLY |
| `raw/go-acceptance-003` | PASS_WITH_UNTESTED (W2/W7 pending lab) |
| `raw/go-acceptance-hidpi` | **PASS** — full matrix incl. W2 and W7 |
| `raw/go-codex-gate-001` | PASS — codex 0.80.0 + clean teardown |
