# Zig Windows candidate architecture

Written before implementation. Fixture `windows-v1.0.1`, freeze commit `49d20a22636db699c034aac1aac3b659931c92fd`. This document makes no correctness or performance claim.

## Stack choice

Use Zig 0.15.2, direct Win32/GDI layered-window presentation, system `RICHEDIT50W` controls for input and response, and Windows Imaging Component for the shared PNG. Zig owns application state, native lifetimes and provider orchestration; mature Windows components own editing, complex-text shaping, fallback and IME. Do not build a custom editor or bind the application to another candidate's UI.

This is independently justified by Zig's direct C ABI/SDK integration, not a requirement for dependency symmetry. WIC avoids writing a PNG codec or adding a general rendering framework, although COM/codec initialization may raise the fresh mascot floor relative to Rust's pure-Rust decoder and Go's standard decoder. That is a real stack choice whose cost must be measured, not hidden or assumed.

Shared native capability evidence is `benchmark/results/windows/preflight/text-stack-004/`: native RichEdit 10.0.26100.8875 produced the exact frozen F1/F2 selection endpoints/copied text, with joined emoji, attached accents and shaped Arabic visible in the response capture. No handwritten grapheme algorithm/ICU dependency is planned. Monochrome rendering is observed; color is not required. This probe is not a Zig acceptance result, and full IME, editing/deletion, clipboard and response gates remain mandatory.

## Windowing and presentation

Set per-monitor-v2 awareness before window creation. Register one mascot class and callbacks once. The mascot uses `WS_POPUP`, layered/topmost/tool-window/no-activate styles, a mascot-sized top-down premultiplied BGRA DIB and `UpdateLayeredWindow`. No screen-sized surface, animation, render thread or idle polling loop.

Use WIC's system COM factory/decoder/frame/converter to obtain 32-bit premultiplied BGRA. Validate the frozen 128x128 dimensions before allocating/copying output. Release COM decoder intermediates after the copy. Retain one small source buffer for scaling/hit testing and the current-DPI DIB; disclose that necessary pair rather than retaining redundant decodes.

Map physical points to source texels with the same nearest mapping used for scaling. Source alpha greater than zero is the exact hit rule. Layered alpha-zero pixels pass clicks to an unrelated underlying process; native caption hit testing/native drag handles the interior. A transparent hit-test return alone is not evidence for cross-process click-through.

Start near the current pointer, clamped to the selected monitor's work area. The common harness positions the pointer for 1x/2x launch checks. Keep the window hidden until effective DPI, size and presentation are correct, then show without focus activation. On `WM_DPICHANGED`, use the suggested rectangle and rescale only DPI-dependent resources. Use frozen 64-DIP mascot and 640x480-DIP composer sizes.

`Ctrl+Alt+Space` with no-repeat explicitly opens/focuses or hides the composer. Register the shared independent cancel hotkey. Registration failure is visible failure, never a different shortcut. The captioned composer is created near the mascot, constrained to the current work area. Native Close hides it while the mascot remains available. Input is 128 DIP, response 272 DIP, outer margins 12 DIP, with remaining space for spacing/status/Send/Cancel.

## Text, IME and warm resources

Both controls enter `TM_PLAINTEXT | TM_MULTILEVELUNDO | TM_MULTICODEPAGE` via `EM_SETTEXTMODE` before receiving text. This is the configuration verified by the native capability probe and avoids importing rich clipboard/OLE content into a plain-text prompt. The provider scenario selector is one-shot: consume it only when a real request is submitted, then restore `normal`.

Initialize COM for WIC on startup; initialize OLE text services on the UI thread at first composer use, balancing both initialization lifetimes. Load only system `msftedit.dll`, not Office/Notepad-private libraries. Use native multiline input and a read-only RichEdit response with 16-DIP Segoe UI, system fallback and advanced typography. Do not downgrade the response renderer.

The response control contains the four fixed history messages followed by the current response. Protocol/control response text excludes the fixed prefix. New requests replace the prior response; keep the prompt after submit. There is no accumulating conversation history.

