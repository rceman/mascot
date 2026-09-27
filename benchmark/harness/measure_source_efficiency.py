"""Source/agent-efficiency measurement per docs/AGENT_SOURCE_EFFICIENCY.md.

Counts candidate-owned handwritten source only: bytes, characters,
nonblank/noncomment LOC, and tokens under the frozen tiktoken/o200k_base
tokenizer. Reports subsystem breakdowns, structural proxies, correction
churn between committed snapshots, and build evidence times. No invented
model-side token usage.
"""
import argparse
import difflib
import json
import pathlib
import statistics
import subprocess
import sys

import tiktoken

TOKENIZER = "o200k_base"
PACKAGE_VERSION = "0.12.0"

# Candidate-owned handwritten source excludes lockfiles, generated bindings,
# build outputs, vendored code and shared fixture files (see protocol §1).
EXCLUDE_NAMES = {"Cargo.lock", "go.sum"}
EXCLUDE_SUFFIXES = {".syso", ".res", ".rc", ".png", ".exe", ".pdb", ".o"}
EXCLUDE_DIRS = {"target", "zig-out", ".zig-cache", "zig-cache", "out", "vendor",
                "node_modules", "__pycache__"}
CONFIG_NAMES = {"Cargo.toml", "go.mod", "build.zig", "config.toml",
                "app.manifest", "rust-toolchain.toml", "build.rs", "go.ps1"}
DOC_SUFFIXES = {".md"}

COMMENT_SYNTAX = {
    ".rs": ("//", (("/*", "*/"),)),
    ".zig": ("//", (("/*", "*/"),)),
    ".go": ("//", (("/*", "*/"),)),
}

SUBSYSTEM_HINTS = {
    "windowing_platform_glue": ("platform", "win", "window", "mascot", "composer",
                                "ui", "wic", "bitmap", "present"),
    "text_ime_rendering": ("text", "ime", "edit", "richedit", "font", "render"),
    "provider_process_io": ("provider", "process", "child", "session", "spawn",
                            "coordinator", "pipe"),
    "benchmark_candidate_hooks": ("control", "hook", "observer", "harness",
                                  "record"),
}
OTHER = "other_candidate_logic"

FFI_HINTS = {
    ".rs": ("unsafe", "windows_sys::", "extern", "SendMessageW("),
    ".zig": ("@cImport", "c.", "extern", "callconv"),
    ".go": ("syscall", "windows.", "unsafe.", "Proc.", "NewLazySystemDLL"),
}

FUNCTION_HINTS = {
    ".rs": ("fn ",), ".zig": ("fn ",), ".go": ("func ",),
}


def included(path):
    parts = set(path.parts)
    if parts & EXCLUDE_DIRS or path.name in EXCLUDE_NAMES:
        return False
    return path.suffix.lower() in COMMENT_SYNTAX or \
        path.name in CONFIG_NAMES or path.suffix.lower() in DOC_SUFFIXES


def strip_comments(text, suffix):
    line_mark, blocks = COMMENT_SYNTAX.get(suffix, ("//", (("/*", "*/"),)))
    out, in_block = [], False
    for line in text.split("\n"):
        result, i = [], 0
        while i < len(line):
            if in_block:
                for start, end in blocks:
                    close = line.find(end, i)
                    if close >= 0:
                        in_block = False
                        i = close + len(end)
                        break
                else:
                    i = len(line)
                continue
            slash = line.find(line_mark, i)
            opened = min((line.find(s, i) for s, _ in blocks if line.find(s, i) >= 0),
                         default=len(line))
            if slash >= 0 and slash < opened:
                result.append(line[i:slash])
                break
            if opened < len(line):
                result.append(line[i:opened])
                for s, _ in blocks:
                    if line.startswith(s, opened):
                        in_block = True
                        i = opened + len(s)
                        break
                continue
            result.append(line[i:])
            break
        out.append("".join(result))
    return "\n".join(out)


def nonblank_noncomment_loc(text, suffix):
    return sum(1 for line in strip_comments(text, suffix).split("\n") if line.strip())


def subsystem(name):
    lowered = name.lower()
    for group, hints in SUBSYSTEM_HINTS.items():
        if any(h in lowered for h in hints):
            return group
    return OTHER


