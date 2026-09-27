# Rust Windows candidate architecture

Written before candidate implementation. Shared fixture: `windows-v1.0.1`, freeze commit `49d20a22636db699c034aac1aac3b659931c92fd`. No candidate correctness or performance result is claimed here.

## Decision and evidence

Use direct Win32 windows and the system `msftedit.dll` `RICHEDIT50W` control for both editable input and read-only response. Rust owns orchestration, bounded state and resource lifetime; Windows owns text shaping, bidi, fallback, caret, selection, clipboard and IME. This avoids owning a new editor/renderer while keeping the transparent mascot separate from lazy text initialization.

This is not a decision that all languages must have the same dependency graph. Rust has mature generated Win32 declarations, a mature safe PNG decoder, and strong ownership tools for this particular native slice. A large cross-platform UI layer would not remove the platform-specific transparent-window/hotkey work, and is not needed if system text controls pass the actual fixture. A structural native-control failure remains grounds to reconsider the stack, not to waive a test.

The shared pre-implementation capability investigation is `benchmark/results/windows/preflight/text-stack-004/`. On system RichEdit 10.0.26100.8875, unmodified native navigation/selection produced F1 UTF-16 endpoints 1..3 and F2 endpoints 1..6 with exact copied text. Captures show attached combining marks, joined emoji and shaped Arabic in the read-only control. Rendering is monochrome, not missing sequence data; color is not an acceptance requirement. Full candidate F1-F10, especially IME commit/submit/hide behavior, still require independent evidence. No custom grapheme engine or ICU dependency is planned.

## Windows UI and presentation

- Set per-monitor-v2 process awareness before creating a window.
- Register a process-local mascot class once. Use `WS_POPUP` with `WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE`.
- Decode the shared PNG using `png`, convert once to premultiplied BGRA, and present a mascot-sized top-down DIB through `UpdateLayeredWindow`. There is no full-screen surface, animation, renderer thread or idle timer.
- Retain one source pixel buffer for DPI resampling/hit testing and one current-size DIB; release decoder intermediates. This small second representation is needed for DPI transitions, not an unexplained duplicate cache.
- Use nearest source-texel mapping for both resampling and the exact frozen alpha-greater-than-zero hit rule. Layered-window zero alpha allows cross-process click-through. `WM_NCHITTEST` returns caption inside the mask and transparent outside; native movement/capture implements dragging. Do not rely on `HTTRANSPARENT` alone to pass cross-process W4.
- Initial position is near the current pointer, clamped to that monitor's work area. The shared harness can select a 1x/2x launch by positioning the pointer before process creation, without candidate-private flags. Create initially hidden, obtain effective window DPI, size/present, then show without activation.
- Handle `WM_DPICHANGED` and its suggested rectangle; recreate only the size-dependent DIB/font/layout. Logical dimensions remain those of the manifest.
- Register `Ctrl+Alt+Space` with `MOD_NOREPEAT`; explicitly show/focus the composer or hide it. Register the shared separate cancellation hotkey. A failed registration is an error, not a silent fallback shortcut.

The composer is a normal native captioned window, positioned near the mascot and clamped to the current monitor. Its fixed client size is 640x480 DIP. Use 12-DIP outer margins, 128-DIP editable input and 272-DIP read-only response, with the remaining space for spacing/status/Send/Cancel. The native close button hides rather than terminates the shell. The four fixed history messages precede the current response in the response control; the protocol response string itself excludes that fixed prefix. Each new request replaces the preceding response, never appends an unbounded transcript. Keep the prompt after submission so explicit repeated submissions remain possible.

## Text/IME ownership and first use

Both controls enter `TM_PLAINTEXT | TM_MULTILEVELUNDO | TM_MULTICODEPAGE` via `EM_SETTEXTMODE` before receiving text. This is the configuration verified by the native capability probe and avoids importing rich clipboard/OLE content into a plain-text prompt. The provider scenario selector is one-shot: consume it only when a real request is submitted, then restore `normal`.