Track native IME start/update/end. Snapshot committed text and selection before preedit; no provider submit while composing. Let the first composition-owned submit key follow native IME behavior. Only a later explicit non-composing submit sends a request. Hide explicitly cancels IME composition through the supported IMM API and restores the snapshot before hiding. Ordinary Enter remains native multiline input except when the IME owns it. The real candidate must prove exact commit and no accidental newline/duplicate submission; a toolkit quirk is not an approved variation.

Set 4096 UTF-16 input units and 32 undo actions. Cap current response at 262144 UTF-8 bytes plus fixed history, with response undo disabled. Convert UTF-8/UTF-16 at explicit API boundaries and normalize native line separators to logical LF in observations. Native caret, selection, mouse, clipboard, shaping and fallback remain authoritative.

Retain the composer, controls, fonts and text module when hidden. This is a realistic warm-activation policy whose memory is included in warm mascot measurements. Native font/text caches have no claimed project-set eviction cap; their plateau must be tested. There are no project-owned glyph/layout caches.

## Provider and concurrency

Use one UI thread with blocking `GetMessageW`, and posted wakeups only when queued work exists. Native callbacks have stable context pointers and defer semantic model transitions to the main dispatcher; do not hold queue locks during re-entrant native calls.

Use Zig's `std.process.Child` for quoted Windows process launch and pipe setup: `.stdin_behavior/.stdout_behavior/.stderr_behavior = .Pipe`, explicit `.env_map`, frozen `.cwd` and `.create_no_window = true`. The selected 0.15.2 API was checked in the installed standard library. Do not inherit candidate-specific environment into the provider. Launch lazily on first submit, retain across ordinary requests and cancel, and never perform pipe I/O on the GUI thread. The inspected standard implementation uses ordinary inheritable-handle launch rather than an explicit handle list. Clear HANDLE_FLAG_INHERIT on the application's own valid stdin/stdout/stderr handles at startup and create all other application kernel handles non-inheritable; only the child pipe ends may propagate. This native ownership glue must be counted, not hidden as a library guarantee.

A coordinator owns each Child/session and stdin commands; stdout/stderr drain independently. Use a bounded 64-frame UI queue and 16-command coordinator queue with mutex/condition-variable blocking, not busy polling. Stderr is continuously drained to a 4096-byte diagnostic tail/count. Only one request is active. Track a session generation on asynchronous events so a failed old child cannot corrupt its replacement.

The incremental framer never retains more than 65536 bytes including LF, rejects the next byte before increasing storage, and validates UTF-8/JSON only after a full frame. The exact production framer is used by the direct-vector executable mode. Record common QPC at complete-frame LF receipt before JSON decoding; preserve provider decimal timestamp strings. Validate request ID, start/chunk/terminal ordering and sequence, and answer the frozen provider-initiated confirm request.

Ordinary cancel sends a cancel frame and waits for the 1000-ms terminal deadline; it does not kill the child. Hidden streaming continues to update bounded response state. Invalid/oversized frames and unexpected exit fail/invalidate the session and reap it within 2000 ms. Only a new explicit submit starts a replacement; no automatic replay. Use bounded native process waiting for shutdown/failed-session cleanup and a termination fallback only if graceful shutdown fails. Give Child.wait/handle closing one owner, finish draining before closing reader handles, and join threads without blocked queue sends after GUI teardown.

Control input obeys the frozen 16-command bound. Serialize diagnostic/control stdout through a bounded 256-record writer so it cannot block the UI; overflow fails the run rather than silently losing evidence. Control-mode overhead is part of the application totals.

## Allocator, ownership and process inventory

Use `std.heap.smp_allocator` for the multithreaded release application. Use short-lived parsing allocations/arenas whose ordinary lifetime ends after a frame is consumed; do not call benchmark-only purges or switch to a different allocator before samples. Maintain explicit ownership for queue messages, environment maps, UTF-16 buffers, threads, COM references, fonts, selected bitmaps/DCs, windows and native handles. Dispose/reselect native GDI objects in the correct order. Safety-enabled release bounds checks remain on.

