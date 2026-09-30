//! `perf-ab --baseline-exe EXE --baseline-head SHA --out DIR [--rounds 2]
//! [--runs 5] [--allow-dirty]` — interleaved A/B perf comparison: A1, B1,
//! A2, B2, ... where A is the baseline exe's `perf` run and B is this exe's.
//! Writes the canonical `<dir>/perf.json` plus the raw JSONs under
//! `<dir>/logs/perf-ab/`.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

const METRICS: [&str; 6] = [
    "first_show_ms",
    "open_warm_ms_p50",
    "submit_response_ms_p50",
    "theme_switch_ms_p50",
    "idle_cpu_pct",
    "growth_private_last_mean",
];

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    match v.len() % 2 {
        1 => v[v.len() / 2],
        _ => (v[v.len() / 2 - 1] + v[v.len() / 2]) / 2.0,
    }
}

fn run_perf(exe: &Path, out: &Path, runs: usize) -> Result<(), String> {
    // cwd = the exe's dir so the child's git calls resolve inside its own
    // tree (a `git archive` export needs a .git marker there, or is a
    // worktree); inheriting our cwd would attribute OUR head to the baseline.
    let cwd = exe.parent().unwrap_or(exe);
    let st = Command::new(exe)
        .current_dir(cwd)
        .arg("perf")
        .arg("--out")
        .arg(out)
        .arg("--runs")
        .arg(runs.to_string())
        .status()
        .map_err(|e| format!("spawn {}: {e}", exe.display()))?;
    if !st.success() {
        return Err(format!(
            "{} perf -> {}: exit {st}",
            exe.display(),
            out.display()
        ));
    }
    Ok(())
}

fn run_metric_values(doc: &Value, name: &str) -> Vec<f64> {
    doc["raw_runs"]
        .as_array()
        .map(|runs| {
            runs.iter()
                .filter_map(|r| r["metrics"].get(name).and_then(|v| v.as_f64()))
                .collect()
        })
        .unwrap_or_default()
}

fn run_metric_max(docs: &[&Value], name: &str) -> f64 {
    docs.iter()
        .flat_map(|d| run_metric_values(d, name))
        .fold(0.0, f64::max)
}

