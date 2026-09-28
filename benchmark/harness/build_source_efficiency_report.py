#!/usr/bin/env python3
"""Assemble Phase 11 source-efficiency deliverables per
docs/AGENT_SOURCE_EFFICIENCY.md:

- benchmark/results/windows/source-efficiency.json (machine-readable)
- benchmark/results/windows/source-efficiency.md   (human report)

Inputs: per-candidate *-source-efficiency.json (from
measure_source_efficiency.py), the committed candidate sources on disk
(== HEAD), and curated rework/build evidence below, each entry citing its
source. Nothing is invented; unmeasurable fields are explicitly marked.
"""
import json
import pathlib
import subprocess
import sys

import tiktoken

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
RAW = ROOT / "benchmark" / "results" / "windows" / "raw"
OUT_DIR = ROOT / "benchmark" / "results" / "windows"
OUT_JSON = OUT_DIR / "source-efficiency.json"
OUT_MD = OUT_DIR / "source-efficiency.md"

RESPONSIBILITIES = [
    ("create_show_hide_transparent_mascot_window",
     "create/show/hide transparent mascot window"),
    ("hit_testing_dragging", "hit testing + dragging"),
    ("global_hotkey", "global hotkey"),
    ("composer_input_ime_bridge", "composer input/IME bridge"),
    ("response_rendering", "response rendering"),
    ("provider_process_launch", "provider process launch"),
    ("framed_stdout_parser", "framed stdout parser"),
    ("stderr_drain", "stderr drain"),
    ("cooperative_cancellation", "cooperative cancellation"),
    ("clean_child_shutdown", "clean child shutdown"),
]

