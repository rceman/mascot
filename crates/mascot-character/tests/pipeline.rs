//! Pipeline tests: profile, guide determinism, board, manifest/validation,
//! fixtures and the generic-code audit.

use mascot_character as mc;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn profile_path() -> PathBuf {
    repo().join("assets/character-templates/biped-3q-v1/profile.json")
}

fn load_profile() -> (mc::Profile, Vec<u8>) {
    mc::Profile::load(&profile_path()).unwrap()
}

// ---------- profile ----------

#[test]
fn profile_loads_and_checks() {
    let (p, _) = load_profile();
    assert_eq!(p.id, "biped-3q-v1");
    assert!(mc::check_profile(&p).is_empty());
}

fn variant(mut j: serde_json::Value, f: impl FnOnce(&mut serde_json::Value)) -> serde_json::Value {
    f(&mut j);
    j
}

#[test]
fn profile_negatives() {
    let base: serde_json::Value =
        serde_json::from_slice(&std::fs::read(profile_path()).unwrap()).unwrap();

    // unknown JSON field -> deny_unknown_fields
    let j = variant(base.clone(), |j| {
        j["bogus"] = serde_json::json!(1);
    });
    assert!(serde_json::from_value::<mc::Profile>(j).is_err());

    // cycle
    let j = variant(base.clone(), |j| {
        j["roles"]["spine"] = serde_json::json!("neck");
    });
    let p: mc::Profile = serde_json::from_value(j).unwrap();
    assert!(mc::check_profile(&p).iter().any(|e| e.contains("cycle")));

    // unknown parent
    let j = variant(base.clone(), |j| {
        j["roles"]["pelvis"] = serde_json::json!("nope");
    });
    let p: mc::Profile = serde_json::from_value(j).unwrap();
    assert!(
        mc::check_profile(&p)
            .iter()
            .any(|e| e.contains("unknown parent"))
    );

    // role without landmark
    let j = variant(base.clone(), |j| {
        j["landmarks"].as_object_mut().unwrap().remove("pelvis");
    });
    let p: mc::Profile = serde_json::from_value(j).unwrap();
    assert!(mc::check_profile(&p).iter().any(|e| e.contains("pelvis")));

    // unknown landmark ref in constraint
    let j = variant(base.clone(), |j| {
        j["pose"].as_array_mut().unwrap()[0]["angle"][0] = serde_json::json!("zzz");
    });
    let p: mc::Profile = serde_json::from_value(j).unwrap();
    assert!(mc::check_profile(&p).iter().any(|e| e.contains("zzz")));

    // unknown landmark ref in chain
    let j = variant(base.clone(), |j| {
        j["chains"]["trunk"].as_array_mut().unwrap()[0] = serde_json::json!("zzz");
    });
    let p: mc::Profile = serde_json::from_value(j).unwrap();
    assert!(mc::check_profile(&p).iter().any(|e| e.contains("zzz")));

    // unknown landmark ref in guide shape
    let j = variant(base.clone(), |j| {
        j["guide"]["layers"][0]["shapes"][0]["capsule"][0] = serde_json::json!("zzz");
    });
    let p: mc::Profile = serde_json::from_value(j).unwrap();
    assert!(mc::check_profile(&p).iter().any(|e| e.contains("zzz")));

    // unknown class
    let j = variant(base.clone(), |j| {
        j["landmarks"]["pelvis"][0] = serde_json::json!("Q");
    });
    assert!(serde_json::from_value::<mc::Profile>(j).is_err());

    // missing required params per constraint kind
    for (i, field) in [
        (0usize, "tol"),
        (9, "range"),
        (13, "min_gap"),
        (23, "line"),
        (23, "tol"),
    ] {
        let j = variant(base.clone(), |j| {
            let pose = &mut j["pose"].as_array_mut().unwrap()[i];
            pose.as_object_mut().unwrap().remove(field);
        });
        let p: mc::Profile = serde_json::from_value(j).unwrap();
        let errs = mc::check_profile(&p);
        assert!(
            errs.iter()
                .any(|e| e.contains("required") || e.contains("must be")),
            "pose[{i}] without {field}: expected param error, got {errs:?}"
        );
    }
    // no_cross with unknown chain
    let j = variant(base.clone(), |j| {
        j["pose"].as_array_mut().unwrap()[25]["no_cross"][0] = serde_json::json!("zzz");
    });
    let p: mc::Profile = serde_json::from_value(j).unwrap();
    assert!(
        mc::check_profile(&p)
            .iter()
            .any(|e| e.contains("unknown chain"))
    );
}

