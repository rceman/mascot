//! `stamp-evidence <dir> [<dir>...]` — set `evidence_head` in evidence-dir
//! receipts (and perf.json) to the current HEAD. Fails unless the tree is
//! clean and the only changes since each receipt's `code_head` are inside
//! `benchmark/results/`.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn git_ok(args: &[&str]) -> bool {
    Command::new("git")
        .args(args)
        .current_dir(crate::capture::repo_root())
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn rel_to_repo(root: &Path, p: &Path) -> Result<String, String> {
    let abs = if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir().map_err(|e| e.to_string())?.join(p)
    };
    let abs = abs
        .canonicalize()
        .map_err(|e| format!("{}: {e}", abs.display()))?;
    // canonicalize both sides: git prints `W:/…`, canonicalize `\\?\W:\…`
    let root = root
        .canonicalize()
        .map_err(|e| format!("{}: {e}", root.display()))?;
    abs.strip_prefix(&root)
        .map(|r| r.to_string_lossy().replace('\\', "/"))
        .map_err(|_| format!("{}: outside the repository", abs.display()))
}

fn stamp_json(path: &Path, head: &str) -> Result<(), String> {
    // byte-exact text substitution: a serde round-trip would re-print floats
    // and the stamp commit must change only the evidence_head field
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    const UNSTAMPED: &str = "\"evidence_head\": null";
    if text.matches(UNSTAMPED).count() != 1 {
        return Err(format!(
            "{}: expected exactly one unstamped `{UNSTAMPED}`",
            path.display()
        ));
    }
    let stamped = text.replacen(UNSTAMPED, &format!("\"evidence_head\": \"{head}\""), 1);
    serde_json::from_str::<Value>(&stamped).map_err(|e| format!("{}: {e}", path.display()))?;
    std::fs::write(path, stamped).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn run() -> Result<(), String> {
    let dirs = crate::positional_args();
    if dirs.is_empty() {
        return Err("usage: stamp-evidence DIR [DIR...]".into());
    }
    if !crate::capture::git(&["status", "--porcelain"])
        .trim()
        .is_empty()
    {
        return Err(
            "stamp-evidence requires a clean tree (git status --porcelain must be empty)".into(),
        );
    }
    let head = crate::capture::git(&["rev-parse", "HEAD"])
        .trim()
        .to_string();
    let root = crate::capture::repo_root();

    for d in &dirs {
        let dir = PathBuf::from(d);
        let receipt_path = dir.join("receipt.json");
        if !receipt_path.exists() {
            return Err(format!("{}: no receipt.json", dir.display()));
        }
        let rel = rel_to_repo(&root, &receipt_path)?;
        if !git_ok(&["ls-files", "--error-unmatch", &rel]) {
            return Err(format!("{rel}: not tracked in HEAD"));
        }
        let receipt: Value = serde_json::from_str(
            &std::fs::read_to_string(&receipt_path).map_err(|e| e.to_string())?,
        )
        .map_err(|e| format!("{}: {e}", receipt_path.display()))?;
        let code_head = receipt["code_head"]
            .as_str()
            .ok_or_else(|| format!("{rel}: no code_head"))?
            .to_string();
        if !git_ok(&["merge-base", "--is-ancestor", &code_head, "HEAD"]) {
            return Err(format!(
                "{rel}: code_head {code_head} is not an ancestor of HEAD"
            ));
        }
        let diff = crate::capture::git(&["diff", "--name-only", &code_head, "HEAD"]);
        let offending: Vec<&str> = diff
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter(|l| !l.starts_with("benchmark/results/"))
            .collect();
        if !offending.is_empty() {
            return Err(format!(
                "{rel}: code changed after code_head {code_head}: {}",
                offending.join(", ")
            ));
        }
        stamp_json(&receipt_path, &head)?;
        println!("stamped {rel}: evidence_head={head}");
        let perf_path = dir.join("perf.json");
        if perf_path.exists() {
            let perf: Value = serde_json::from_str(
                &std::fs::read_to_string(&perf_path).map_err(|e| e.to_string())?,
            )
            .map_err(|e| format!("{}: {e}", perf_path.display()))?;
            if perf.get("code_head").is_some() {
                stamp_json(&perf_path, &head)?;
                println!("stamped {}: evidence_head={head}", perf_path.display());
            }
        }
    }
    Ok(())
}
