//! Deterministic pose-stress suite for rig v0.2 (spec section 14-16).
//!
//! Renders with the WARP device so output is reproducible, then writes:
//! - poses/<name>.png          individual stress renders (runtime outline + shadow)
//! - contact_sheet.png         all poses, labelled
//! - joint_guards/*.png        guard sweep sheets (guard circles drawn)
//! - rest/                     canonical vs runtime rest, alpha + colour diffs
//! - pivots.png                bone / pivot / attachment-bounds debug view
//! - stress_report.json        per-pose defect metrics, guard results, rest stats
//!
//! Usage: mascot-rig-stress [--rig DIR] [--source PNG] [--out DIR] [--hardware]

use mascot_animation::{Affine, Pose, Rig, RootPlacement};
use mascot_render_win32::image::RgbaImage;
use mascot_render_win32::{DeviceKind, Layers, RenderOptions, Renderer, View};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::time::Instant;

const EVIDENCE_SCALE: f32 = 0.5;
const EVIDENCE_SIZE: [u32; 2] = [740, 740];
const BG: [f32; 4] = [0.93, 0.95, 0.98, 1.0];

struct PoseSpec {
    name: String,
    group: &'static str,
    bones: Vec<(&'static str, f32)>,
    mirror: bool,
}

fn pose(name: impl Into<String>, group: &'static str, bones: &[(&'static str, f32)], mirror: bool) -> PoseSpec {
    PoseSpec { name: name.into(), group, bones: bones.to_vec(), mirror }
}

fn poses() -> Vec<PoseSpec> {
    let mut v = vec![pose("rest", "rest", &[], false)];
    for a in [-10.0, -5.0, 5.0, 10.0] {
        v.push(pose(format!("head_{a:+}"), "head", &[("head", a)], false));
    }
    for e in ["ear_near", "ear_far"] {
        for a in [-12.0, -6.0, 6.0, 12.0] {
            v.push(pose(format!("{e}_{a:+}"), "ear", &[(e, a)], false));
        }
    }
    for (b, label) in [
        ("arm_near_upper", "arm_near"),
        ("arm_far_upper", "arm_far"),
        ("leg_near_upper", "leg_near"),
        ("leg_far_upper", "leg_far"),
    ] {
        for a in [-15.0, -10.0, 10.0, 15.0] {
            v.push(pose(format!("{label}_{a:+}"), if label.starts_with("arm") { "arm" } else { "leg" }, &[(b, a)], false));
        }
    }
    for a in [-20.0, -10.0, 10.0, 20.0] {
        v.push(pose(format!("tail_{a:+}"), "tail", &[("tail", a)], false));
    }
    let greeting: &[(&str, f32)] = &[("head", -6.0), ("ear_near", 8.0), ("ear_far", -6.0), ("arm_near_upper", -12.0), ("tail", 12.0)];
    let posture: &[(&str, f32)] = &[("hips", 2.0), ("body", -2.0), ("chest", -2.0), ("neck", 3.0), ("head", -5.0), ("laptop_screen", 4.0)];
    let curious: &[(&str, f32)] = &[("head", 10.0), ("ear_near", -10.0), ("ear_far", 10.0), ("tail", 20.0), ("leg_far_upper", 10.0)];
    let extremes_a: &[(&str, f32)] = &[
        ("head", -10.0), ("ear_near", 12.0), ("ear_far", 12.0), ("arm_near_upper", -15.0), ("arm_far_upper", -15.0),
        ("leg_near_upper", 15.0), ("leg_far_upper", 15.0), ("tail", -20.0),
    ];
    let extremes_b: &[(&str, f32)] = &[
        ("head", 10.0), ("ear_near", -12.0), ("ear_far", -12.0), ("arm_near_upper", 15.0), ("arm_far_upper", 15.0),
        ("leg_near_upper", -15.0), ("leg_far_upper", -15.0), ("tail", 20.0),
    ];
    let stretch: &[(&str, f32)] = &[
        ("chest", -2.0), ("head", 8.0), ("arm_near_upper", -15.0), ("arm_far_upper", -12.0), ("leg_near_upper", -10.0),
        ("leg_far_upper", -8.0), ("tail", -15.0), ("ear_near", -8.0), ("ear_far", -8.0),
    ];
    v.push(pose("mixed_greeting", "mixed", greeting, false));
    v.push(pose("mixed_posture", "mixed", posture, false));
    v.push(pose("mixed_curious", "mixed", curious, false));
    v.push(pose("mixed_stretch", "mixed", stretch, false));
    v.push(pose("mixed_extremes_a", "mixed", extremes_a, false));
    v.push(pose("mixed_extremes_b", "mixed", extremes_b, false));
    v.push(pose("mirror_rest", "mirror", &[], true));
    v.push(pose("mirror_greeting", "mirror", greeting, true));
    v.push(pose("mirror_extremes_a", "mirror", extremes_a, true));
    v
}