// ---------- hash ----------

#[test]
fn sha256_known_answer() {
    assert_eq!(
        mc::hash::sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

// ---------- guide ----------

#[test]
fn guide_deterministic_and_committed() {
    let (p, bytes) = load_profile();
    let a = mc::render_generation(&p, 1024).unwrap();
    let b = mc::render_generation(&p, 1024).unwrap();
    assert_eq!(a.data, b.data);
    let c = mc::render_annotated(&p, 2048).unwrap();
    let d = mc::render_annotated(&p, 2048).unwrap();
    assert_eq!(c.data, d.data);

    // committed guide files match a fresh render
    let tmp = std::env::temp_dir().join("mc_guide_test");
    let _ = std::fs::remove_dir_all(&tmp);
    let rec = mc::write_guide(&p, &bytes, &tmp).unwrap();
    let guide_dir = profile_path().parent().unwrap().join("guide");
    for name in ["dummy-generation.png", "dummy-annotated.png", "guide.json"] {
        let committed = std::fs::read(guide_dir.join(name))
            .unwrap_or_else(|e| panic!("committed {name} missing: {e}"));
        let fresh = std::fs::read(tmp.join(name)).unwrap();
        assert_eq!(committed, fresh, "{name} is stale vs fresh render");
    }
    assert_eq!(rec.format, "mascot-guide-receipt/1");
}

// ---------- board ----------

#[test]
fn board_fit_math() {
    // wide
    let wide = mc::png_io::Image::new(2000, 500, [10, 20, 30, 255]);
    let (f, s) = mc::fit_uniform(&wide, 1024, 1024);
    assert_eq!((f.w, f.h), (1024, 256));
    assert!((s - 0.512).abs() < 1e-9);
    // tall
    let tall = mc::png_io::Image::new(500, 2000, [10, 20, 30, 255]);
    let (f, s) = mc::fit_uniform(&tall, 1024, 1024);
    assert_eq!((f.w, f.h), (256, 1024));
    assert!((s - 0.512).abs() < 1e-9);
    // square upscale
    let sq = mc::png_io::Image::new(100, 100, [10, 20, 30, 255]);
    let (f, s) = mc::fit_uniform(&sq, 1024, 1024);
    assert_eq!((f.w, f.h), (1024, 1024));
    assert!(s > 1.0);
    assert_eq!(f.px(500, 500), [10, 20, 30, 255]);
}

#[test]
fn board_deterministic_receipt() {
    let (p, bytes) = load_profile();
    let style = repo().join("assets/mascot.png");
    let t1 = std::env::temp_dir().join("mc_board_a");
    let t2 = std::env::temp_dir().join("mc_board_b");
    let (_i1, r1) = mc::make_board(&p, &profile_path(), &bytes, &style, &t1, Some("x")).unwrap();
    let (_i2, r2) = mc::make_board(&p, &profile_path(), &bytes, &style, &t2, Some("x")).unwrap();
    assert_eq!(r1.board.sha256, r2.board.sha256);
    assert_eq!(r1.guide.sha256, r2.guide.sha256);
    assert!(r1.prompt.is_some());
    // receipt board hash matches the written png
    assert_eq!(
        r1.board.sha256,
        mc::hash::sha256_file(&t1.join("board.png")).unwrap()
    );
    // receipt guide hash = sha of the raw guide png bytes used
    assert_eq!(r1.layout.size, [2048, 1024]);
    assert_eq!(r1.layout.style_rect, [1024, 0, 1024, 1024]);
    // mascot.png is 1254x1254 -> fits at 1024
    assert!((r1.layout.scale - 1024.0 / 1254.0).abs() < 1e-9);
}

// ---------- dummy self-validation ----------

#[test]
fn dummy_self_validates_clean() {
    let (p, pbytes) = load_profile();
    let img = mc::render_generation(&p, 1024).unwrap();
    let tmp = std::env::temp_dir().join("mc_dummy");
    std::fs::create_dir_all(&tmp).unwrap();
    let ipath = tmp.join("source.png");
    mc::png_io::save_png(&ipath, &img).unwrap();
    let sha = mc::hash::sha256_file(&ipath).unwrap();
    // manifest annotated with the profile's own landmarks
    let landmarks: BTreeMap<String, [f64; 2]> = p
        .landmarks
        .iter()
        .map(|(k, l)| (k.clone(), [l.x, l.y]))
        .collect();
    let manifest_json = serde_json::json!({
        "format": "mascot-character-source/1",
        "character_id": "dummy",
        "profile": {"id": p.id, "revision": p.revision},
        "image": {"path": "source.png", "sha256": sha},
        "provenance": {"kind": "synthetic", "created": "2026-01-01", "tool": "test"},
        "generation": null,
        "annotation": {
            "status": "approved", "owner": "agent", "image_sha256": sha,
            "landmarks": landmarks, "appendages": [], "accepted_deviations": []
        }
    });
    let spath = tmp.join("source.json");
    std::fs::write(
        &spath,
        serde_json::to_string_pretty(&manifest_json).unwrap(),
    )
    .unwrap();
    let src = mc::CharacterSource::load(&spath).unwrap();
    let report = mc::validate_source(&spath, &src, &p, &mc::hash::sha256_hex(&pbytes), &repo());
    let interesting: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.severity != mc::Severity::Info)
        .collect();
    assert!(
        interesting.is_empty(),
        "dummy should be clean: {interesting:?}"
    );
}

// ---------- fixtures ----------

fn fixture_dirs() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> =
        std::fs::read_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/biped-3q-v1"))
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_dir())
            .collect();
    v.sort();
    v
}

