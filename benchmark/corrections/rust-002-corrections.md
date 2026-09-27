# Rust correction pass — rust-002

Scope: consolidate the `rust-001-review.md` findings into a corrected implementation, then verify. The first buildable snapshot remains preserved at commit `65609963cca879945516129fe2f83823c2a6ba66` and is not correctness-ready; this document covers the correction landing after it.

## Fixes applied

Provider/session (`src/provider.rs`):

- Error teardown now grants only a short bounded grace before `kill`, then waits out the shared 2000 ms deadline. Previously the full deadline was consumed waiting for natural exit, so the kill (and observed exit) landed past the deadline — the root cause of the `rust-provider-001` oversized-case failure.
- `graceful_shutdown` requires a real `shutdown_ack`; EOF alone no longer counts as acknowledgement. The wait loop still stops on EOF so a dead child is not awaited.
- Coordinator uses blocking `push` for provider→UI events (required backpressure, no drops); `send` uses `try_push` so a full command queue fails loudly instead of silently deferring.
- `terminal_emitted` single-claim helper preserves the real `last_seq`; stale-generation guard in `handle_frame`; `client_request` requires the streaming phase; `shutdown_ack` requires `shutdown_sent` and is accepted mid-stream per the frozen provider contract; `requires_client_response`/`client_response_sent` are written once, before cancel and shutdown frames; bounded native-handle wait plus bounded reader joins under one shutdown deadline.

Platform/UI (`src/platform.rs`, `src/text.rs`, `src/main.rs`, `src/control.rs`, `src/config.rs`, `build.rs`):

- Real owned `Surface` (DIB DC + bitmap, nearest-neighbor write into checked `CreateDIBSection` bits, correct drop order); `present_mascot` returns `Result` with checked `GetWindowRect`.
- `WM_NCCREATE` stores the mascot handle early; DPI-correct `font_height(dpi)` at both font sites; monitor-clamped composer placement; shared system `COLOR_WINDOW` brush; no guard/borrow held across re-entrant native calls; IME snapshot cloned before `ImmNotifyIME`.
- `append_bounded` + `EM_REPLACESEL` append; `accepted_qpc` recorded after append and `last_seq`; a response-limit breach invalidates the run and cancels rather than truncating.
- `consume_submit_key(&MSG)` runs before `TranslateMessage`/`DispatchMessageW` so Ctrl+Enter submits exactly once; `WM_QUIT` `wParam` is the process exit code; `GetMessageW` failure requests backend stop with bounded cleanup.
- Control stdin uses the shared `Decoder` with a bounded buffer; invalid/oversized/incomplete-EOF input records a bounded error and requests orderly shutdown. Control teardown waits and `CancelSynchronousIo`s within a 2 s bound; `JoinHandle::as_raw_handle()` (stable since Rust 1.9) replaced an earlier `DuplicateHandle` workaround — fewer owned handles, no publish race.
- Asset identity uses the OS CNG `BCrypt*` SHA-256 provider (bespoke digest removed); `build.rs` pins the installed SDK `rc.exe` and fails if absent.
- Terminal events are recorded only for the current request/generation so stale sessions cannot pollute the event stream; `queue_capacity_frames` reports the real capacity.

## Evidence

| Run dir | Result |
|---|---|
| `raw/rust-test-004` | 30 unit tests pass |
| `raw/rust-build-004` | release build, zero warnings |
| `raw/rust-smoke-006`, `rust-smoke-007` | PASS_SMOKE_ONLY at 96 and 192 DPI launch points (pre-fix binary `2845a3f6…`) |
| `raw/candidate-tools-002` | 9 harness self-tests pass |
| `raw/rust-provider-001` | **FAIL** — oversized-case provider exit observed after the shared deadline (preserved as the failed-before-fix record) |
| `raw/rust-provider-002` | PASS_PROVIDER_REGRESSION_ONLY — all 15 cases: normal×4, fragmented, client_request, cancel (last_seq 49 within deadline, child reused), stderr, backpressure (256), maximum frame, hidden stream/cancel, unexpected_exit (exit 23, fresh child, no replay), oversized (failed, reaped in deadline), clean shutdown exit 0 |
| `raw/rust-smoke-008` | PASS_SMOKE_ONLY on corrected binary `9e5542c7…` at 96 DPI launch point |

Deployed binary after this pass: `out/rust/mascot.exe` SHA-256 `9e5542c70aa95f8511130289335041f22ba7b9d65898c3154b261b96c3b12110`.

## Remaining limits

- The 192-DPI smoke was run on the pre-correction binary; the corrected binary will be re-verified at 200% during the benchmark display session (main display restored to 125% in the meantime).
- `verify_provider_regression.py` is targeted protocol/control coverage. Still untested: full text/IME acceptance (F1–F10 incl. Japanese composition), WM_DPICHANGED monitor transitions, drag/click-through W3–W4, stability batches, and the external visible-presentation observer.