fn build_pose(rig: &Rig, bones: &[(&str, f32)]) -> Pose {
    let mut p = rig.skeleton.rest_pose();
    for (b, deg) in bones {
        let i = rig.skeleton.find(b).unwrap_or_else(|| panic!("unknown bone {b}"));
        p.bones[i].rotation_deg += deg;
    }
    p
}

fn evidence_view(rig: &Rig) -> View {
    let c = rig.canvas_size();
    View {
        scale: EVIDENCE_SCALE,
        offset: [
            (EVIDENCE_SIZE[0] as f32 - c[0] as f32 * EVIDENCE_SCALE) / 2.0,
            (EVIDENCE_SIZE[1] as f32 - c[1] as f32 * EVIDENCE_SCALE) / 2.0,
        ],
    }
}

fn render(r: &mut Renderer, rig: &Rig, pose: &Pose, mirror: bool, size: [u32; 2], view: View, opts: &RenderOptions) -> (Layers, Vec<Affine>) {
    let world = rig.skeleton.world(pose, RootPlacement { mirror });
    let items = rig.draw_list(&world, pose);
    let layers = r.render_layers(size, rig, &items, &world, view, opts).expect("render");
    (layers, world)
}

/// Pixels with neither fill nor line coverage that are enclosed by the silhouette:
/// these would show the black outline through the body (holes / black wedges).
fn enclosed_gaps(l: &Layers) -> (usize, Vec<[usize; 4]>) {
    let (w, h) = (l.color.width as usize, l.color.height as usize);
    let gap: Vec<bool> = l.color.data.chunks_exact(4).zip(&l.fill_alpha).map(|(c, &f)| c[3] < 128 && f < 128).collect();
    let mut reached = vec![false; w * h];
    let mut stack: Vec<usize> = Vec::new();
    for x in 0..w {
        stack.push(x);
        stack.push((h - 1) * w + x);
    }
    for y in 0..h {
        stack.push(y * w);
        stack.push(y * w + w - 1);
    }
    while let Some(i) = stack.pop() {
        if reached[i] || !gap[i] {
            continue;
        }
        reached[i] = true;
        let (x, y) = (i % w, i / w);
        if x > 0 { stack.push(i - 1); }
        if x + 1 < w { stack.push(i + 1); }
        if y > 0 { stack.push(i - w); }
        if y + 1 < h { stack.push(i + w); }
    }
    // connected components of enclosed gaps -> bounding boxes
    let mut seen = vec![false; w * h];
    let mut boxes = Vec::new();
    let mut total = 0;
    for s in 0..w * h {
        if !gap[s] || reached[s] || seen[s] {
            continue;
        }
        let mut bb = [usize::MAX, usize::MAX, 0, 0];
        let mut n = 0;
        stack.push(s);
        while let Some(i) = stack.pop() {
            if seen[i] || !gap[i] || reached[i] {
                continue;
            }
            seen[i] = true;
            n += 1;
            let (x, y) = (i % w, i / w);
            bb = [bb[0].min(x), bb[1].min(y), bb[2].max(x), bb[3].max(y)];
            if x > 0 { stack.push(i - 1); }
            if x + 1 < w { stack.push(i + 1); }
            if y > 0 { stack.push(i - w); }
            if y + 1 < h { stack.push(i + w); }
        }
        total += n;
        if n >= 4 {
            boxes.push(bb);
        }
    }
    (total, boxes)
}