# Homologous region map: file + inclusive 1-based line range against the
# committed final source (HEAD == deployed binaries). `None` = whole file.
# Attribution notes per entry are in REGION_NOTES.
REGION_MAP = {
    "rust": {
        "create_show_hide_transparent_mascot_window": [
            ["src/platform.rs", 60, 208],   # Surface struct + impl + Drop
            ["src/platform.rs", 290, 347],  # present_mascot
            ["src/platform.rs", 975, 1010], # mascot_proc skeleton/app arms
            ["src/platform.rs", 1099, 1110],# WM_CLOSE/WM_DESTROY/default
            ["src/platform.rs", 1432, 1456],# decode_png
            ["src/platform.rs", 1458, 1494],# register_classes
            ["src/platform.rs", 1496, 1540],# create_mascot
        ],
        "hit_testing_dragging": [
            ["src/platform.rs", 1030, 1055],  # WM_NCHITTEST arm; drag is
                                            # system HTCAPTION (zero extra code)
        ],
        "global_hotkey": [
            ["src/main.rs", 150, 170],        # RegisterHotKey pair
            ["src/main.rs", 230, 236],        # UnregisterHotKey pair
            ["src/platform.rs", 1011, 1029],  # WM_HOTKEY arm
        ],
        "composer_input_ime_bridge": [
            ["src/text.rs", None, None],      # whole text/IME bridge module
            ["src/platform.rs", 1112, 1341],  # composer_proc + create_children
            ["src/platform.rs", 1343, 1427],  # relayout
            ["src/platform.rs", 427, 463],    # enforce_composer_client
            ["src/platform.rs", 235, 257],    # consume_submit_key
            ["src/platform.rs", 213, 233],    # composing + on_ime_* hooks
        ],
        "response_rendering": [
            ["src/platform.rs", 36, 47],      # append_bounded
            ["src/platform.rs", 576, 599],    # reset/append response + status
        ],
        "provider_process_launch": [
            ["src/provider.rs", 187, 223],    # Provider::spawn
            ["src/provider.rs", 282, 361],    # spawn_child
        ],
        "framed_stdout_parser": [
            ["src/framing.rs", None, None],   # bounded NDJSON decoder
            ["src/provider.rs", 855, 973],    # stdout_run
            ["src/provider.rs", 975, 1240],   # handle_frame (protocol+dispatch)
        ],
        "stderr_drain": [
            ["src/provider.rs", 163, 178],    # StderrTail
            ["src/provider.rs", 334, 353],    # stderr reader thread
        ],
        "cooperative_cancellation": [
            ["src/platform.rs", 549, 574],    # cancel_request
            ["src/provider.rs", 694, 810],    # Command::Cancel arm in
                                              # coordinator_run
        ],
        "clean_child_shutdown": [
            ["src/provider.rs", 384, 401],    # wait_handle + join_readers
            ["src/provider.rs", 403, 464],    # teardown
            ["src/provider.rs", 508, 569],    # graceful_shutdown
            ["src/platform.rs", 912, 932],    # request_shutdown
            ["src/platform.rs", 934, 972],    # platform teardown
        ],
    },
    "zig": {
        "create_show_hide_transparent_mascot_window": [
            ["src/platform.zig", 48, 96],      # Surface create/deinit
            ["src/platform.zig", 190, 254],    # registerClasses + createMascot
            ["src/platform.zig", 256, 291],    # presentMascot
            ["src/platform.zig", 1250, 1289],  # decodePng (WIC)
            ["src/platform.zig", 803, 823],    # mascotProc skeleton/app arms
            ["src/platform.zig", 834, 849],    # menu/close/destroy arms
        ],
        "hit_testing_dragging": [
            ["src/platform.zig", 293, 310],    # hitTest
        ],
        "global_hotkey": [
            ["src/main.zig", 214, 226],        # Register/UnregisterHotKey
            ["src/platform.zig", 824, 833],    # WM_HOTKEY arm
        ],
        "composer_input_ime_bridge": [
            ["src/text.zig", None, None],      # text/IME bridge module
            ["src/platform.zig", 861, 919],    # composerProc
            ["src/platform.zig", 927, 956],    # controlSubclassProc
            ["src/platform.zig", 958, 967],    # onImeStart
            ["src/platform.zig", 456, 532],    # createChildren
            ["src/platform.zig", 419, 431],    # enforceComposerClient
            ["src/platform.zig", 438, 454],    # relayout
        ],
        "response_rendering": [
            ["src/platform.zig", 164, 167],    # appendBounded
            ["src/platform.zig", 759, 769],    # reset/append response view
            ["src/platform.zig", 969, 978],    # refreshStatus
        ],
        "provider_process_launch": [
            ["src/provider.zig", 226, 267],    # Provider.spawn
            ["src/provider.zig", 329, 427],    # spawnChild
        ],
        "framed_stdout_parser": [
            ["src/framing.zig", None, None],   # bounded NDJSON decoder
            ["src/provider.zig", 805, 893],    # stdoutRun
            ["src/provider.zig", 966, 1129],   # handleFrame
            ["src/provider.zig", 921, 937],    # buildFrameRecord
        ],
        "stderr_drain": [
            ["src/provider.zig", 129, 155],    # StderrTail
            ["src/provider.zig", 899, 908],    # stderrRun
        ],
        "cooperative_cancellation": [
            ["src/platform.zig", 771, 781],    # cancelRequest
            ["src/provider.zig", 689, 760],    # .cancel arm in coordinatorRun
        ],
        "clean_child_shutdown": [
            ["src/provider.zig", 460, 533],    # teardown
            ["src/provider.zig", 542, 581],    # gracefulShutdown
            ["src/provider.zig", 299, 303],    # join
            ["src/platform.zig", 783, 799],    # requestShutdown
            ["src/platform.zig", 1220, 1244],  # teardown
        ],
    },
    "go": {
        "create_show_hide_transparent_mascot_window": [
            ["ui.go", 117, 146],               # decodePNG (stdlib image/png)
            ["ui.go", 148, 204],               # newSurface + destroy
            ["ui.go", 206, 245],               # presentMascot
            ["wndproc.go", 35, 59],            # mascotWndProc skeleton
            ["wndproc.go", 98, 138],           # dpi/menu/close/destroy arms
            ["wndproc.go", 404, 458],          # registerClasses + createMascot
        ],
        "hit_testing_dragging": [
            ["wndproc.go", 76, 97],            # wmNCHITTEST case
        ],
        "global_hotkey": [
            ["main.go", 158, 166],             # RegisterHotKey pair
            ["main.go", 190, 193],             # UnregisterHotKey pair
            ["wndproc.go", 60, 75],            # wmHOTKEY case
        ],
        "composer_input_ime_bridge": [
            ["text.go", None, None],           # edit/IME bridge module
            ["wndproc.go", 140, 198],          # composerWndProc
            ["wndproc.go", 361, 402],          # controlSubclass + subclassControl
            ["wndproc.go", 200, 312],          # createChildren
            ["wndproc.go", 314, 356],          # relayout
            ["ui.go", 819, 831],               # onIME* hooks
        ],
        "response_rendering": [
            ["ui.go", 91, 97],                 # appendBounded
            ["ui.go", 426, 442],               # reset/append response + status
        ],
        "provider_process_launch": [
            ["provider.go", 230, 255],         # spawnProvider
            ["provider.go", 320, 381],         # spawnChild
        ],
        "framed_stdout_parser": [
            ["framing.go", None, None],        # bounded NDJSON decoder
            ["provider.go", 704, 777],         # stdoutSink.Write + finish
            ["provider.go", 817, 1103],        # handleFrame
        ],
        "stderr_drain": [
            ["provider.go", 155, 181],         # stderrTail + stderrSink.Write
        ],
        "cooperative_cancellation": [
            ["ui.go", 409, 424],               # cancelRequest
            ["provider.go", 280, 285],         # sendCancel
            ["provider.go", 603, 652],         # workCancel arm in run()
        ],
        "clean_child_shutdown": [
            ["provider.go", 294, 301],         # join
            ["provider.go", 398, 450],         # teardown
            ["provider.go", 494, 529],         # gracefulShutdown
            ["ui.go", 736, 754],               # requestShutdown
            ["ui.go", 756, 787],               # teardown
        ],
    },
}