#[test]
fn fixtures_stale_and_validate() {
    let (p, pbytes) = load_profile();
    let psha = mc::hash::sha256_hex(&pbytes);
    for dir in fixture_dirs() {
        let name = dir.file_name().unwrap().to_string_lossy().to_string();
        let spec = mc::synth::SynthSpec::load(&dir.join("spec.json")).unwrap();
        // re-render into a temp dir; committed files must be byte-identical
        let tmp = std::env::temp_dir().join(format!("mc_fx_{name}"));
        let _ = std::fs::remove_dir_all(&tmp);
        mc::synth::render_synth(&spec, &p, &tmp).unwrap();
        for f in ["source.png", "source.json"] {
            assert_eq!(
                std::fs::read(tmp.join(f)).unwrap(),
                std::fs::read(dir.join(f)).unwrap(),
                "{name}/{f} stale vs spec"
            );
        }
        let spath = dir.join("source.json");
        let src = mc::CharacterSource::load(&spath).unwrap();
        let report = mc::validate_source(&spath, &src, &p, &psha, &repo());
        let expect: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.join("expect.json")).unwrap()).unwrap();
        assert_eq!(
            report.result,
            expect["result"].as_str().unwrap(),
            "{name} result"
        );
        let got: Vec<Vec<String>> = report
            .findings
            .iter()
            .map(|f| {
                vec![
                    f.severity.tag().to_string(),
                    f.code.clone(),
                    f.subject.clone(),
                ]
            })
            .collect();
        let want: Vec<Vec<String>> = expect["findings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| {
                a.as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap().to_string())
                    .collect()
            })
            .collect();
        assert_eq!(got, want, "{name} findings differ from expect.json");
    }
}

// ---------- synth inheritance ----------

