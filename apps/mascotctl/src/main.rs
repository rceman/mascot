//! mascotctl — character pipeline tooling.
//!
//!   mascotctl profile check <profile.json>
//!   mascotctl profile guide <profile.json> [--out DIR]
//!   mascotctl character board --profile <p> --style <img> --out DIR [--brief TEXT]
//!   mascotctl character synth <spec.json> --out DIR
//!   mascotctl character validate <source.json>... [--profile P] [--report-dir DIR]

use mascot_character as mc;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn fatal(msg: &str) -> ! {
    eprintln!("error: {msg}");
    std::process::exit(2);
}

fn usage() -> ! {
    eprintln!(
        "usage:
  mascotctl profile check <profile.json>
  mascotctl profile guide <profile.json> [--out DIR]
  mascotctl character board --profile <p> --style <img> --out DIR [--brief TEXT]
  mascotctl character synth <spec.json> --out DIR
  mascotctl character validate <source.json>... [--profile P] [--report-dir DIR]"
    );
    std::process::exit(2);
}

fn opt_value(args: &mut impl Iterator<Item = String>, flag: &str) -> String {
    args.next()
        .unwrap_or_else(|| fatal(&format!("{flag} needs a value")))
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(kind) = args.next() else { usage() };
    match kind.as_str() {
        "profile" => match args.next().as_deref() {
            Some("check") => {
                let Some(path) = args.next() else { usage() };
                profile_check(Path::new(&path))
            }
            Some("guide") => {
                let Some(path) = args.next() else { usage() };
                let mut out = None;
                while let Some(a) = args.next() {
                    match a.as_str() {
                        "--out" => out = Some(opt_value(&mut args, "--out").into()),
                        other => fatal(&format!("unknown argument {other}")),
                    }
                }
                profile_guide(Path::new(&path), out)
            }
            _ => usage(),
        },
        "character" => match args.next().as_deref() {
            Some("board") => char_board(args.collect()),
            Some("synth") => {
                let Some(path) = args.next() else { usage() };
                let mut out = None;
                while let Some(a) = args.next() {
                    match a.as_str() {
                        "--out" => out = Some(opt_value(&mut args, "--out").into()),
                        other => fatal(&format!("unknown argument {other}")),
                    }
                }
                char_synth(Path::new(&path), out.unwrap_or_else(|| usage()))
            }
            Some("validate") => char_validate(args.collect()),
            _ => usage(),
        },
        _ => usage(),
    }
}

fn load_profile(path: &Path) -> (mc::Profile, Vec<u8>) {
    mc::Profile::load(path).unwrap_or_else(|e| fatal(&e))
}

fn profile_check(path: &Path) -> ExitCode {
    let (p, _bytes) = load_profile(path);
    let errs = mc::check_profile(&p);
    println!("profile {}", p.id);
    println!("  roles:       {}", p.roles.len());
    println!("  landmarks:   {}", p.landmarks.len());
    println!("  pose rules:  {}", p.pose.len());
    println!("  proportions: {}", p.proportions.len());
    println!("  clearance:   {}", p.clearance.len());
    println!("  chains:      {}", p.chains.len());
    println!("  sockets:     {}", p.sockets.len());
    if errs.is_empty() {
        println!("check: OK");
        ExitCode::from(0)
    } else {
        for e in &errs {
            println!("error: {e}");
        }
        println!("check: {} error(s)", errs.len());
        ExitCode::from(1)
    }
}

fn profile_guide(path: &Path, out: Option<PathBuf>) -> ExitCode {
    let (p, bytes) = load_profile(path);
    let errs = mc::check_profile(&p);
    if !errs.is_empty() {
        for e in &errs {
            println!("error: {e}");
        }
        return ExitCode::from(1);
    }
    let dir = out.unwrap_or_else(|| path.parent().unwrap_or(Path::new(".")).join("guide"));
    match mc::write_guide(&p, &bytes, &dir) {
        Ok(rec) => {
            for (name, o) in &rec.outputs {
                println!(
                    "{name}: {}x{} sha256 {}…",
                    o.size[0],
                    o.size[1],
                    &o.sha256[..16]
                );
            }
            println!("guide OK -> {}", dir.display());
            ExitCode::from(0)
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(1)
        }
    }
}