# Rework/commit evidence derived from `git log -- <candidate>/` in the
# committed branch (verified at report build time). Doc-only commits
# (ARCHITECTURE.md contracts) are excluded from correction counts.
REWORK = {
    "rust": {
        "first_complete_commit": "6560996",
        "candidate_dir_commits_total": 4,
        "focused_correction_commits": 1,   # 447ba34 (consolidated review fixes)
        "post_ready_source_commits": 2,    # 757a22f asset-generic, 76303a4 gate
        "first_complete_note":
            "first buildable snapshot preserved at 6560996; review recorded "
            "in corrections/rust-001-review.md; consolidated correction landed "
            "in 447ba34 (corrections/rust-002-corrections.md)",
        "structural_stack_changes": 0,
        "benchmark_exceptions_requested": 0,
        "correction_loop_count": 4,
        "correction_loop_evidence":
            "build+test, smoke(2x 96/192dpi), provider regression fail at "
            "rust-provider-001 then pass at 002/003; codex-gate add verified",
    },
    "zig": {
        "first_complete_commit": "9dfcf8d",
        "candidate_dir_commits_total": 3,
        "focused_correction_commits": 1,   # composer z-order fix inside
        # implementation pass; separate asset+gate commits counted below
        "post_ready_source_commits": 2,    # 757a22f asset-generic, 76303a4 gate
        "first_complete_note":
            "implementation pass at 9dfcf8d included in-flight fixes found by "
            "the shared gates (shutdown timeout, composer z-order, u64 cast); "
            "documented in corrections/zig-001-implementation.md",
        "structural_stack_changes": 0,
        "benchmark_exceptions_requested": 0,
        "correction_loop_count": 4,
        "correction_loop_evidence":
            "zig build+test, smoke, provider regression, acceptance cycles "
            "(005 -> hidpi -> final); composer z-order fix verified by T1",
    },
    "go": {
        "first_complete_commit": "bb1804e",
        "candidate_dir_commits_total": 3,
        "focused_correction_commits": 0,   # passed gates on first impl;
        # later commits are feature-add (gate) and asset-generality, not fixes
        "post_ready_source_commits": 2,    # 757a22f asset-generic, 76303a4 gate
        "first_complete_note":
            "implementation landed correctness-ready at bb1804e; the only "
            "post-pass source changes were the v1.0.2 asset generality and "
            "the codex-gate entrypoint (corrections/go-001-implementation.md)",
        "structural_stack_changes": 0,
        "benchmark_exceptions_requested": 0,
        "correction_loop_count": 2,
        "correction_loop_evidence":
            "smoke+provider+acceptance passed on first complete binary; "
            "gate re-verified after codex_gate.go addition and after the "
            "windowsgui rebuild (conhost finding)",
    },
}