fn guard_coverage(l: &Layers, center: [f32; 2], radius: f32) -> f32 {
    let (w, h) = (l.color.width as i32, l.color.height as i32);
    let (mut n, mut ok) = (0u32, 0u32);
    let r = radius.ceil() as i32;
    for dy in -r..=r {
        for dx in -r..=r {
            if (dx * dx + dy * dy) as f32 > radius * radius {
                continue;
            }
            let (x, y) = (center[0].round() as i32 + dx, center[1].round() as i32 + dy);
            if x < 0 || y < 0 || x >= w || y >= h {
                continue;
            }
            n += 1;
            let i = (y * w + x) as usize;
            if l.color.data[i * 4 + 3] >= 128 {
                ok += 1;
            }
        }
    }
    if n == 0 { 0.0 } else { ok as f32 / n as f32 }
}

fn draw_circle(img: &mut RgbaImage, c: [f32; 2], r: f32, col: [u8; 4]) {
    let steps = (r * 8.0) as usize + 16;
    for t in 0..steps {
        let a = std::f32::consts::TAU * t as f32 / steps as f32;
        for k in 0..2 {
            let rr = r + k as f32;
            let (x, y) = ((c[0] + rr * a.cos()).round() as i64, (c[1] + rr * a.sin()).round() as i64);
            if x >= 0 && y >= 0 && (x as u32) < img.width && (y as u32) < img.height {
                let i = ((y as u32 * img.width + x as u32) * 4) as usize;
                img.data[i..i + 4].copy_from_slice(&col);
            }
        }
    }
}

fn mark_boxes(img: &mut RgbaImage, boxes: &[[usize; 4]]) {
    for b in boxes {
        let (x0, y0, x1, y1) = (b[0].saturating_sub(4) as u32, b[1].saturating_sub(4) as u32, (b[2] + 4) as u32, (b[3] + 4) as u32);
        img.fill_rect(x0, y0, x1.saturating_sub(x0), 2, [255, 0, 80, 255]);
        img.fill_rect(x0, y1, x1.saturating_sub(x0), 2, [255, 0, 80, 255]);
        img.fill_rect(x0, y0, 2, y1.saturating_sub(y0), [255, 0, 80, 255]);
        img.fill_rect(x1, y0, 2, y1.saturating_sub(y0), [255, 0, 80, 255]);
    }
}

/// Renders several poses (separated by `::`) at canvas resolution, cropped, side by side.
fn probe(rig_dir: &PathBuf, kind: DeviceKind, path: &PathBuf, crop: &[u32], spec: &[String]) {
    let rig = Rig::load(rig_dir).unwrap();
    let mut r = Renderer::new(kind).unwrap();
    r.load_rig(&rig).unwrap();
    let canvas = rig.canvas_size();
    let pad = 200.0;
    let size = [canvas[0] + 400, canvas[1] + 400];
    let view = View { scale: 1.0, offset: [pad, pad] };
    let opts = RenderOptions { background: Some(BG), ..Default::default() };
    let mut imgs = Vec::new();
    for group in spec.split(|s| s == "::") {
        let mut p = rig.skeleton.rest_pose();
        let mut mirror = false;
        for s in group {
            if s == "mirror" {
                mirror = true;
                continue;
            }
            let (b, d) = s.split_once('=').expect("bone=deg");
            p.bones[rig.skeleton.find(b).expect("bone")].rotation_deg = d.parse().unwrap();
        }
        let (l, _) = render(&mut r, &rig, &p, mirror, size, view, &opts);
        imgs.push((group.join(" "), l.final_image.crop(crop[0] + 200, crop[1] + 200, crop[2], crop[3])));
    }
    let cells: Vec<(String, &RgbaImage)> = imgs.iter().map(|(n, i)| (n.clone(), i)).collect();
    r.compose_sheet(&cells, cells.len(), [crop[2], crop[3]], 20, [1.0, 1.0, 1.0]).unwrap().save(path).unwrap();
}