There is one application process and no Zig-owned helper process. The identical shared provider child is excluded; all Zig code/data, thread stacks, native allocations, WIC/COM/RichEdit/GDI resources and measurement-hook state remain counted. No hidden C/C++ daemon or shared Rust UI is involved.

## Control serialization

Use the same concrete envelope in every implementation: a command is `{"token":1,"command":"state"}`, with `name` for scenario or `text` for set_text. Replies follow the frozen token/ok/error/state envelope. The text reply additionally carries `"text":{"input":"...","response":"...","selection_start":0,"selection_end":0,"composing":false}`; obtain input and response from the real native controls and exclude only the fixed history prefix from response. A text/state query must not create the composer. set_text requires an already-created input control. Events use `"event":"frame_received"`, `"event":"chunk_accepted"` or `"event":"terminal"` with the frozen fields. Receipt events cover chunk frames, whose provider emission timestamps exist. No extra event is a visible-presentation endpoint.

## Observation

Implement `benchmark/harness/control-v1.json` unchanged. Supply real window handles, bounded queue/state/text data, complete-frame receipt/acceptance events, terminal events and decoder-vector output. Count actual mascot presentation calls and native paint callbacks, disclosing that native caret drawing may not always send `WM_PAINT`.

Neither callback completion nor `UpdateLayeredWindow` returning qualifies visible presentation. The common external screen/compositor observer and separate input-readiness test must be qualified before headline timing. Null with a reason is required where a metric is unavailable.

## Build and dependencies

Pin the existing Zig executable at `W:\devin_folder\tools\zig-x86_64-windows-0.15.2\zig.exe`; target x86_64 Windows GNU using its bundled C headers/import libraries. Use `@cImport` for Windows, controls, RichEdit, IMM, OLE and WIC declarations. Generated SDK declarations are excluded from handwritten counts; all project-owned FFI/adapters are included. No third-party Zig package and no handwritten C/C++ runtime bridge are planned.

System dependencies: kernel32/user32/gdi32/comctl32/imm32/ole32/windowscodecs/msftedit and standard Windows UUID/interface definitions. System libraries are not copied out of Windows for redistribution. Embed an ordinary supported Windows/common-controls application manifest using Zig's supported `win32_manifest` build property, checked in 0.15.2's build implementation.

Release policy: `ReleaseSafe`, LLVM enabled, `want_lto = true`, `root_module.strip = true`, no safety-off blocks to improve scores, no nonstandard allocator cleanup. The pinned build API exposes both properties. Default platform ASLR/NX/security behavior is retained. Any native-linker incompatibility must be reported before changing this configuration.

```powershell
& 'W:\devin_folder\tools\zig-x86_64-windows-0.15.2\zig.exe' build test -Doptimize=ReleaseSafe
& 'W:\devin_folder\tools\zig-x86_64-windows-0.15.2\zig.exe' build -Doptimize=ReleaseSafe -Dtarget=x86_64-windows-gnu -p '..\out\zig'
```

Deploy the final executable as `W:\devin_folder\mascot\out\zig\mascot.exe`; record exact installed layout/build flags. Runtime-payload inventory distinguishes executable/required asset/manifest from OS dependencies and the excluded common fixture.

## Portability and expected tradeoffs

Most native window, hit-test, RichEdit/IMM, hotkey and WIC glue is Windows-specific. Framing and lifecycle state remain portable Zig. A macOS path would need a real AppKit bundle, NSPanel/NSTextView and a small explicit Objective-C/native bridge; C ABI access alone is not proof of focus/IME correctness. Linux needs a demonstrated native text stack such as GTK/Pango. X11 key grabs/input regions are plausible; Wayland placement, always-on-top and global shortcuts require a documented portal/compositor capability floor or accepted degraded policy.

Expected tradeoff, not a result: direct control over bounded allocations/native ownership and no GC, with more explicit C ABI/resource/error handling and a pinned Zig API/toolchain. WIC may affect startup/retained memory. These costs must be measured and maintenance burden counted; they do not establish a language winner.