# Build/friction evidence: times measured 2026-09-28 on the benchmark host
# (i9-9900K, Balanced power). Clean = no compiler cache; incremental =
# single source file touched, warm cache.
BUILD = {
    "rust": {
        "clean_build_cmd": "cargo clean && cargo build --release",
        "incremental_build_cmd": "cargo build --release  (after touching src/text.rs)",
        "clean_build_ms": 16487,
        "incremental_build_ms": 8581,
        "toolchain": "rustc/cargo 1.94.0, edition 2024, lto=thin",
        "ffi_setup": "windows-sys 0.61.2 (extern declarations via crate); "
                     "SDK rc.exe pinned for the app manifest resource "
                     "(build.rs fails if absent)",
        "deps": "windows-sys, serde, serde_json, png, base64 (5 direct)",
        "workarounds": [
            "bounded native-handle wait via JoinHandle::as_raw_handle "
            "(documented in rust-002)",
            "prompt-kill teardown path to satisfy the shared 2s provider "
            "deadline",
            "BCrypt SHA-256 via windows-sys instead of a bespoke digest",
            "stale-generation event filtering added after regression run "
            "showed stale-session pollution",
        ],
        "diagnostic_notes":
            "rustc caught real bugs (unused assignments, borrow across "
            "re-entrant calls); the failing case was a runtime deadline "
            "violation visible only under the shared provider gate",
        "platform_debug_notes":
            "conhost/NoWindow verified via CREATE_NO_WINDOW; PDH instance "
            "naming had to be discovered (process instance strips .exe)",
    },
    "zig": {
        "clean_build_cmd": "zig build -Doptimize=ReleaseSafe  (cold .zig-cache)",
        "incremental_build_cmd": "zig build -Doptimize=ReleaseSafe  (after touching src/text.zig)",
        "clean_build_ms": 17635,
        "incremental_build_ms": 221,
        "toolchain": "zig 0.15.2, ReleaseSafe, LTO, windows subsystem",
        "ffi_setup": "handwritten win32 extern declarations in src/win32.zig; "
                     "system libs linked via build.zig; WIC COM for PNG; "
                     "app.manifest via exe.win32_manifest",
        "deps": "none (std only; handwritten json.zig + extern declarations)",
        "workarounds": [
            "extern win32 declaration set maintained by hand (no bindgen)",
            "WIC COM vtable calls used for PNG decode",
            "composer raised to top of non-topmost band after "
            "SetForegroundWindow foreground-lock failure (zig-acceptance-001)",
            "global-hotkey second-launch sequencing discovered via "
            "acceptance run",
        ],
        "diagnostic_notes":
            "compiler caught const-correctness on atomic.Value pointers; "
            "shutdown deadline was a runtime violation found by the gate",
        "platform_debug_notes":
            "foreground-lock behavior required z-order fix; ~100 MB stable "
            "commit reservation measured (arena/queue capacity)",
    },
    "go": {
        "clean_build_cmd":
            "GOCACHE=<empty> go build -trimpath -buildvcs=false "
            "-ldflags='-H windowsgui -s -w -buildid=' -o bin/mascot.exe .",
        "incremental_build_cmd":
            "go build (warm cache) after touching text.go; full pinned build "
            "is build.ps1 (rc.exe -> gen_syso.go -> rsrc syso -> go test -> "
            "build)",
        "clean_build_ms": 12396,
        "incremental_build_ms": 708,
        "toolchain": "go 1.25.3, CGO_ENABLED=0, -H windowsgui",
        "ffi_setup": "syscall via golang.org/x/sys/windows Proc table + "
                     "handwritten win32 wrappers in win32.go; syso resource "
                     "produced by rc.exe + custom gen_syso.go because MSVC "
                     "cvtres output is rejected by the Go linker",
        "deps": "golang.org/x/sys (1 direct; image/png and encoding/json "
                "from stdlib)",
        "workarounds": [
            "gen_syso.go emits the .syso by hand since cvtres output "
            "contains .debug$S/@comp.id symbols the Go linker rejects",
            "explicit CreationFlags CREATE_NO_WINDOW on provider spawn",
            "SendMessageTimeout wrappers used for cross-thread text reads",
        ],
        "diagnostic_notes":
            "go vet/build clean; a console-subsystem build mistake "
            "(plain go build instead of -H windowsgui via build.ps1) was "
            "caught by the provider gate as a stray conhost child",
        "platform_debug_notes":
            "subwindow subclassing needed for Enter/Escape key routing in "
            "the RichEdit composer",
    },
}


