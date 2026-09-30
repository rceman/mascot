//! `diff-images <dirA> <dirB> [--out FILE]` — recursive PNG comparison by
//! relative path: identical (byte-equal) / pixel-identical / changed (with
//! changed-pixel count and bounding box) / missing / extra.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use mascot_render_win32::image::RgbaImage;

fn collect_pngs(dir: &Path) -> Result<BTreeSet<String>, String> {
    let mut out = BTreeSet::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).map_err(|e| format!("{}: {e}", d.display()))? {
            let e = e.map_err(|e| e.to_string())?;
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "png") {
                let rel = p
                    .strip_prefix(dir)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.insert(rel);
            }
        }
    }
    Ok(out)
}

/// changed-pixel count + bounding box (x, y, w, h); equal pixels => (0, ...).
fn pixel_diff(a: &RgbaImage, b: &RgbaImage) -> (u64, u32, u32, u32, u32) {
    let (mut n, mut minx, mut miny, mut maxx, mut maxy) = (0u64, u32::MAX, u32::MAX, 0u32, 0u32);
    for y in 0..a.height {
        for x in 0..a.width {
            let i = ((y * a.width + x) * 4) as usize;
            if a.data[i..i + 4] != b.data[i..i + 4] {
                n += 1;
                minx = minx.min(x);
                miny = miny.min(y);
                maxx = maxx.max(x);
                maxy = maxy.max(y);
            }
        }
    }
    if n == 0 {
        return (0, 0, 0, 0, 0);
    }
    (n, minx, miny, maxx - minx + 1, maxy - miny + 1)
}

pub fn run() -> Result<(), String> {
    let pos = crate::positional_args();
    if pos.len() < 2 {
        return Err("usage: diff-images DIR_A DIR_B [--out FILE]".into());
    }
    let (dir_a, dir_b) = (PathBuf::from(&pos[0]), PathBuf::from(&pos[1]));
    let set_a = collect_pngs(&dir_a)?;
    let set_b = collect_pngs(&dir_b)?;

    let mut lines = vec![format!(
        "diff-images {} vs {}",
        dir_a.display(),
        dir_b.display()
    )];
    let (mut identical, mut total) = (0usize, 0usize);
    let missing: Vec<&String> = set_a.difference(&set_b).collect();
    let extra: Vec<&String> = set_b.difference(&set_a).collect();

    for rel in set_a.intersection(&set_b) {
        total += 1;
        let (pa, pb) = (dir_a.join(rel), dir_b.join(rel));
        let (ba, bb) = (
            std::fs::read(&pa).map_err(|e| format!("{}: {e}", pa.display()))?,
            std::fs::read(&pb).map_err(|e| format!("{}: {e}", pb.display()))?,
        );
        if ba == bb {
            identical += 1;
            continue;
        }
        let (ia, ib) = (
            RgbaImage::load(&pa).map_err(|e| format!("{rel}: {e}"))?,
            RgbaImage::load(&pb).map_err(|e| format!("{rel}: {e}"))?,
        );
        if ia.width != ib.width || ia.height != ib.height {
            lines.push(format!(
                "changed {rel}: size {}x{} vs {}x{}",
                ia.width, ia.height, ib.width, ib.height
            ));
            continue;
        }
        let (n, x, y, w, h) = pixel_diff(&ia, &ib);
        if n == 0 {
            lines.push(format!("pixel-identical {rel}"));
        } else {
            lines.push(format!("changed {rel}: {n} px, bbox {x},{y},{w},{h}"));
        }
    }
    for m in &missing {
        lines.push(format!("missing {m} (A only)"));
    }
    for e in &extra {
        lines.push(format!("extra {e} (B only)"));
    }
    lines.push(format!("identical: {identical}/{total}"));
    let report = lines.join("\n");
    println!("{report}");
    if let Some(out) = crate::get_arg("--out") {
        std::fs::write(&out, &report).map_err(|e| format!("{out}: {e}"))?;
        println!("wrote {out}");
    }
    Ok(())
}
