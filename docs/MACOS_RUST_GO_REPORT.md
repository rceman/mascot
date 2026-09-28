# macOS Stage B report — Rust vs Go critical slice

Question under test: for an ultra-low-memory, ultra-low-latency native
desktop agent shell, does either surviving candidate (Rust, Go) develop a
structural macOS problem that changes the foundation decision?

**Answer: no.** Both candidates pass the complete Stage B critical slice
on native macOS — native AppKit UI, real `.app` bundles, all nine provider
scenarios, full interactive matrix, streamed Unicode rendering, measured
idle/streaming resources, and clean teardown. Neither requires a fragile
unsupported hack for the critical-slice feature set. The differences that
remain are quantitative (footprint, threads, glue size, build speed), not
structural.

1. **Base SHA:** `7858b54a1f5ec07f370e8e767dc5d751eebbaed6`
2. **Branch / HEAD:** `agent/macos-rust-go-critical-slice-v1`; evidence
   HEAD `1971d4a` (report committed on top; `git rev-parse HEAD` is the
   tip containing this file).
3. **Machine:** Apple M2, 16 GiB RAM, arm64; macOS 26.7 (25G229),
   Darwin 25.6.0; Apple clang 21.0.0; Command Line Tools only (no full
   Xcode); SDK pinned to `MacOSX26.sdk`.

## 4. Rust candidate

- Stack: Rust (cargo 1.94.0 via `rust-toolchain.toml`), `objc2` 0.6.4,
  `objc2-app-kit`/`objc2-foundation` 0.3.2, `block2` 0.6.2, `libc`.
  Direct Carbon FFI for global hotkeys; `dispatch_async_f` main-queue
  wakeups; `define_class!` subclasses (`NSView` mascot, `NSTextView`
  input/response, `NSObject` window delegate).
- Bundle: `rust/packaging/MascotRust.app` — `Contents/MacOS/mascot`
  (765,936 bytes), `Info.plist`, `_CodeSignature`; ad-hoc linker-signed,
  756 K total, no adjacent payload.
- Result: 9/9 provider scenarios, 19/19 interactive checks, clean
  shutdown, zero stderr, measured resources below.

## 5. Go candidate

- Stack: Go 1.25.3, cgo; all Cocoa code lives in the project-owned
  Objective-C shim `go/mascot_darwin.m` (+ `mascot_darwin.h` C API);
  `golang.org/x/sys` v0.36.0 for unix primitives. Carbon hotkeys and TIS
  calls compile directly in the shim.
- Bundle: `go/packaging/MascotGo.app` — `Contents/MacOS/mascot`
  (4,118,834 bytes), `Info.plist`, `_CodeSignature`; ad-hoc linker-signed,
  3.9 MB total, no adjacent payload.
- Result: 9/9 provider scenarios, 19/19 interactive checks, clean
  shutdown, zero stderr, measured resources below.

## 6. Correctness

| Check | Rust | Go |
|---|---|---|
| Fixture: normal / fragmented / maximum | PASS | PASS |
| Fixture: cancel (cooperative, child survives) | PASS | PASS |
| Fixture: client_request / backpressure | PASS | PASS |
| Fixture: oversized → failed, run marked invalid | PASS | PASS |
| Fixture: unexpected_exit → failed | PASS | PASS |
| Fixture: stderr pressure (262,144 B tail-counted) | PASS | PASS |
| Hotkey show/hide, hotkey cancel | PASS | PASS |
| Composer takes real key focus on show | PASS | PASS |
| ASCII + Unicode typing roundtrip | PASS | PASS |
| Marked-text composition flag | PASS | PASS |
| IME commit / cancel (composition pipeline) | PASS* | PASS* |
| Submit blocked while composing | PASS | PASS |
| Hide during composition leaves consistent state | PASS | PASS |
| Cmd+Return submits; plain Return = newline | PASS | PASS |
| Streamed Unicode response renders (LV/CYR/emoji/AR-bidi) | PASS | PASS |
| Mascot perches on composer top edge; anchored during moves | PASS | PASS |
| Transparent-pixel click-through | PASS | PASS |
| Screenshots / recording | PASS | PASS |
| Clean shutdown, no orphan children, no stderr | PASS | PASS |