def load_snapshot(candidate):
    """Final committed source files (rel path -> text) from disk."""
    files = {}
    src = ROOT / candidate
    for path in sorted(src.rglob("*")):
        if not path.is_file():
            continue
        rel = str(path.relative_to(src)).replace("\\", "/")
        try:
            files[rel] = path.read_bytes().decode("utf-8") \
                .replace("\r\n", "\n").replace("\r", "\n")
        except UnicodeDecodeError:
            pass
    return files


def region_tokens(files, encoder, regions):
    total = 0
    covered = []
    for entry in regions:
        rel, a, b = entry
        text = files.get(rel)
        if text is None:
            raise KeyError(f"mapping references missing file {rel}")
        if a is None:
            slice_ = text
        else:
            lines = text.split("\n")
            slice_ = "\n".join(lines[a - 1:b])
        total += len(encoder.encode(slice_))
        covered.append({"file": rel,
                        "lines": "all" if a is None else [a, b],
                        "tokens": len(encoder.encode(slice_))})
    return total, covered


def main():
    if tiktoken.__version__ != "0.12.0":
        print("tiktoken version mismatch", file=sys.stderr)
        sys.exit(64)
    encoder = tiktoken.get_encoding("o200k_base")

    per_candidate = {}
    for cand in ("rust", "zig", "go"):
        eff = json.loads((RAW / f"{cand}-source-efficiency.json")
                         .read_text(encoding="utf-8"))
        files = load_snapshot(cand)
        regions = {}
        for key, label in RESPONSIBILITIES:
            tokens, covered = region_tokens(files, encoder,
                                            REGION_MAP[cand][key])
            regions[key] = {"label": label, "tokens_o200k_base": tokens,
                            "regions": covered}
        per_candidate[cand] = {
            "final": eff["final"],
            "first_complete": eff.get("first_complete"),
            "correction_churn": eff.get("correction_churn"),
            "homologous_regions": regions,
            "rework": REWORK[cand],
            "build": BUILD[cand],
        }

    doc = {
        "schema": "mascot-source-efficiency-report-1",
        "protocol": "docs/AGENT_SOURCE_EFFICIENCY.md v0.1",
        "tokenizer": "tiktoken/o200k_base 0.12.0",
        "scope": ("candidate-owned handwritten implementation under rust/, "
                  "zig/, go/; shared benchmark/, assets/, lockfiles, "
                  "generated resources and build outputs excluded"),
        "implementation_order": ["rust", "zig", "go"],
        "candidates": per_candidate,
        "unavailable": {
            "model_prompt_completion_tokens":
                "the agent platform does not expose per-phase model token "
                "usage; not recorded (protocol §5 allows absence)",
            "first_complete_tokens_note":
                "first-complete snapshots are committed pre-correction "
                "states (rust=6560996, zig=9dfcf8d, go=bb1804e); churn is "
                "computed as a committed diff, not reconstructed usage",
        },
    }
    OUT_JSON.write_text(json.dumps(doc, indent=2, ensure_ascii=False) + "\n",
                       encoding="utf-8", newline="\n")

    f = lambda c, k: per_candidate[c]["final"].get(k)
    fc = lambda c, k: ((per_candidate[c]["first_complete"] or {}).get(k))
    ch = lambda c, k: ((per_candidate[c]["correction_churn"] or {}).get(k))
    rw = lambda c, k: per_candidate[c]["rework"].get(k)
    bd = lambda c, k: per_candidate[c]["build"].get(k)
    sub = lambda c, k: per_candidate[c]["final"]["subsystem_tokens_o200k_base"] \
        .get(k, 0)
    reg = lambda c, k: per_candidate[c]["homologous_regions"][k][
        "tokens_o200k_base"]

    lines = ["# Source / agent-efficiency report (Protocol v0.1)", "",
        "Fixture `windows-v1.0.2`; tokenizer `tiktoken/o200k_base` v0.12.0; "
        "implementation order Rust -> Zig -> Go (later candidates may have "
        "benefited from questions already answered by earlier ones — see "
        "notes).", "",
        "## Whole-project metrics", "",
        "| Metric | Rust | Zig | Go |", "|---|---:|---:|---:|"]
    rows = [
        ("Handwritten source bytes", lambda c: f(c, "handwritten_source_bytes")),
        ("Handwritten source chars", lambda c: f(c, "handwritten_source_chars")),
        ("Nonblank/noncomment LOC", lambda c: f(c, "nonblank_noncomment_loc")),
        ("Source file count", lambda c: f(c, "source_file_count")),
        ("Median source file bytes", lambda c: f(c, "median_source_file_bytes")),
        ("Largest source file", lambda c: f(c, "largest_source_file")),
        ("Build/config bytes", lambda c: f(c, "build_config_bytes")),
        ("Architecture doc bytes", lambda c: f(c, "architecture_doc_bytes")),
        ("Source tokens (o200k)", lambda c: f(c, "source_tokens_o200k_base")),
        ("Code-only tokens (comments stripped)",
         lambda c: f(c, "source_tokens_code_only_o200k_base")),
        ("Tokens / nonblank-noncomment LOC", lambda c: f(c, "tokens_per_loc")),
        ("Tokens / source file",
         lambda c: round(f(c, "source_tokens_o200k_base") /
                         f(c, "source_file_count"), 1)),
        ("First-complete snapshot tokens", lambda c: fc(c, "source_tokens_o200k_base")),
        ("Final - first delta",
         lambda c: f(c, "source_tokens_o200k_base") - fc(c, "source_tokens_o200k_base")),
        ("Correction churn +added", lambda c: ch(c, "tokens_added")),
        ("Correction churn -removed", lambda c: ch(c, "tokens_removed")),
        ("Focused correction commits", lambda c: rw(c, "focused_correction_commits")),
        ("Candidate-dir commits total", lambda c: rw(c, "candidate_dir_commits_total")),
        ("Structural stack changes", lambda c: rw(c, "structural_stack_changes")),
        ("Benchmark exceptions requested",
         lambda c: rw(c, "benchmark_exceptions_requested")),
        ("Correction loops observed", lambda c: rw(c, "correction_loop_count")),
        ("Clean build ms", lambda c: bd(c, "clean_build_ms")),
        ("Incremental build ms", lambda c: bd(c, "incremental_build_ms")),
        ("Max function lines", lambda c: f(c, "max_function_lines")),
        ("Median function lines", lambda c: f(c, "median_function_lines")),
        ("Max nesting depth", lambda c: f(c, "max_nesting_depth")),
        ("FFI/native boundary lines", lambda c: f(c, "ffi_boundary_lines")),
    ]
    for label, getter in rows:
        r = [str(getter(c)) for c in ("rust", "zig", "go")]
        lines.append("| " + label + " | " + " | ".join(r) + " |")

    lines += ["", "## Tokens by subsystem", "",
              "| Subsystem | Rust | Zig | Go |", "|---|---:|---:|---:|"]
    for key in ("windowing_platform_glue", "text_ime_rendering",
                "provider_process_io", "benchmark_candidate_hooks",
                "other_candidate_logic"):
        lines.append("| {} | {} | {} | {} |".format(
            key, sub("rust", key), sub("zig", key), sub("go", key)))

    lines += ["", "## Normalized same-function comparison (o200k_base tokens)",
              "", "| Responsibility | Rust | Zig | Go |", "|---|---:|---:|---:|"]
    for key, label in RESPONSIBILITIES:
        lines.append("| {} | {} | {} | {} |".format(
            label, reg("rust", key), reg("zig", key), reg("go", key)))
    lines += ["",
        "Region map is committed in "
        "`benchmark/harness/build_source_efficiency_report.py` "
        "(REGION_MAP); boundaries are at function or case-arm granularity. "
        "Window-procedure skeletons count under mascot-window creation; "
        "shared hit-test uses system HTCAPTION drag (zero extra drag code).",
        "",
        "Dependency delegation (mechanism, not hidden cost): Rust delegates "
        "PNG decode to `png` and JSON to `serde_json`; Go delegates to "
        "stdlib `image/png`/`encoding/json` + `x/sys` Proc table; Zig "
        "delegates PNG decode to WIC (OS COM) and uses handwritten "
        "`json.zig`.", "",
        "## Build / debug friction", "",
        "| Metric | Rust | Zig | Go |", "|---|---|---|---|"]
    for label, key in (("Clean build command", "clean_build_cmd"),
                       ("Incremental command", "incremental_build_cmd"),
                       ("Toolchain", "toolchain"),
                       ("FFI/native setup", "ffi_setup"),
                       ("Dependencies", "deps"),
                       ("Diagnostic notes", "diagnostic_notes"),
                       ("Platform debugging", "platform_debug_notes")):
        lines.append("| {} | {} | {} | {} |".format(
            label, bd("rust", key), bd("zig", key), bd("go", key)))
    lines += ["", "### Project-maintained workarounds", ""]
    for c in ("rust", "zig", "go"):
        lines.append(f"**{c}** ({len(BUILD[c]['workarounds'])}): " +
                     "; ".join(BUILD[c]["workarounds"]))
    lines += ["", "## Rework / correction evidence", ""]
    for c in ("rust", "zig", "go"):
        r = per_candidate[c]["rework"]
        lines.append(f"- **{c}**: first-complete `{r['first_complete_commit']}`"
                     f"; {r['candidate_dir_commits_total']} candidate-dir "
                     f"commits; {r['focused_correction_commits']} focused "
                     f"correction commit(s); loops: "
                     f"{r['correction_loop_evidence']}. "
                     f"Note: {r['first_complete_note']}.")
    lines += ["", "## Explicitly unavailable / caveats", "",
        "- Model-side prompt/completion token usage is not exposed by the "
        "agent platform; not recorded (§5 permits absence).",
        "- `first_complete` = the committed first-buildable snapshot; churn "
        "is a committed diff, not a reconstruction of edit history.",
        "- Learning-order caveat: Zig and Go were implemented after Rust; "
        "known answers (deadline semantics, DPI quirks, control-host "
        "choices) transferred into later candidates' first drafts, so lower "
        "Zig/Go correction churn is partially order effect.",
        "- No cross-platform winner is declared; this is Windows Stage A "
        "evidence only.",
        ""]
    OUT_MD.write_text("\n".join(lines), encoding="utf-8", newline="\n")
    print(f"wrote {OUT_JSON}")
    print(f"wrote {OUT_MD}")


if __name__ == "__main__":
    main()
