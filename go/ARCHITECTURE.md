# Go Windows candidate architecture

Written before implementation. Fixture `windows-v1.0.1`, freeze commit `49d20a22636db699c034aac1aac3b659931c92fd`. This is a real Go shell design, not a daemon behind another candidate, and contains no measured result.

## Decision and evidence

Use direct Win32 windows/GDI layered presentation and system `RICHEDIT50W` controls for both editable input and read-only response. Use Go's standard PNG, JSON, base64, process and concurrency support, with `golang.org/x/sys/windows` for supported system-DLL calls and native handle types. No cgo is required for this Windows slice.

Gio and other native rendering libraries are allowed, but a GPU/custom-text layer is not required for the narrow mascot plus native editor slice. Direct Win32 lets mature OS text services own shaping, bidi, fallback, caret, selection, clipboard and IME, while Go's ordinary process/concurrency model remains genuinely exercised. This choice is not a dependency-symmetry requirement or a claim that Go has the same memory floor as Rust/Zig.

The shared native investigation at `benchmark/results/windows/preflight/text-stack-004/` recorded exact F1/F2 native selection/copy behavior and visible combining/emoji/Arabic rendering on RichEdit 10.0.26100.8875. Monochrome emoji are observed; color is not required. No custom grapheme engine/ICU dependency is planned. Actual Go IME, navigation/deletion, clipboard and response correctness still require every shared test and screenshot; this investigation grants no candidate PASS.

## Native windows and presentation

Call `runtime.LockOSThread()` for the UI owner before COM/window creation and set per-monitor-v2 awareness. Register process-local classes and static Go callbacks once. Never create a new `windows.NewCallback` for each open/close cycle; callback trampolines are process-lifetime resources.

The mascot is a `WS_POPUP` layered/topmost/tool-window/no-activate HWND. Decode the shared PNG with `image/png`, convert to premultiplied BGRA, present a mascot-sized top-down DIB through `UpdateLayeredWindow`, and let temporary decoder objects become unreachable normally. No forced GC follows initialization. Retain one small source buffer for DPI scaling/hit testing and one current-size native DIB, with no screen-sized backing surface.

Use identical nearest source-texel mapping for display scaling and source-alpha-greater-than-zero hit testing. Layered zero-alpha pixels provide actual cross-process click-through; caption hit testing/native dragging applies only inside the silhouette. Do not accept `HTTRANSPARENT` alone as W4 evidence.

Place the initially hidden mascot near the pointer, clamped to that monitor. Query its effective DPI and present at 64 logical pixels before showing without activation. The common harness can choose 1x/2x launch by positioning the pointer first. Handle suggested rectangles and scaling in `WM_DPICHANGED`; no continuous animation/redraw timer.

Register `Ctrl+Alt+Space` with no-repeat, plus the separate shared cancellation hotkey. Failure is an error, not a private replacement shortcut. Explicit hotkey use opens/focuses or hides the composer. The captioned composer appears near the mascot within the monitor work area; native Close hides it while the shell continues. Use the frozen 640x480-DIP client, 128-DIP input, 272-DIP response and 12-DIP outer margins, with remaining space for spacing/status/Send/Cancel.

## Text/IME and initialization policy

Both controls enter `TM_PLAINTEXT | TM_MULTILEVELUNDO | TM_MULTICODEPAGE` via `EM_SETTEXTMODE` before receiving text. This is the configuration verified by the native capability probe and avoids importing rich clipboard/OLE content into a plain-text prompt. The provider scenario selector is one-shot: consume it only when a real request is submitted, then restore `normal`.

Load only the system RichEdit DLL via the system-DLL loader on first composer use. Initialize OLE on the locked UI thread then. Native multiline input and read-only response share 16-DIP Segoe UI, Windows fallback and advanced typography; no cheaper response-only renderer, Office DLL or undocumented D2D switch.

The four fixed history strings precede the current response in the response control. Model/control response text excludes that fixed prefix. A new request replaces the previous response rather than retaining an unbounded transcript; the prompt stays available after submission.