Load system RichEdit via `LoadLibraryExW(..., LOAD_LIBRARY_SEARCH_SYSTEM32)` only on first composer use. Initialize OLE on the UI thread then; create input, response, native buttons and the 16-DIP Segoe UI font with system fallback. Use advanced typography and native multiline RichEdit behavior; response uses the same backend, read-only. Do not add Office/Notepad-private DLL dependencies or undocumented D2D switching.

Track `WM_IME_STARTCOMPOSITION`, composition messages and `WM_IME_ENDCOMPOSITION` without substituting programmatic Unicode for IME input. Snapshot committed text and selection when composition begins. Submit is never a provider action while composing; the first submit key is left to native composition handling, and a later explicit non-composing submit sends one request. Hiding explicitly cancels composition via the supported IMM operation, then restores the committed snapshot before hiding. Ordinary Enter is multiline input except when owned by the IME. Any commit-key/newline routing defect must be demonstrated and corrected, not permitted as a variation.

Set the native input limit to 4096 UTF-16 units and bound input undo history to 32 actions. The current response is capped at 262144 UTF-8 bytes plus the small fixed history prefix; disable response undo. Use explicit UTF-16 native API boundaries and normalize native CR/CRLF to logical LF in control snapshots. Real keyboard/mouse/clipboard paths remain native.

Keep the composer/control/font/module resources after hiding as a realistic warm-activation choice. Startup does not pre-create them or launch the provider. Fresh, first-use and retained warm costs therefore remain separately observable. Internal system font/text caches are not claimed to have a known application-configurable cap; stability evidence must establish their behavior. There is no project-owned glyph/layout cache.

## Concurrency, framing and lifecycle

The main thread owns UI/model state and runs blocking `GetMessageW`. Use posted messages to wake it only when work arrives. Win32 callbacks access stable platform handles and short-lived interior-mutability fields; do not hold a Rust `&mut App`, a `RefCell` borrow or a mutex across a Win32 call that can synchronously re-enter a callback. Semantic commands are posted to the main-loop dispatcher rather than re-entering model transitions.

Use `std::process::Command`, explicit `.env_clear().envs(...)`, frozen arguments/cwd and `CREATE_NO_WINDOW` for the shared provider. Launch on first submit; reuse the child for ordinary requests and cancellation. A worker owns child/stdin, with independent stdout and stderr drains. Pipe I/O never runs on the UI thread. Drain stderr continuously into a 4096-byte tail/count, not an unbounded log.

The stdout framer keeps at most 65536 bytes including LF and rejects the next byte before growing the buffer. Validate complete-frame UTF-8 and JSON only after framing. The same framer backs `--decode-vectors`. Record QPC when the complete frame's LF reaches the framer, before JSON decoding, then carry that timestamp with the frame. Preserve the provider's decimal QPC string exactly. Enforce active request ID, start/chunk/terminal order and sequence; automatically answer only the frozen `benchmark.confirm` client request.

Use a bounded 64-frame UI queue and 16-command worker queue. Full provider queues apply backpressure to reader workers, never a spin loop. Control input has the frozen 16-command bound; a separate bounded 256-record output writer keeps diagnostic stdout off the UI thread. A telemetry overflow invalidates the run rather than silently dropping timing/cancel evidence. All instrumentation allocations/threads remain counted.

Only one request is active. Explicit cancel writes a cooperative cancel command and allows the frozen 1000-ms terminal deadline; ordinary cancellation never kills the child. Hide/show does not cancel or stop consuming frames. Invalid/oversized output or unexpected exit produces a failed session, closes/reaps it within 2000 ms, and waits for a new explicit submit before launching another child. Tag asynchronous events with session generation so stale readers cannot mutate a replacement session. Shutdown sends the protocol shutdown command, drains acknowledgement/EOF, and uses a bounded native process wait with termination fallback only for failed shutdown. Join workers and close pipes/handles before final process exit. Never poll an idle child.

## Control serialization

