# Source / agent-efficiency report (Protocol v0.1)

Fixture `windows-v1.0.2`; tokenizer `tiktoken/o200k_base` v0.12.0; implementation order Rust -> Zig -> Go (later candidates may have benefited from questions already answered by earlier ones — see notes).

## Whole-project metrics

| Metric | Rust | Zig | Go |
|---|---:|---:|---:|
| Handwritten source bytes | 158299 | 167582 | 126956 |
| Handwritten source chars | 158294 | 167579 | 126951 |
| Nonblank/noncomment LOC | 4528 | 4123 | 4187 |
| Source file count | 9 | 11 | 15 |
| Median source file bytes | 7600 | 7367 | 5964 |
| Largest source file | src/provider.rs | src/provider.zig | provider.go |
| Build/config bytes | 3373 | 2612 | 1367 |
| Architecture doc bytes | 18318 | 15441 | 15680 |
| Source tokens (o200k) | 34801 | 43905 | 36903 |
| Code-only tokens (comments stripped) | 34643 | 42583 | 34805 |
| Tokens / nonblank-noncomment LOC | 7.6857 | 10.6488 | 8.8137 |
| Tokens / source file | 3866.8 | 3991.4 | 2460.2 |
| First-complete snapshot tokens | 28463 | 41848 | 35378 |
| Final - first delta | 6338 | 2057 | 1525 |
| Correction churn +added | 16520 | 2057 | 1822 |
| Correction churn -removed | 10207 | 0 | 296 |
| Focused correction commits | 1 | 1 | 0 |
| Candidate-dir commits total | 4 | 3 | 3 |
| Structural stack changes | 0 | 0 | 0 |
| Benchmark exceptions requested | 0 | 0 | 0 |
| Correction loops observed | 4 | 4 | 2 |
| Clean build ms | 16487 | 17635 | 12396 |
| Incremental build ms | 8581 | 221 | 708 |
| Max function lines | 149 | 221 | 157 |
| Median function lines | 8.0 | 9.0 | 11.0 |
| Max nesting depth | 7 | 10 | 7 |
| FFI/native boundary lines | 102 | 358 | 88 |

## Tokens by subsystem

| Subsystem | Rust | Zig | Go |
|---|---:|---:|---:|
| windowing_platform_glue | 11576 | 14668 | 9876 |
| text_ime_rendering | 2373 | 1913 | 1135 |
| provider_process_io | 11403 | 13258 | 10874 |
| benchmark_candidate_hooks | 1649 | 1524 | 1328 |
| other_candidate_logic | 7800 | 12542 | 13690 |

## Normalized same-function comparison (o200k_base tokens)

| Responsibility | Rust | Zig | Go |
|---|---:|---:|---:|
| create/show/hide transparent mascot window | 2878 | 2854 | 2469 |
| hit testing + dragging | 282 | 354 | 251 |
| global hotkey | 322 | 240 | 302 |
| composer input/IME bridge | 5173 | 4429 | 3640 |
| response rendering | 255 | 264 | 196 |
| provider process launch | 890 | 1170 | 669 |
| framed stdout parser | 3690 | 4626 | 3155 |
| stderr drain | 244 | 310 | 204 |
| cooperative cancellation | 946 | 813 | 613 |
| clean child shutdown | 1437 | 1397 | 1024 |

Region map is committed in `benchmark/harness/build_source_efficiency_report.py` (REGION_MAP); boundaries are at function or case-arm granularity. Window-procedure skeletons count under mascot-window creation; shared hit-test uses system HTCAPTION drag (zero extra drag code).

Dependency delegation (mechanism, not hidden cost): Rust delegates PNG decode to `png` and JSON to `serde_json`; Go delegates to stdlib `image/png`/`encoding/json` + `x/sys` Proc table; Zig delegates PNG decode to WIC (OS COM) and uses handwritten `json.zig`.

## Build / debug friction