\* Composition was exercised through the *same* `NSTextInputClient` entry
points (`setMarkedText:`/`insertText:`/`unmarkText`) that a live input
method drives, because selecting an input *method* source requires a
genuine user gesture on macOS 26 (see §12). The marked-text pipeline —
composing flag, submit guard, cancel/discard, hide-during-composition —
is identical from the view's side.

### macOS shortcut map (both candidates)

| Logical action | macOS binding | Notes |
|---|---|---|
| Toggle composer | Ctrl+Alt+Space (Carbon hotkey) | macOS-appropriate custom combo; does not collide with system input-source or Spotlight defaults on this machine |
| Cancel active request | Ctrl+Alt+Escape (Carbon hotkey) | same registration path |
| Submit | Cmd+Return | native convention; plain Return = newline |
| Newline | Return | multiline input |
| Copy/Paste/Cut/Select-All | Cmd+C / Cmd+V / Cmd+X / Cmd+A | provided natively by `NSTextView` |
| Composition commit | Return (during marked text) | native IME semantics |
| Composition cancel | Escape (during marked text) | native IME semantics |

## 7. Resources and latency

Same machine/power/display, one measurement pass per candidate;
`raw/{rust,go}-measurements/measurements.json`.

| Metric | Rust | Go |
|---|---|---|
| Launch → mascot visible | 400 ms (repeats 267–327 ms) | 350 ms (repeats 264–335 ms) |
| First composer activation | 427 ms | 422 ms |
| Warm composer activation | 325–333 ms | 159–330 ms |
| Idle mascot RSS / phys footprint | 64.9 / 35.7 MB | 71.3 / 38.9 MB |
| Composer-open RSS / footprint | 113.0 / 45.9 MB | 100.5 / 48.5 MB |
| Streaming-peak RSS / footprint | 120.1 / 73.2 MB | 122.4 / 81.5 MB |
| Idle threads | 9 | 13 |
| Streaming threads | 12 | 17 |
| Idle CPU (one-core) | 0.0% | 0.0% |
| Child inventory while streaming | 1 (fixture PID 54551) | 1 (fixture PID 54641) |
| Show/hide + submit/cancel loop failures | 0 | 0 |
| RSS drift across loops | +3.0 MB | +5.2 MB |
| Clean shutdown | 25.9 ms | 17.4 ms |

Rust idles ~6.4 MB lower RSS / ~3.2 MB lower footprint; Go opens the
composer ~12.5 MB cheaper on RSS but peaks ~8 MB higher streaming; Go
carries +4 threads idle/streaming (Go runtime + cgo). Both are
event-driven at idle (0.0% CPU, no polling/redraw loop).

## 8. macOS-specific LOC / tokens / FFI

Nonblank, noncomment LOC; tokens via frozen `tiktoken/o200k_base` 0.12.0.

| | Rust | Go |
|---|---|---|
| macOS-specific files | 4 | 13 |
| macOS-specific LOC | 1,778 | 1,671 |
| macOS-specific tokens | 15,314 | 14,721 |
| Shared-from-Windows LOC | 2,993 | 1,839 |
| Shared-from-Windows tokens | 23,100 | 14,508 |

Largest macOS units: `rust/src/platform/macos.rs` 1,670 LOC / 14,440 tok;
`go/mascot_darwin.m` 529 LOC / 5,445 tok; `go/ui_darwin.go` 604 LOC /
4,726 tok; `go/platform_darwin.go` 227 LOC / 1,684 tok.

FFI/native boundaries:

- Rust: `unsafe extern` blocks (Carbon hotkeys, `dispatch_async_f`,
  `CFRelease`); `dlsym`-resolved TIS functions; `objc2 define_class!`
  subclasses; `msg_send!` for ungenerated AppKit calls. `unsafe` is
  concentrated in `platform/macos.rs`.