Track native composition start/update/end and preserve a committed-text/selection snapshot before preedit. Never submit a provider request while composing. Leave the first composition-owned submit action to the native IME and allow exactly one request only on a later explicit non-composing submit. Hiding invokes supported IMM cancellation, restores the committed snapshot and then hides. Ordinary Enter is native multiline input unless owned by composition. Candidate gates must verify exact committed text, including absence of an accidental newline/duplicate request; earlier Windows Forms probe behavior is not an allowed variation.

Enforce 4096 UTF-16 input units and 32 native undo actions. Bound response data to 262144 UTF-8 bytes plus fixed history and disable response undo. Convert native UTF-16 explicitly and normalize CR/CRLF to logical LF for observations, without altering Unicode scalars. Keyboard/mouse selection, clipboard and rendering remain native.

Keep the composer, controls, font and loaded text module after hiding for realistic warm activation. Do not initialize them or the provider during mascot-only startup. Include retained native caches in warm footprint, and do not claim undocumented system caches have a known application-set cap. There is no project-owned glyph/layout cache.

## Go concurrency and provider lifetime

The locked UI thread blocks in `GetMessageW` and wakes through posted messages when work exists. Callbacks defer semantic state changes to the main dispatcher. Other goroutines never access HWND-owned text/model state directly, and no Go mutex is held across re-entrant native calls. Keep callback contexts/pointers alive for the full native window lifetime; destroy controls before releasing native fonts/DCs/bitmaps or callback state.

Use `os/exec.Cmd` with the exact executable/argv/cwd and a replacement eight-key provider environment, plus `CREATE_NO_WINDOW`. Launch only on first submit and retain the child across normal requests/cancellation. Go-only app runtime settings must not leak into the excluded provider.

Use bounded channels: 64 parsed provider frames to the UI and 16 outgoing provider commands. Independent stdout and stderr drains never run on the UI thread. Stderr retention is a 4096-byte tail/count, not an unbounded log. Use a bounded 256-record control/event writer to keep application stdout off the UI thread, with a 16-command control-input limit. Overflow invalidates the run rather than silently dropping evidence. Control-mode goroutines, native threads, queues and allocations are counted.

A persistent framer enforces 65536 bytes including LF before buffer growth. Validate UTF-8 before `encoding/json`, since JSON replacement behavior must not conceal invalid wire UTF-8. Decode only full frames, preserving decimal QPC strings. Capture common QPC when complete-frame LF reaches the framer, before JSON decoding. The same framer implements direct-vector mode. Validate current request ID, start/chunk/terminal order and sequence and respond to only the frozen `benchmark.confirm` client request.

A coordinator owns session transitions/stdin writes. Use `Cmd.Stdout` and `Cmd.Stderr` drain writers, `Cmd.StdinPipe` for the coordinator-owned input, and exactly one `Cmd.Wait` goroutine; os/exec waits for its drain writers before completing. Do not use `StdoutPipe` with an early Wait. Tag events with session generation, preventing a failed old reader from updating a replacement. Only one request is active.

Cancel is a cooperative write with the 1000-ms terminal deadline, not an ordinary process kill. Hidden streaming/cancellation continue. Oversized/invalid output or unexpected exit fail and invalidate the session; reap within 2000 ms and require a new explicit request to launch a fresh child. Shutdown sends the shared command, drains acknowledgement/EOF, waits boundedly and uses termination only for exceptional timeout. Close channels/pipes and join drain/coordinator completion without blocking a sender after GUI teardown. There is no idle child polling loop.

## Runtime/GC and accounting

Pin Go 1.25.3 and use `GOTOOLCHAIN=local` during builds. `CGO_ENABLED=0`. Shipping/benchmark defaults are GOGC 100, GOMEMLIMIT off, empty GODEBUG, and default GOMAXPROCS derived from the OS-visible CPU count; record the effective value for every run. No `GOGC=off`, forced `runtime.GC`, `debug.FreeOSMemory`, working-set trimming or benchmark-only prewarming/cleanup.