pub fn run() -> Result<(), String> {
    let baseline_exe =
        PathBuf::from(crate::get_arg("--baseline-exe").ok_or("perf-ab needs --baseline-exe EXE")?);
    let baseline_head =
        crate::get_arg("--baseline-head").ok_or("perf-ab needs --baseline-head SHA")?;
    let out_dir = PathBuf::from(crate::get_arg("--out").ok_or("perf-ab needs --out DIR")?);
    let rounds: usize = crate::get_arg("--rounds")
        .and_then(|v| v.parse().ok())
        .unwrap_or(2);
    let runs: usize = crate::get_arg("--runs")
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);
    let allow_dirty = crate::has_flag("--allow-dirty");

    if !baseline_exe.exists() {
        return Err(format!(
            "baseline exe not found: {}",
            baseline_exe.display()
        ));
    }
    let current_exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let ab_dir = out_dir.join("logs").join("perf-ab");
    std::fs::create_dir_all(&ab_dir).map_err(|e| format!("{}: {e}", ab_dir.display()))?;

    // strict alternation A1, B1, A2, B2, ...
    let mut order = Vec::new();
    let mut a_paths = Vec::new();
    let mut b_paths = Vec::new();
    for r in 1..=rounds {
        let a = ab_dir.join(format!("A{r}.json"));
        run_perf(&baseline_exe, &a, runs)?;
        a_paths.push(a);
        order.push(format!("A{r}"));
        let b = ab_dir.join(format!("B{r}.json"));
        run_perf(&current_exe, &b, runs)?;
        b_paths.push(b);
        order.push(format!("B{r}"));
    }

    // load + validate
    let load = |p: &Path| -> Result<Value, String> {
        serde_json::from_str(&std::fs::read_to_string(p).map_err(|e| e.to_string())?)
            .map_err(|e| format!("{}: {e}", p.display()))
    };
    let mut a_docs = Vec::new();
    for p in &a_paths {
        let d = load(p)?;
        let head = d["env"]["head"].as_str().unwrap_or_default();
        if head != baseline_head && !head.starts_with(&baseline_head) {
            return Err(format!(
                "{}: env.head {head} does not match --baseline-head {baseline_head}",
                p.display()
            ));
        }
        a_docs.push(d);
    }
    let mut b_docs = Vec::new();
    let mut b_head = String::new();
    let mut b_dirty = false;
    for p in &b_paths {
        let d = load(p)?;
        let head = d["env"]["head"].as_str().unwrap_or_default().to_string();
        if b_head.is_empty() {
            b_head = head;
        } else if head != b_head {
            return Err(format!(
                "{}: B head {head} differs from {b_head}",
                p.display()
            ));
        }
        if d["env"]["dirty"].as_bool() == Some(true) {
            b_dirty = true;
        }
        if b_dirty && !allow_dirty {
            return Err(format!(
                "{}: B run on a dirty tree (use --allow-dirty for smoke tests)",
                p.display()
            ));
        }
        b_docs.push(d);
    }

    let a_refs: Vec<&Value> = a_docs.iter().collect();
    let b_refs: Vec<&Value> = b_docs.iter().collect();

    let mut metrics = serde_json::Map::new();
    for name in METRICS {
        let a_vals: Vec<f64> = a_refs
            .iter()
            .flat_map(|d| run_metric_values(d, name))
            .collect();
        let b_vals: Vec<f64> = b_refs
            .iter()
            .flat_map(|d| run_metric_values(d, name))
            .collect();
        let a_med = median(a_vals.clone());
        let b_med = median(b_vals.clone());
        metrics.insert(
            name.to_string(),
            json!({
                "a_values": a_vals,
                "b_values": b_vals,
                "a_median": a_med,
                "b_median": b_med,
                "delta": b_med - a_med,
                "delta_pct": if a_med != 0.0 { 100.0 * (b_med - a_med) / a_med } else { f64::NAN },
            }),
        );
    }

    let b_env = &b_docs[0]["env"];
    let env = json!({
        "cpu": b_env["cpu"],
        "gpu": b_env["gpu"],
        "os_build": b_env["os_build"],
        "device": b_env["device"],
        "profile": b_env["profile"],
        "scale": b_env["scale"],
        "font": b_env["font"],
    });
    let raw: Vec<String> = a_paths
        .iter()
        .zip(b_paths.iter())
        .flat_map(|(a, b)| [a, b])
        .map(|p| {
            p.strip_prefix(&out_dir)
                .unwrap_or(p)
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();

    let plateau = |docs: &[&Value]| -> bool {
        docs.iter().all(|d| {
            d["raw_runs"].as_array().is_some_and(|rs| {
                rs.iter()
                    .all(|r| r["metrics"]["growth_plateau_after_1000"].as_bool() == Some(true))
            })
        })
    };

    let doc = json!({
        "kind": "alternating-ab",
        "canonical": true,
        "code_head": b_head,
        "code_dirty": b_dirty,
        "evidence_head": Value::Null,
        "env": env,
        "baseline": {
            "head": a_docs[0]["env"]["head"],
            "exe": baseline_exe.to_string_lossy(),
        },
        "order": order,
        "runs_per_set": runs,
        "raw": raw,
        "metrics": metrics,
        "zero_idle": {
            "a_idle_presents_10s_max": run_metric_max(&a_refs, "idle_presents_10s"),
            "b_idle_presents_10s_max": run_metric_max(&b_refs, "idle_presents_10s"),
            "b_focused_quiet_presents_10s_max": run_metric_max(&b_refs, "focused_quiet_presents_10s"),
            "b_focused_blink_presents_max": run_metric_max(&b_refs, "focused_blink_presents"),
        },
        "growth_plateau_all": plateau(&a_refs) && plateau(&b_refs),
    });
    let out = out_dir.join("perf.json");
    std::fs::write(&out, serde_json::to_string_pretty(&doc).unwrap())
        .map_err(|e| format!("{}: {e}", out.display()))?;
    println!(
        "perf-ab: {} rounds x {} runs -> {}",
        rounds,
        runs,
        out.display()
    );
    Ok(())
}