- Go: cgo boundary `Go → mascot_darwin.h` (opaque pointers); Objective-C
  `→ Go //export` callbacks (`goUI*`, `goDispatch*`); Carbon/TIS calls
  inside the shim; `unsafe` on the Go side is pointer plumbing only.

## 9. Build / feedback loop

| | Rust | Go |
|---|---|---|
| Incremental release build (single-file change) | ~9.7 s | ~4.1 s |
| Bundle step | `packaging/build_app.sh` (cp + plist + codesign -s -) | `build_app.sh` (same shape) |
| Toolchain friction | edition-2024 FFI rules, objc2 API discovery, 15+ warnings to clean | cgo header-ABI mismatches, header/type fixes, cgo export signatures |
| SDK workaround required | yes (SDKROOT pin) | yes (SDKROOT pin) |

## 10. Visual evidence

`benchmark/results/macos/visual/`:

- `rust/mascot.png`, `rust/composer.png`, `rust/perched.png`,
  `rust/response.png`, `rust-interaction.mov` (launch→drag→click-through→
  hotkey→type→submit→stream→hide)
- `go/mascot.png`, `go/composer.png`, `go/perched.png`,
  `go/response.png`, `go-interaction.mov` (same sequence)

Captures are own-window `CGWindowListCreateImage` + full-screen
`screencapture -V`; the perched shot shows the mascot centered on the
composer's top edge; `response.png` shows streamed Latvian/Cyrillic/
emoji/Arabic text in a native scroll view.

## 11. ACP gate status (Devin, not Codex app-server)

Both candidates spawn `devin acp`, complete `initialize` and
`session/new`, then `session/prompt` fails with "Please log in to use
Devin." **Status: UNAVAILABLE** — the ACP transport/protocol path is
proven; only the interactive-auth sub-gate could not be completed
unattended. Evidence: `raw/acp-gate/{rust,go}.ndjson`.

## 12. Structural findings, workarounds, risks

1. **Accessory-app activation (both).** On macOS 26 an accessory-policy
   app cannot be activated by synthetic input — injected keys, clicks and
   hotkeys do not register as a user gesture; `NSApp.activate()` and
   `activateIgnoringOtherApps:` are ignored in that context. Verified by
   frontmost-app inspection (`Devin` stayed frontmost after
   show/click/hotkey). With `MASCOT_REGULAR_APP=1` the same binaries
   activate correctly (`frontmost: mascot`), proving the activation code
   path is sound; real users activate accessory apps via genuine
   clicks/hotkey presses. Residual risk: on the strictest future macOS,
   activation may require a real gesture in all cases — acceptable for a
   hotkey/click-driven product, but worth a manual sanity check.
2. **Input-method selection (both).** `TISSelectInputSource` returns
   `paramErr(-50)` for input-method sources (Kotoeri) — it works for
   keylayouts (ABC selected fine). `NSTextInputContext.selectedKeyboard-
   InputSource` silently ignores input-method IDs. Programmatic input-
   method switching is thus not available; the app relies on the system
   input menu / user shortcut. Composition itself was verified through
   the identical `NSTextInputClient` calls an IME makes.
3. **Hide during composition (both).** Resigning key commits preedit
   (native `NSTextView` behavior) rather than Windows-style discard;
   committed state stays consistent. Documented platform difference.
4. **SDK 27 linker breakage (environment).** Default CLT SDK's `.tbd`
   files contain `arm64e.x1-*` entries the linker rejects; all builds pin
   `SDKROOT=MacOSX26.sdk`. Recorded in `build_app.sh` scripts.
5. **TIS symbol lookup (Rust).** HIToolbox is not linkable via
   `-framework Carbon` and `TISCopyInputSourceWithID` was removed on
   macOS 26; Rust resolves `TISCreateInputSourceList`/`TISSelectInput-
   Source`/`TISGetInputSourceProperty` via `dlsym(RTLD_DEFAULT)`.
6. **CGWindowListCreateImage unavailability (harness).** Marked
   unavailable on macOS 15+ SDKs; the screenshot helper calls it through
   `dlsym` — works at runtime for own-window capture.
7. **Documents TCC (harness).** `screencapture -V` cannot write into
   `~/Documents`; recordings were written to `/tmp` then moved.