Use ordinary bounded Go objects and let the collector/runtime manage unreachable temporary allocations. Windows text/font/bitmap/COM resources need explicit native disposal; Go reachability alone does not release them. One application process, no Go-owned helper process, and one identical shared provider child. All Go runtime/GC memory and CPU, stacks, scheduler/native threads, callback trampolines, native allocations and control-mode overhead are included in application totals.

A separate, non-headline diagnostic pass will report HeapAlloc, HeapSys, HeapInuse, GC count/pause total, goroutines and runtime controls where available. These do not substitute for OS private working set/commit and are not called during only one candidate's headline samples. HeapAlloc is not necessarily reachable memory and GOMEMLIMIT is not a process cap.

## Control serialization

Use the same concrete envelope in every implementation: a command is `{"token":1,"command":"state"}`, with `name` for scenario or `text` for set_text. Replies follow the frozen token/ok/error/state envelope. The text reply additionally carries `"text":{"input":"...","response":"...","selection_start":0,"selection_end":0,"composing":false}`; obtain input and response from the real native controls and exclude only the fixed history prefix from response. A text/state query must not create the composer. set_text requires an already-created input control. Events use `"event":"frame_received"`, `"event":"chunk_accepted"` or `"event":"terminal"` with the frozen fields. Receipt events cover chunk frames, whose provider emission timestamps exist. No extra event is a visible-presentation endpoint.

## Observation

Implement `benchmark/harness/control-v1.json` unchanged, including real native handles, state/text queries, receipt/acceptance/terminal events and direct decoder-vector output. Count actual mascot presentation calls and native paint callbacks. Native caret drawing coverage must be disclosed rather than assuming every blink is a `WM_PAINT`.

App callbacks and `UpdateLayeredWindow` returns are diagnostic only. The same external visible-output observer and input-readiness test must be independently qualified for all candidates before headline timing. Unavailable fields are null with a reason.

## Dependencies and build

Direct module: `golang.org/x/sys v0.36.0` (published 2025-09-05), for generated Windows support/system-DLL loading. Everything else uses Go 1.25.3 standard packages. Commit go.mod/go.sum. Native dependencies are ordinary Windows user32/gdi32/comctl32/imm32/ole32/msftedit/kernel32; none is redistributed as a private copied system DLL. There is no handwritten C/C++ bridge; Go ABI structs/callback/procedure glue remain candidate-owned source and are counted.

Embed a supported Windows/common-controls application manifest through SDK resource compilation and MSVC resource-to-COFF conversion, producing a build-generated `.syso` linked by Go. Build/config glue is counted separately; the generated resource object is excluded from handwritten source. Preserve default platform security behavior.

Release intent: default Go optimized compiler with bounds checks/runtime safety, no `-N`/`-l`, path trimming, VCS injection disabled, GUI subsystem, symbol/DWARF stripping and empty build ID. Go has no separate user-selected LTO switch here.

```powershell
$env:GOTOOLCHAIN='local'
$env:CGO_ENABLED='0'
go test ./...
go build -trimpath -buildvcs=false -ldflags='-H windowsgui -s -w -buildid=' -o '..\out\go\mascot.exe' .
```

The build script performs the pinned SDK resource step first. Deploy to `W:\devin_folder\mascot\out\go\mascot.exe`; enumerate executable/required asset/manifest separately from system dependencies and excluded fixture tooling in runtime payload results.

## Other platforms and tradeoffs

The Win32/RichEdit/IMM/hotkey layer is Windows-specific, not a cross-platform Go UI. Framing/state/process orchestration can be retained. macOS needs an actual AppKit bundle with NSPanel/NSTextView and an explicit Objective-C/cgo bridge, including native allocation/focus/IME tests; Windows no-cgo does not imply macOS no-cgo. Linux would need a demonstrated native text stack such as GTK/Pango. X11 alpha/input regions/global key grabs have a path; Wayland placement/always-on-top/global shortcuts depend on portal/compositor support and an explicitly accepted capability floor.

Expected tradeoff, not a measurement: simpler orchestration and mature standard packages with a Go runtime/GC footprint plus handwritten Win32 marshaling. The measurement must show the actual premium, retained native cache behavior and engineering cost. No Windows or final cross-platform winner is selected here.