| Metric | Rust | Zig | Go |
|---|---|---|---|
| Clean build command | cargo clean && cargo build --release | zig build -Doptimize=ReleaseSafe  (cold .zig-cache) | GOCACHE=<empty> go build -trimpath -buildvcs=false -ldflags='-H windowsgui -s -w -buildid=' -o bin/mascot.exe . |
| Incremental command | cargo build --release  (after touching src/text.rs) | zig build -Doptimize=ReleaseSafe  (after touching src/text.zig) | go build (warm cache) after touching text.go; full pinned build is build.ps1 (rc.exe -> gen_syso.go -> rsrc syso -> go test -> build) |
| Toolchain | rustc/cargo 1.94.0, edition 2024, lto=thin | zig 0.15.2, ReleaseSafe, LTO, windows subsystem | go 1.25.3, CGO_ENABLED=0, -H windowsgui |
| FFI/native setup | windows-sys 0.61.2 (extern declarations via crate); SDK rc.exe pinned for the app manifest resource (build.rs fails if absent) | handwritten win32 extern declarations in src/win32.zig; system libs linked via build.zig; WIC COM for PNG; app.manifest via exe.win32_manifest | syscall via golang.org/x/sys/windows Proc table + handwritten win32 wrappers in win32.go; syso resource produced by rc.exe + custom gen_syso.go because MSVC cvtres output is rejected by the Go linker |
| Dependencies | windows-sys, serde, serde_json, png, base64 (5 direct) | none (std only; handwritten json.zig + extern declarations) | golang.org/x/sys (1 direct; image/png and encoding/json from stdlib) |
| Diagnostic notes | rustc caught real bugs (unused assignments, borrow across re-entrant calls); the failing case was a runtime deadline violation visible only under the shared provider gate | compiler caught const-correctness on atomic.Value pointers; shutdown deadline was a runtime violation found by the gate | go vet/build clean; a console-subsystem build mistake (plain go build instead of -H windowsgui via build.ps1) was caught by the provider gate as a stray conhost child |
| Platform debugging | conhost/NoWindow verified via CREATE_NO_WINDOW; PDH instance naming had to be discovered (process instance strips .exe) | foreground-lock behavior required z-order fix; ~100 MB stable commit reservation measured (arena/queue capacity) | subwindow subclassing needed for Enter/Escape key routing in the RichEdit composer |

### Project-maintained workarounds

**rust** (4): bounded native-handle wait via JoinHandle::as_raw_handle (documented in rust-002); prompt-kill teardown path to satisfy the shared 2s provider deadline; BCrypt SHA-256 via windows-sys instead of a bespoke digest; stale-generation event filtering added after regression run showed stale-session pollution
**zig** (4): extern win32 declaration set maintained by hand (no bindgen); WIC COM vtable calls used for PNG decode; composer raised to top of non-topmost band after SetForegroundWindow foreground-lock failure (zig-acceptance-001); global-hotkey second-launch sequencing discovered via acceptance run
**go** (3): gen_syso.go emits the .syso by hand since cvtres output contains .debug$S/@comp.id symbols the Go linker rejects; explicit CreationFlags CREATE_NO_WINDOW on provider spawn; SendMessageTimeout wrappers used for cross-thread text reads

## Rework / correction evidence

- **rust**: first-complete `6560996`; 4 candidate-dir commits; 1 focused correction commit(s); loops: build+test, smoke(2x 96/192dpi), provider regression fail at rust-provider-001 then pass at 002/003; codex-gate add verified. Note: first buildable snapshot preserved at 6560996; review recorded in corrections/rust-001-review.md; consolidated correction landed in 447ba34 (corrections/rust-002-corrections.md).
- **zig**: first-complete `9dfcf8d`; 3 candidate-dir commits; 1 focused correction commit(s); loops: zig build+test, smoke, provider regression, acceptance cycles (005 -> hidpi -> final); composer z-order fix verified by T1. Note: implementation pass at 9dfcf8d included in-flight fixes found by the shared gates (shutdown timeout, composer z-order, u64 cast); documented in corrections/zig-001-implementation.md.
- **go**: first-complete `bb1804e`; 3 candidate-dir commits; 0 focused correction commit(s); loops: smoke+provider+acceptance passed on first complete binary; gate re-verified after codex_gate.go addition and after the windowsgui rebuild (conhost finding). Note: implementation landed correctness-ready at bb1804e; the only post-pass source changes were the v1.0.2 asset generality and the codex-gate entrypoint (corrections/go-001-implementation.md).

## Explicitly unavailable / caveats

- Model-side prompt/completion token usage is not exposed by the agent platform; not recorded (§5 permits absence).
- `first_complete` = the committed first-buildable snapshot; churn is a committed diff, not a reconstruction of edit history.
- Learning-order caveat: Zig and Go were implemented after Rust; known answers (deadline semantics, DPI quirks, control-host choices) transferred into later candidates' first drafts, so lower Zig/Go correction churn is partially order effect.
- No cross-platform winner is declared; this is Windows Stage A evidence only.