8. **Accidental side effect cleaned up:** an earlier `imeSelect` helper
   briefly enabled a Katakana input source while probing; this is a
   user-menu change worth noting though it had no effect on results.
9. **Non-blocking windows.** The mascot panel is `NonactivatingPanel`,
   floating level, per-pixel-alpha hit-tested (click-through verified);
   click focus tests confirmed the composer takes real key focus while
   the mascot stays non-activating.

## 13. Unavailable measurements

- **Real-provider streamed prompt via Devin ACP** — unavailable: CLI not
  authenticated; `devin auth login` requires interactive credentials.
  All ACP steps up to the prompt succeed; see §11.
- **Live Japanese IME session** — unavailable under automation only:
  input-method source selection requires a real user gesture (§12.2).
  Composition/commit/cancel verified via identical entry points.
- **Streaming-peak CPU %** — the `top` delta sample did not resolve the
  CPU column for the short streaming window; idle CPU is 0.0% for both.
- **Full-screen "perched composite" capture** — own-window captures only;
  `perched.png` shows the mascot window itself over the composer top edge
  (geometry verified numerically via CGWindowList bounds).

## 14. Commits on this branch (newest first)

- `1971d4a` anchored perch + macOS keyboard semantics + interactive evidence
- `21c318f` mascot↔composer top-edge anchoring
- `2d913f4` native macOS AppKit Go candidate
- `fcd79fa` native macOS AppKit Rust candidate
- `a3a9e17` native darwin/arm64 fixture build + manifest
- `2d69601` handoff doc

## 15. Clean worktree

Verified at report time with `git status --porcelain` — branch pushed,
no untracked/modified files remain.

## Factual Rust-vs-Go comparison

| Dimension | Rust | Go | Edge |
|---|---|---|---|
| Correctness gates | 9/9 scenarios, 19/19 interactive | 9/9, 19/19 | tie |
| Native-window behavior | NSPanel alpha-hittest, floating, per-pixel click-through | identical AppKit via ObjC shim | tie |
| Activation/focus | accessory+force-activate call; key window + first responder verified | same path in shim | tie |
| IME/text | NSTextInputClient overrides, composing flag, submit guard | identical via callbacks | tie |
| Response rendering | NSTextView stream, zero errors | same | tie |
| Process I/O | thread-queued frames → main-queue dispatch | channel → cgo post → main queue | tie |
| Idle RSS / footprint | 64.9 / 35.7 MB | 71.3 / 38.9 MB | Rust ~6.4/3.2 MB lower |
| Composer-open RSS | 113.0 MB | 100.5 MB | Go ~12.5 MB lower |
| Streaming peak RSS / footprint | 120.1 / 73.2 MB | 122.4 / 81.5 MB | Rust lower |
| Threads idle / streaming | 9 / 12 | 13 / 17 | Rust fewer |
| Idle CPU | 0.0% | 0.0% | tie |
| macOS glue LOC / tokens | 1,778 / 15,314 | 1,671 / 14,721 | Go slightly less, split across more files |
| Binary size | 0.77 MB | 4.12 MB | Rust 5.4× smaller |
| Incremental build | ~9.7 s | ~4.1 s | Go ~2.4× faster |
| Unsafe/FFI surface | Rust `unsafe`+`msg_send!` concentrated in one 1.7k-LOC file | cgo shim in ObjC + `//export` callbacks | Rust: unsafe contained; Go: two-language boundary |
| Build friction | objc2 API discovery + edition-2024 FFI | cgo header ABI + ObjC debugging | comparable |
| Structural risks | none blocking | none blocking | tie |

**Conclusion for the foundation question:** on macOS both candidates are
viable — no structural failure on either side. Rust retains the Stage-A
advantages (smaller idle footprint, fewer threads, 5.4× smaller binary,
unsafe isolated to one file) at the cost of a ~2.4× slower incremental
build and steeper objc2 API-discovery curve. Go trades a larger runtime
footprint and a two-language (Go/ObjC) boundary for faster iteration and
slightly less platform glue. The platform itself introduced no new
risk for either.