def function_sizes(text, suffix):
    markers = FUNCTION_HINTS.get(suffix)
    if not markers:
        return []
    sizes, current, depth = [], None, 0
    for line in text.split("\n"):
        stripped = line.strip()
        depth += line.count("{") - line.count("}")
        if any(stripped.startswith(m) or f" {m}" in stripped for m in markers):
            current = {"lines": 0, "max_depth": 0}
        if current is not None:
            current["lines"] += 1
            current["max_depth"] = max(current["max_depth"], depth)
            if depth == 0 and current["lines"] > 1:
                sizes.append(current)
                current = None
    if current is not None:
        sizes.append(current)
    return sizes


def ffi_boundary_lines(text, suffix):
    hints = FFI_HINTS.get(suffix, ())
    return sum(1 for line in text.split("\n")
               if any(h in line for h in hints))


def normalize(data):
    return data.decode("utf-8").replace("\r\n", "\n").replace("\r", "\n")


def snapshot_from_disk(src):
    files = {}
    for path in sorted(src.rglob("*")):
        if path.is_file() and included(path.relative_to(src)):
            rel = str(path.relative_to(src)).replace("\\", "/")
            files[rel] = normalize(path.read_bytes())
    return files


def git_prefix(repo):
    """'wsl:DISTRO:PATH' routes through wsl; anything else runs git -C PATH."""
    text = str(repo)
    if text.startswith("wsl:"):
        _, distro, path = text.split(":", 2)
        return ["wsl", "-d", distro, "--", "git", "-C", path]
    return ["git", "-C", text]


def snapshot_from_git(repo, revision, subdir):
    prefix = git_prefix(repo)
    listing = subprocess.run(
        prefix + ["ls-tree", "-r", "--name-only", revision, "--", subdir],
        capture_output=True, check=True, text=True).stdout.split()
    files = {}
    for rel in listing:
        candidate_rel = pathlib.PurePosixPath(rel).relative_to(subdir)
        if not included(candidate_rel):
            continue
        data = subprocess.run(prefix + ["show", f"{revision}:{rel}"],
                              capture_output=True, check=True).stdout
        files[str(candidate_rel)] = normalize(data)
    return files


def measure(files, encoder):
    rows, subsystem_tokens = [], {}
    total_bytes = total_chars = total_loc = total_tokens = total_code_tokens = 0
    code_only_loc = 0
    function_records = []
    ffi_lines = 0
    doc_bytes = config_bytes = 0
    for rel, text in sorted(files.items()):
        suffix = pathlib.PurePosixPath(rel).suffix.lower()
        name = pathlib.PurePosixPath(rel).name
        raw = text.encode("utf-8")
        if suffix in DOC_SUFFIXES:
            doc_bytes += len(raw)
            continue
        if name in CONFIG_NAMES:
            config_bytes += len(raw)
            continue
        if suffix not in COMMENT_SYNTAX:
            continue
        code = strip_comments(text, suffix)
        loc = nonblank_noncomment_loc(text, suffix)
        tokens = len(encoder.encode(text))
        code_tokens = len(encoder.encode(code))
        functions = function_sizes(text, suffix)
        ffi = ffi_boundary_lines(text, suffix)
        group = subsystem(pathlib.PurePosixPath(rel).stem)
        subsystem_tokens[group] = subsystem_tokens.get(group, 0) + tokens
        rows.append({"file": rel, "bytes": len(raw), "chars": len(text),
                     "loc_nonblank_noncomment": loc, "tokens": tokens,
                     "tokens_code_only": code_tokens, "subsystem": group,
                     "functions": len(functions), "ffi_boundary_lines": ffi})
        total_bytes += len(raw)
        total_chars += len(text)
        total_loc += loc
        code_only_loc += loc
        total_tokens += tokens
        total_code_tokens += code_tokens
        function_records += functions
        ffi_lines += ffi
    sizes = [f["lines"] for f in function_records]
    medians = [f["bytes"] for f in rows]
    return {
        "files": rows,
        "handwritten_source_bytes": total_bytes,
        "handwritten_source_chars": total_chars,
        "nonblank_noncomment_loc": total_loc,
        "source_tokens_o200k_base": total_tokens,
        "source_tokens_code_only_o200k_base": total_code_tokens,
        "tokens_per_loc": round(total_tokens / total_loc, 4) if total_loc else None,
        "source_file_count": len(rows),
        "median_source_file_bytes": statistics.median(medians) if medians else None,
        "largest_source_file": max(rows, key=lambda f: f["bytes"])["file"] if rows else None,
        "subsystem_tokens_o200k_base": subsystem_tokens,
        "max_function_lines": max(sizes) if sizes else None,
        "median_function_lines": statistics.median(sizes) if sizes else None,
        "max_nesting_depth": max((f["max_depth"] for f in function_records),
                                 default=None),
        "ffi_boundary_lines": ffi_lines,
        "architecture_doc_bytes": doc_bytes,
        "build_config_bytes": config_bytes,
    }