Use the same concrete envelope in every implementation: a command is `{"token":1,"command":"state"}`, with `name` for scenario or `text` for set_text. Replies follow the frozen token/ok/error/state envelope. The text reply additionally carries `"text":{"input":"...","response":"...","selection_start":0,"selection_end":0,"composing":false}`; obtain input and response from the real native controls and exclude only the fixed history prefix from response. A text/state query must not create the composer. set_text requires an already-created input control. Events use `"event":"frame_received"`, `"event":"chunk_accepted"` or `"event":"terminal"` with the frozen fields. Receipt events cover chunk frames, whose provider emission timestamps exist. No extra event is a visible-presentation endpoint.

## Observation and accounting

Implement `benchmark/harness/control-v1.json` unchanged, including real window handles, native paint/present counters, state/text snapshots, chunk receipt/acceptance events and decoder-vector mode. Native caret drawing may not all appear as `WM_PAINT`; disclose that coverage. Internal events and `UpdateLayeredWindow` return times are diagnostics, not independent visible endpoints. Headline timing waits for the shared external observer qualification and separate input-readiness check.

One application process; no candidate helper process. The identical common provider is the only launched child and is excluded by the shared harness. Rust/CRT/native-control/COM allocations, stacks, fonts, GDI/USER/kernel handles and all control-mode overhead belong to application totals.

## Dependencies, build and safety

Toolchain: Rust/Cargo 1.94.0, target `x86_64-pc-windows-msvc`, installed Visual Studio 2022 Build Tools and Windows SDK 10.0.26100.0.

Planned direct crate pins:

| Crate | Version | Purpose |
|---|---|---|
| windows-sys | =0.61.2 | Generated Win32 declarations; candidate-owned unsafe wrappers remain counted |
| serde | =1.0.228 with derive | Typed manifest/control data |
| serde_json | =1.0.150 | Strict structured provider/control JSON |
| png | =0.18.1 | Mature safe PNG decoding without another GUI framework |
| base64 | =0.22.1 | Exact shared direct-decoder vector transport |

These are established releases, not floating latest versions. Commit Cargo.lock, record resolved transitives and use `--locked` after resolution. No custom native C/C++ runtime bridge is planned. SDK resource tooling embeds the ordinary Windows application manifest; all build glue is candidate-owned and counted separately.

Release policy: `opt-level="s"`, thin LTO, one codegen unit, `panic="abort"`, symbol stripping, overflow checks enabled, static MSVC CRT. The default system allocator remains in use. No allocator purge, working-set trim, benchmark-only prewarming or special measurement cleanup. Native unsafe boundaries must have explicit ownership and valid callback lifetimes.

```powershell
cargo +1.94.0 test --locked
cargo +1.94.0 build --release --locked --target x86_64-pc-windows-msvc
```

Deploy the release executable under `W:\devin_folder\mascot\out\rust\mascot.exe`, alongside the same required asset identity/configuration used by the other candidates. Required OS DLLs are system dependencies, not bundled private redistributables. Runtime payload accounting will separately list the executable, any required application manifest/asset and system dependencies; the common provider/tooling is not Rust payload.

## Portability and tradeoffs to validate

Win32 windowing, GDI presentation, RichEdit/IMM, hotkeys and SDK resource glue are Windows-specific. Framing, manifest/control data and most lifecycle state can remain Rust. The macOS path is an actual AppKit bundle with a floating/nonactivating NSPanel plus native NSTextView, using mature Objective-C bindings/limited native glue; it must validate focus/IME/resource lifetime on macOS before eligibility for final selection.

Linux would need a real GTK/Pango text path or another demonstrated native stack. X11 can provide alpha/input regions and global key grabs; ordinary Wayland clients cannot assume arbitrary positioning, always-on-top or global shortcuts. Portal/compositor support and any layer-shell/degraded policy are unresolved platform gates.

Expected tradeoff, not measured result: a small event-driven native dependency surface and Rust-owned lifetime discipline, in exchange for explicit Win32 unsafe/FFI glue and lazy first-use text cost. Nothing here establishes a Windows or cross-platform winner.