fn char_board(args: Vec<String>) -> ExitCode {
    let (mut profile, mut style, mut out, mut brief): (
        Option<PathBuf>,
        Option<PathBuf>,
        Option<PathBuf>,
        Option<String>,
    ) = (None, None, None, None);
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--profile" => profile = Some(opt_value(&mut it, "--profile").into()),
            "--style" => style = Some(opt_value(&mut it, "--style").into()),
            "--out" => out = Some(opt_value(&mut it, "--out").into()),
            "--brief" => brief = Some(opt_value(&mut it, "--brief")),
            other => fatal(&format!("unknown argument {other}")),
        }
    }
    let (Some(pp), Some(sp), Some(od)) = (profile, style, out) else {
        usage()
    };
    let (p, bytes) = load_profile(&pp);
    match mc::make_board(&p, &pp, &bytes, &sp, &od, brief.as_deref()) {
        Ok((_img, rec)) => {
            println!(
                "board {}x{} sha256 {}",
                rec.board.size[0],
                rec.board.size[1],
                &rec.board.sha256[..16]
            );
            println!("board OK -> {}", od.display());
            ExitCode::from(0)
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(1)
        }
    }
}

fn char_synth(spec_path: &Path, out: PathBuf) -> ExitCode {
    let spec = mc::synth::SynthSpec::load(spec_path).unwrap_or_else(|e| fatal(&e));
    match mc::synth::render_synth(&spec, &out) {
        Ok((_img, _m)) => {
            println!("synth {} -> {}", spec.character_id, out.display());
            ExitCode::from(0)
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(1)
        }
    }
}

fn char_validate(args: Vec<String>) -> ExitCode {
    let mut sources = Vec::new();
    let (mut profile, mut report_dir): (Option<PathBuf>, Option<PathBuf>) = (None, None);
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--profile" => profile = Some(opt_value(&mut it, "--profile").into()),
            "--report-dir" => report_dir = Some(opt_value(&mut it, "--report-dir").into()),
            other if other.starts_with('-') => fatal(&format!("unknown argument {other}")),
            other => sources.push(PathBuf::from(other)),
        }
    }
    if sources.is_empty() {
        usage();
    }
    let repo_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut any_fail = false;
    for src_path in &sources {
        let src = match mc::CharacterSource::load(src_path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error: {e}");
                return ExitCode::from(2);
            }
        };
        let pp = match &profile {
            Some(p) => p.clone(),
            None => {
                let dir = src_path.parent().unwrap_or(Path::new("."));
                match mc::find_profile_up(dir, &src.profile.id) {
                    Some(p) => p,
                    None => {
                        eprintln!(
                            "error: cannot locate profile '{}' from {}",
                            src.profile.id,
                            src_path.display()
                        );
                        return ExitCode::from(2);
                    }
                }
            }
        };
        let (p, pbytes) = match mc::Profile::load_checked(&pp) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("error: {e}");
                return ExitCode::from(2);
            }
        };
        let psha = mc::hash::sha256_hex(&pbytes);
        let report = mc::validate_source(src_path, &src, &p, &psha, &repo_root);
        println!(
            "{} {}: {} fail, {} warn",
            report.result, report.character_id, report.counts.fail, report.counts.warn
        );
        for f in &report.findings {
            if f.severity != mc::Severity::Info {
                println!(
                    "  {} {} {}: {}",
                    f.severity.tag(),
                    f.code,
                    f.subject,
                    f.message
                );
            }
        }
        if report.counts.fail > 0 {
            any_fail = true;
        }
        if let Some(rd) = &report_dir {
            std::fs::create_dir_all(rd).expect("report dir");
            let name = src_path
                .parent()
                .and_then(|p| p.file_name())
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let j = serde_json::to_string_pretty(&report).expect("report json");
            std::fs::write(rd.join(format!("{name}.json")), j + "\n").expect("write report");
        }
    }
    if any_fail {
        ExitCode::from(1)
    } else {
        ExitCode::from(0)
    }
}
