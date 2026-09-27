# Zig implementation and corrections — zig-001

Scope: first complete Zig candidate (Zig 0.15.2, `-Doptimize=ReleaseSafe`,
`zig-out/bin/mascot.exe`), followed by corrections found during the shared
gates. First-complete snapshot is commit `9dfcf8d` (plus the asset-generic
follow-up that arrived in `757a22f`).

## Implementation summary

- `src/win32.zig` — extern declarations and constants for user32/kernel32/
  gdi32/comctl32/imm32/ole32/windowscodecs used by the candidate.
- `src/platform.zig` — layered mascot window (WIC PNG decode, nearest-
  neighbor resample into a top-down DIB, alpha-driven `UpdateLayeredWindow`),
  composer/dialog hosting RichEdit input + response, DPI handling, global
  hotkey (`CTRL+ALT+SPACE`) and cancel hotkey (`CTRL+ALT+ESCAPE`).
- `src/provider.zig` — coordinator thread spawning the fixture provider with
  explicit environment and no inheritance, stdin writer thread, stdout/stderr
  reader threads, persistent-session reuse, cancel/shutdown state machine,
  strict `shutdown_ack`, prompt-kill teardown on protocol failure.
- `src/framing.zig`, `src/queue.zig` — bounded NDJSON decoder (65,536-byte
  frame cap) and bounded queues with backpressure; `pop`/`tryPush`/`push`/
  `close` semantics shared by control and provider pipes.
- `src/text.zig`, `src/json.zig` — UTF-8/UTF-16 bridging, JSON parse/emit.
- `src/config.zig` — manifest validation incl. provider spec, asset identity
  (path + SHA-256 via BCrypt), UI geometry and schedule cross-checks.
- `src/codex_gate.zig` — `--codex-gate` mode driving a pinned
  `codex app-server` over stdio (JSON-RPC initialize + `config/read` +
  `thread/list`, unsolicited-notification accounting, clean teardown).
  Added for the §16 compatibility gate; gate-only code path.

## Corrections found by the shared gates

1. **Shutdown timeout (pre-gate self-test).** Provider teardown initially
   waited past the shared deadline on protocol-violation paths; corrected to
   bounded grace + `TerminateProcess`, matching the Rust fix pattern.
2. **Composer z-order (zig-acceptance-001/002).** `SetForegroundWindow`
   alone left the composer below unrelated windows on the primary desktop
   (foreground-lock). `showComposer` now raises the composer to the top of
   the non-topmost band. Runs `zig-acceptance-001..004` preserve the
   failures; the earlier W3 drag `(0,0)` delta and transparent-pixel
   `WindowFromPoint` misses were determined to be manual-cursor interference
   and unrelated-window occlusion on the user's primary desktop — resolved
   by the deterministic launch point on the bare virtual display
   (`--launch-point`), not by a candidate change.
3. **u64/i64 manifest conversion** — `config.zig` integer helper returned
   `i64`; checked cast added for `u64` fields.
4. **Asset generality (windows-v1.0.2).** Decode/resample/hit-mask now read
   `pixel_width`/`pixel_height` from the manifest instead of assuming
   128×128 (see `fixture-001-asset-rev.md`).

## Evidence

| Run dir | Result |
|---|---|
| `raw/zig-test-*` | 25/25 unit tests |
| `raw/zig-smoke-002` | PASS on windows-v1.0.2 (approved asset) |
| `raw/zig-provider-002` | PASS_PROVIDER_REGRESSION_ONLY |
| `raw/zig-acceptance-005` | PASS_WITH_UNTESTED (W2/W7 pending lab) |
| `raw/zig-acceptance-hidpi` | **PASS** — full matrix incl. W2 (150% launch) and W7 (125↔100% transition) |
| `raw/zig-codex-gate-001` | PASS — codex 0.80.0 initialize/config/read/thread/list + clean teardown |