fn main() {
    let mut rig_dir = PathBuf::from("assets/mascot/rig-v0.2");
    let mut source = PathBuf::from("assets/mascot.png");
    let mut out = PathBuf::from("benchmark/results/windows/animation-rig-v0.2");
    let mut kind = DeviceKind::Warp;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--rig" => rig_dir = args.next().expect("--rig DIR").into(),
            "--source" => source = args.next().expect("--source PNG").into(),
            "--out" => out = args.next().expect("--out DIR").into(),
            "--hardware" => kind = DeviceKind::Hardware,
            // --probe OUT.png X,Y,W,H [mirror] bone=deg... : full-resolution crop for inspection
            "--probe" => {
                let path = PathBuf::from(args.next().expect("OUT.png"));
                let crop: Vec<u32> = args.next().expect("X,Y,W,H").split(',').map(|v| v.parse().unwrap()).collect();
                let rest: Vec<String> = args.by_ref().collect();
                return probe(&rig_dir, kind, &path, &crop, &rest);
            }
            other => panic!("unknown argument {other}"),
        }
    }
    let t0 = Instant::now();
    let rig = Rig::load(&rig_dir).unwrap_or_else(|e| panic!("load rig: {e}"));
    let mut r = Renderer::new(kind).expect("renderer");
    r.load_rig(&rig).expect("load sprites");
    println!("device: {:?}; rig: {} bones, {} attachments", r.kind, rig.skeleton.bones.len(), rig.slots.len());
    let view = evidence_view(&rig);
    let opts = RenderOptions { background: Some(BG), shadow: true, ..Default::default() };

    // --- poses ---------------------------------------------------------------
    let specs = poses();
    let mut pose_reports = Vec::new();
    let mut sheet_imgs: Vec<(String, RgbaImage)> = Vec::new();
    let mut rest_gap = 0usize;
    for spec in &specs {
        let p = build_pose(&rig, &spec.bones);
        for (b, d) in &spec.bones {
            let i = rig.skeleton.find(b).unwrap();
            let [lo, hi] = rig.skeleton.bones[i].safe_rotation_deg;
            assert!(*d >= lo - 1e-3 && *d <= hi + 1e-3, "pose {} drives {b} to {d} outside safe range [{lo}, {hi}]", spec.name);
        }
        let (layers, _) = render(&mut r, &rig, &p, spec.mirror, EVIDENCE_SIZE, view, &opts);
        let (gap_px, boxes) = enclosed_gaps(&layers);
        if spec.name == "rest" {
            rest_gap = gap_px;
        }
        layers.final_image.save(&out.join("poses").join(format!("{}.png", spec.name))).unwrap();
        let mut marked = layers.final_image.clone();
        mark_boxes(&mut marked, &boxes);
        sheet_imgs.push((spec.name.clone(), marked));
        pose_reports.push(json!({
            "name": spec.name, "group": spec.group, "mirror": spec.mirror,
            "bones_deg": spec.bones.iter().map(|(b, d)| json!([b, d])).collect::<Vec<_>>(),
            "enclosed_gap_px": gap_px, "enclosed_gap_boxes": boxes,
        }));
        println!("pose {:<22} enclosed_gap_px {gap_px}", spec.name);
    }
    let cells: Vec<(String, &RgbaImage)> = sheet_imgs.iter().map(|(n, i)| (n.clone(), i)).collect();
    r.compose_sheet(&cells, 7, [370, 370], 22, [1.0, 1.0, 1.0]).unwrap().save(&out.join("contact_sheet.png")).unwrap();

    // --- joint guards --------------------------------------------------------
    let mut guard_reports = Vec::new();
    let mut guards_ok = true;
    for g in &rig.file.joint_guards {
        let gb = rig.skeleton.find(&g.bone).unwrap();
        for (drive, range) in &g.drives {
            let db = rig.skeleton.find(drive).unwrap();
            let mut samples = Vec::new();
            let mut cells_owned = Vec::new();
            let steps = ((range[1] - range[0]) / 2.5).round() as usize;
            let mut min_cov: f32 = 1.0;
            for s in 0..=steps {
                let deg = range[0] + (range[1] - range[0]) * s as f32 / steps as f32;
                let mut p = rig.skeleton.rest_pose();
                p.bones[db].rotation_deg = deg;
                let (layers, world) = render(&mut r, &rig, &p, false, EVIDENCE_SIZE, view, &opts);
                let c = view.affine().apply(world[gb].apply(g.center));
                let rad = g.radius * view.scale;
                let cov = guard_coverage(&layers, c, rad);
                min_cov = min_cov.min(cov);
                samples.push(json!({"deg": deg, "coverage": cov}));
                if s % 2 == 0 {
                    let mut img = layers.final_image.crop(
                        (c[0] - 110.0).max(0.0) as u32,
                        (c[1] - 110.0).max(0.0) as u32,
                        220,
                        220,
                    );
                    let lc = [c[0] - (c[0] - 110.0).max(0.0), c[1] - (c[1] - 110.0).max(0.0)];
                    draw_circle(&mut img, lc, rad, if cov >= g.min_coverage { [0, 170, 60, 255] } else { [255, 0, 80, 255] });
                    cells_owned.push((format!("{drive} {deg:+.1} cov {:.3}", cov), img));
                }
            }
            let pass = min_cov >= g.min_coverage;
            guards_ok &= pass;
            let cells: Vec<(String, &RgbaImage)> = cells_owned.iter().map(|(n, i)| (n.clone(), i)).collect();
            r.compose_sheet(&cells, cells.len(), [220, 220], 20, [1.0, 1.0, 1.0])
                .unwrap()
                .save(&out.join("joint_guards").join(format!("{}.png", g.id)))
                .unwrap();
            println!("guard {:<16} {drive:<16} min coverage {min_cov:.4} (need {}) {}", g.id, g.min_coverage, if pass { "PASS" } else { "FAIL" });
            guard_reports.push(json!({"id": g.id, "bone": g.bone, "drive": drive, "range": range,
                "min_coverage": min_cov, "required": g.min_coverage, "pass": pass, "samples": samples}));
        }
    }

    // --- rest comparison (full canvas resolution) ---------------------------
    let canvas = rig.canvas_size();
    let full = View { scale: 1.0, offset: [0.0, 0.0] };
    let rest_pose = rig.skeleton.rest_pose();
    let (rest_layers, _) = render(&mut r, &rig, &rest_pose, false, canvas, full, &RenderOptions::default());
    let canon = RgbaImage::load(&source).expect("canonical source");
    let runtime = &rest_layers.final_image;
    let rest_dir = out.join("rest");
    canon.save(&rest_dir.join("canonical.png")).unwrap();
    runtime.save(&rest_dir.join("runtime_rest.png")).unwrap();
    let mut alpha_diff = RgbaImage::new(canvas[0], canvas[1]);
    let mut color_diff = RgbaImage::new(canvas[0], canvas[1]);
    let (cw, rw) = (canon.over([255, 255, 255]), runtime.over([255, 255, 255]));
    let (mut sum, mut gt32, mut gt96, mut a64) = (0f64, 0usize, 0usize, 0usize);
    for i in 0..(canvas[0] * canvas[1]) as usize {
        let ad = (canon.data[i * 4 + 3] as i32 - runtime.data[i * 4 + 3] as i32).unsigned_abs() as u8;
        let cd = (0..3).map(|c| (cw.data[i * 4 + c] as i32 - rw.data[i * 4 + c] as i32).unsigned_abs()).max().unwrap() as u8;
        sum += cd as f64;
        gt32 += (cd > 32) as usize;
        gt96 += (cd > 96) as usize;
        a64 += (ad > 64) as usize;
        let av = ad.saturating_mul(3);
        alpha_diff.data[i * 4..i * 4 + 4].copy_from_slice(&[av, av, av, 255]);
        let cv = cd.saturating_mul(2);
        color_diff.data[i * 4..i * 4 + 4].copy_from_slice(&[cv, (cv / 3), 255 - cv, 255]);
    }
    alpha_diff.save(&rest_dir.join("alpha_diff_x3.png")).unwrap();
    color_diff.save(&rest_dir.join("rgb_diff_x2_heat.png")).unwrap();
    let side = r
        .compose_sheet(
            &[("canonical assets/mascot.png".into(), &cw), ("runtime rest (D2D, runtime outline)".into(), &rw), ("RGB diff x2".into(), &color_diff)],
            3,
            [600, 600],
            24,
            [1.0, 1.0, 1.0],
        )
        .unwrap();
    side.save(&rest_dir.join("rest_comparison.png")).unwrap();
    let n = (canvas[0] * canvas[1]) as f64;
    let rest_stats = json!({"rgb_absdiff_mean_over_white": sum / n, "rgb_diff_gt32_px": gt32, "rgb_diff_gt96_px": gt96,
        "alpha_diff_gt64_px": a64, "canvas_px": n});
    println!("rest: {rest_stats}");

    // --- pivot / bone debug view ---------------------------------------------
    let piv_opts = RenderOptions {
        background: Some([1.0, 1.0, 1.0, 1.0]),
        bones: true,
        pivots: true,
        bounds: true,
        labels: true,
        ..Default::default()
    };
    let pv = View::fit(canvas, [1100, 1100], 20.0);
    let (piv, _) = render(&mut r, &rig, &rest_pose, false, [1100, 1100], pv, &piv_opts);
    piv.final_image.save(&out.join("pivots.png")).unwrap();
    let mut mp = rig.skeleton.rest_pose();
    for (b, d) in [("head", 10.0), ("arm_near_upper", -15.0), ("leg_near_upper", 15.0), ("tail", 20.0), ("ear_near", 12.0)] {
        mp.bones[rig.skeleton.find(b).unwrap()].rotation_deg = d;
    }
    let (pivm, _) = render(&mut r, &rig, &mp, true, [1100, 1100], pv, &piv_opts);
    pivm.final_image.save(&out.join("pivots_mirrored_posed.png")).unwrap();

    let max_gap_delta = pose_reports.iter().map(|p| p["enclosed_gap_px"].as_u64().unwrap() as i64 - rest_gap as i64).max().unwrap_or(0);
    let report = json!({
        "tool": "mascot-rig-stress", "device": format!("{:?}", r.kind),
        "rig": rig_dir.display().to_string(), "rig_version": rig.file.version,
        "evidence_view": {"scale": EVIDENCE_SCALE, "size": EVIDENCE_SIZE},
        "outline_radius_canvas_px": rig.outline_radius(),
        "poses": pose_reports, "rest_enclosed_gap_px": rest_gap, "max_enclosed_gap_delta_vs_rest": max_gap_delta,
        "joint_guards": guard_reports, "joint_guards_pass": guards_ok,
        "rest_comparison": rest_stats,
        "elapsed_s": t0.elapsed().as_secs_f32(),
    });
    std::fs::write(out.join("stress_report.json"), serde_json::to_string_pretty(&report).unwrap() + "\n").unwrap();
    let _: Value = report;
    println!("guards pass: {guards_ok}; max enclosed-gap delta vs rest: {max_gap_delta}px; {:.1}s", t0.elapsed().as_secs_f32());
}