#[test]
fn synth_inherits_dummy_landmarks() {
    let (p, _) = load_profile();
    let spec = mc::synth::SynthSpec::load(
        &repo().join("crates/mascot-character/fixtures/biped-3q-v1/robot/spec.json"),
    )
    .unwrap();
    let resolved = spec.resolved(&p).unwrap();
    // every profile landmark the spec doesn't exclude resolves to dummy coords
    for (id, l) in &p.landmarks {
        if spec.annotate_exclude.contains(id) {
            continue;
        }
        assert_eq!(resolved[id], [l.x, l.y], "{id}");
    }
}

#[test]
fn synth_rejects_bad_profile_and_override() {
    let (p, _) = load_profile();
    let spec_json = std::fs::read_to_string(
        repo().join("crates/mascot-character/fixtures/biped-3q-v1/robot/spec.json"),
    )
    .unwrap();
    // revision mismatch
    let bad_rev = spec_json.replace("\"revision\": 1", "\"revision\": 99");
    let tmp = std::env::temp_dir().join("mc_spec_badrev.json");
    std::fs::write(&tmp, &bad_rev).unwrap();
    let spec2 = mc::synth::SynthSpec::load(&tmp).unwrap();
    assert!(spec2.resolved(&p).is_err(), "revision mismatch must error");
    // unknown override id
    let bad_lm = spec_json.replace(
        "\"landmarks\": {}",
        "\"landmarks\": {\"nope.bogus\": [0.5, 0.5]}",
    );
    std::fs::write(&tmp, &bad_lm).unwrap();
    let spec3 = mc::synth::SynthSpec::load(&tmp).unwrap();
    assert!(spec3.resolved(&p).is_err(), "unknown override must error");
}

// ---------- audit: no character-specific tokens in generic code ----------

#[test]
fn audit_generic_code() {
    const FORBIDDEN: &[&str] = &[
        "otter",
        "beaver",
        "ferret",
        "robot",
        "mascotlike",
        "fur",
        "muzzle",
        "species",
        "tail",
        "ear",
        "ears",
        "antenna",
        "visor",
        "laptop",
        "paw",
    ];
    let (p, _) = load_profile();
    let mut ids: Vec<String> = Vec::new();
    ids.extend(p.landmarks.keys().cloned());
    ids.extend(p.roles.keys().cloned());
    ids.extend(p.sockets.keys().cloned());
    ids.extend(p.chains.keys().cloned());
    ids.extend(p.appendage_types.iter().cloned());
    let mut roots = vec![
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        repo().join("apps/mascotctl/src"),
    ];
    let mut files = Vec::new();
    while let Some(d) = roots.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let e = e.unwrap();
            let pth = e.path();
            if pth.is_dir() {
                roots.push(pth);
            } else if pth.extension().is_some_and(|x| x == "rs") {
                files.push(pth);
            }
        }
    }
    let tok_re = regex_split();
    let mut bad = Vec::new();
    for f in &files {
        let text = std::fs::read_to_string(f).unwrap();
        for (ln, line) in text.lines().enumerate() {
            for tok in tok_re.split(line).filter(|s| !s.is_empty()) {
                if FORBIDDEN.contains(&tok.to_lowercase().as_str()) {
                    bad.push(format!(
                        "{}:{} forbidden token '{tok}'",
                        f.display(),
                        ln + 1
                    ));
                }
            }
            for lit in string_literals(line) {
                if ids.contains(&lit) {
                    bad.push(format!(
                        "{}:{} profile id literal \"{lit}\"",
                        f.display(),
                        ln + 1
                    ));
                }
            }
        }
    }
    assert!(bad.is_empty(), "audit violations:\n{}", bad.join("\n"));
}

fn regex_split() -> SimpleSplit {
    SimpleSplit
}

struct SimpleSplit;
impl SimpleSplit {
    fn split<'a>(&self, s: &'a str) -> impl Iterator<Item = &'a str> {
        s.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
    }
}

fn string_literals(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_lit = false;
    let mut esc = false;
    for c in line.chars() {
        if in_lit {
            if esc {
                esc = false;
                cur.push(c);
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_lit = false;
                out.push(std::mem::take(&mut cur));
            } else {
                cur.push(c);
            }
        } else if c == '"' {
            in_lit = true;
        }
    }
    out
}