def churn(before, after, encoder):
    """Token volume added/removed between two committed source snapshots."""
    added = removed = 0
    changed_files = []
    for name in sorted(set(before) | set(after)):
        old = before.get(name, "")
        new = after.get(name, "")
        if old == new:
            continue
        old_lines, new_lines = old.split("\n"), new.split("\n")
        diff = difflib.SequenceMatcher(None, old_lines, new_lines)
        file_added = file_removed = 0
        for tag, i1, i2, j1, j2 in diff.get_opcodes():
            if tag in ("replace", "delete"):
                file_removed += len(encoder.encode("\n".join(old_lines[i1:i2])))
            if tag in ("replace", "insert"):
                file_added += len(encoder.encode("\n".join(new_lines[j1:j2])))
        added += file_added
        removed += file_removed
        changed_files.append({"file": name, "tokens_added": file_added,
                              "tokens_removed": file_removed})
    return {"tokens_added": added, "tokens_removed": removed,
            "changed_files": changed_files}


def build_evidence(paths):
    rows = []
    for path in paths:
        record = json.loads(pathlib.Path(path).read_text(encoding="utf-8"))
        rows.append({"record": str(path), "argv": record.get("argv"),
                     "elapsed_ms": record.get("elapsed_ms"),
                     "exit_code": record.get("exit_code")})
    return rows


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--candidate", required=True)
    parser.add_argument("--src", type=pathlib.Path, required=True,
                        help="Current (final) candidate source directory")
    parser.add_argument("--repo",
                        help="Git repo for committed-snapshot churn; "
                             "'wsl:DISTRO:/abs/path' routes through wsl git")
    parser.add_argument("--subdir", help="Candidate subdir inside repo (e.g. rust)")
    parser.add_argument("--baseline-rev", help="First-complete snapshot revision")
    parser.add_argument("--build-evidence", nargs="*", type=pathlib.Path, default=[],
                        help="record_command command.json paths")
    parser.add_argument("--output", type=pathlib.Path, required=True)
    args = parser.parse_args()

    require_ver = tiktoken.__version__ if hasattr(tiktoken, "__version__") else "unknown"
    if require_ver != PACKAGE_VERSION:
        print(f"ERROR: tiktoken {require_ver} != frozen {PACKAGE_VERSION}; refusing",
              file=sys.stderr)
        sys.exit(64)
    encoder = tiktoken.get_encoding(TOKENIZER)

    final_files = snapshot_from_disk(args.src)
    result = {"schema": "mascot-source-efficiency-1", "candidate": args.candidate,
              "tokenizer": f"tiktoken/{TOKENIZER}", "tokenizer_version": PACKAGE_VERSION,
              "normalization": "committed UTF-8 bytes, LF endings, no minification",
              "exclusions": sorted(EXCLUDE_NAMES) + sorted(EXCLUDE_SUFFIXES) +
                            ["dirs: " + ", ".join(sorted(EXCLUDE_DIRS))],
              "final": measure(final_files, encoder),
              "build_evidence": build_evidence(args.build_evidence),
              "model_token_usage": "not reliably exposed; not recorded (per protocol)"}
    if args.repo and args.baseline_rev:
        baseline = snapshot_from_git(args.repo, args.baseline_rev,
                                     args.subdir or args.candidate)
        result["baseline_revision"] = args.baseline_rev
        result["first_complete"] = measure(baseline, encoder)
        result["correction_churn"] = churn(baseline, final_files, encoder)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2, ensure_ascii=False) + "\n",
                           encoding="utf-8", newline="\n")
    print(json.dumps({"candidate": args.candidate,
                      "source_tokens": result["final"]["source_tokens_o200k_base"],
                      "loc": result["final"]["nonblank_noncomment_loc"],
                      "files": result["final"]["source_file_count"]}))


if __name__ == "__main__":
    main()
