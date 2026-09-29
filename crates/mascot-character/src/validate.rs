//! Character validation: manifest + canvas + landmarks + envelope + pose +
//! proportions + clearance checks against a RigProfile.

use crate::png_io::Image;
use crate::profile::{LmClass, Profile};
use crate::report::{Finding, ImageRef, ProfileRef, Severity, ValidationReport};
use crate::source::{AnnotationStatus, CharacterSource};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

type LmMap = HashMap<String, [f64; 2]>;
type Emit<'a> = dyn FnMut(Severity, &str, &str, String) + 'a;

fn is_required(c: LmClass) -> bool {
    matches!(c, LmClass::R | LmClass::J | LmClass::T)
}

/// Should a constraint that references `id` run? Returns true to run; emits
/// INFO when skipping on a missing required landmark, silent for optional.
fn lm_ok(id: &str, lm: &LmMap, p: &Profile, emit: &mut Emit, ctx: &str) -> bool {
    if lm.contains_key(id) {
        return true;
    }
    let cls = p.landmarks.get(id).map(|l| l.class);
    match cls {
        Some(c) if is_required(c) => emit(
            Severity::Info,
            "check.skipped",
            ctx,
            format!("skipped: required landmark '{id}' not annotated"),
        ),
        Some(_) => {} // optional F/S absent -> silent skip
        None => {}    // unknown id -> already a landmark.unknown FAIL
    }
    false
}

fn wrap_deg(d: f64) -> f64 {
    let mut a = d % 360.0;
    if a > 180.0 {
        a -= 360.0;
    }
    if a < -180.0 {
        a += 360.0;
    }
    a
}

fn seg_intersect(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> bool {
    // proper intersection (open segments)
    let s = |u: [f64; 2], v: [f64; 2], w: [f64; 2]| {
        (v[0] - u[0]) * (w[1] - u[1]) - (v[1] - u[1]) * (w[0] - u[0])
    };
    let d1 = s(c, d, a);
    let d2 = s(c, d, b);
    let d3 = s(a, b, c);
    let d4 = s(a, b, d);
    ((d1 > 0.0) != (d2 > 0.0)) && ((d3 > 0.0) != (d4 > 0.0))
}

/// Evaluate pose constraints on a landmark set; report through `emit`.
pub fn eval_pose_constraints(p: &Profile, lm: &LmMap, emit: &mut Emit) {
    let dummy: LmMap = p
        .landmarks
        .iter()
        .map(|(k, l)| (k.clone(), [l.x, l.y]))
        .collect();
    for c in &p.pose {
        if let Some(ids) = &c.angle {
            let (a, b) = (&ids[0], &ids[1]);
            if !(lm_ok(a, lm, p, emit, &format!("{a}->{b}"))
                && lm_ok(b, lm, p, emit, &format!("{a}->{b}")))
            {
                continue;
            }
            let pa = lm[a];
            let pb = lm[b];
            let ang = (pb[0] - pa[0]).atan2(pb[1] - pa[1]).to_degrees();
            let d0 = dummy[a];
            let d1 = dummy[b];
            let ref_ = (d1[0] - d0[0]).atan2(d1[1] - d0[1]).to_degrees();
            let diff = wrap_deg(ang - ref_).abs();
            if diff > c.tol.unwrap() {
                emit(
                    Severity::Fail,
                    "pose.angle",
                    &format!("{a}->{b}"),
                    format!(
                        "angle {ang:.1}° deviates {diff:.1}° from dummy {ref_:.1}° (tol {}°)",
                        c.tol.unwrap()
                    ),
                );
            }
        }
        if let Some(ids) = &c.bend {
            let (a, b, cc) = (&ids[0], &ids[1], &ids[2]);
            if !(lm_ok(a, lm, p, emit, &format!("{a},{b},{cc}"))
                && lm_ok(b, lm, p, emit, &format!("{a},{b},{cc}"))
                && lm_ok(cc, lm, p, emit, &format!("{a},{b},{cc}")))
            {
                continue;
            }
            let flex = |p1: [f64; 2], p2: [f64; 2], p3: [f64; 2]| -> f64 {
                let (u, v) = (
                    [p2[0] - p1[0], p2[1] - p1[1]],
                    [p3[0] - p2[0], p3[1] - p2[1]],
                );
                let dot = (u[0] * v[0] + u[1] * v[1])
                    / ((u[0].powi(2) + u[1].powi(2)).sqrt()
                        * (v[0].powi(2) + v[1].powi(2)).sqrt().max(1e-12));
                dot.clamp(-1.0, 1.0).acos().to_degrees()
            };
            let sign = |p1: [f64; 2], p2: [f64; 2], p3: [f64; 2]| -> f64 {
                (p2[0] - p1[0]) * (p3[1] - p2[1]) - (p2[1] - p1[1]) * (p3[0] - p2[0])
            };
            let f = flex(lm[a], lm[b], lm[cc]);
            let range = c.range.unwrap();
            if f < range[0] || f > range[1] {
                emit(
                    Severity::Fail,
                    "pose.bend",
                    &format!("{a},{b},{cc}"),
                    format!("flexion {f:.1}° outside [{:.0},{:.0}]°", range[0], range[1]),
                );
            }
            let f0 = flex(dummy[a], dummy[b], dummy[cc]);
            let (s, s0) = (
                sign(lm[a], lm[b], lm[cc]),
                sign(dummy[a], dummy[b], dummy[cc]),
            );
            if f0 >= 3.0 && f >= 3.0 && (s * s0 < 0.0) {
                emit(
                    Severity::Fail,
                    "pose.bend_reversed",
                    &format!("{a},{b},{cc}"),
                    format!("bend sign reversed vs dummy (flexion {f:.1}°)"),
                );
            }
        }
        if let Some(ids) = &c.order_x {
            let (a, b) = (&ids[0], &ids[1]);
            if !(lm_ok(a, lm, p, emit, &format!("{a}<{b}"))
                && lm_ok(b, lm, p, emit, &format!("{a}<{b}")))
            {
                continue;
            }
            let gap = lm[b][0] - lm[a][0];
            if gap < c.min_gap.unwrap() {
                emit(
                    Severity::Fail,
                    "pose.order_x",
                    &format!("{a}<{b}"),
                    format!("x gap {gap:.3} < {:.3}", c.min_gap.unwrap()),
                );
            }
        }
        if let Some(pts) = &c.on_ground {
            let gy = if c.line.as_deref() == Some("far") {
                p.ground.far
            } else {
                p.ground.near
            };
            for id in pts {
                if !lm_ok(id, lm, p, emit, id) {
                    continue;
                }
                let d = (lm[id][1] - gy).abs();
                if d > c.tol.unwrap() {
                    emit(
                        Severity::Fail,
                        "pose.on_ground",
                        id,
                        format!(
                            "y {:.3} off {} ground line {:.3} by {:.3}",
                            lm[id][1],
                            c.line.as_deref().unwrap_or("near"),
                            gy,
                            d
                        ),
                    );
                }
            }
        }
        if let Some(chs) = &c.no_cross {
            let (a, b) = (&chs[0], &chs[1]);
            let missing_req: Vec<&String> = p.chains[a]
                .iter()
                .chain(p.chains[b].iter())
                .filter(|id| {
                    !lm.contains_key(*id)
                        && p.landmarks.get(*id).is_some_and(|l| is_required(l.class))
                })
                .collect();
            if !missing_req.is_empty() {
                emit(
                    Severity::Info,
                    "check.skipped",
                    &format!("{a}/{b}"),
                    format!(
                        "skipped: required landmarks not annotated: {}",
                        missing_req
                            .iter()
                            .map(|s| s.as_str())
                            .collect::<Vec<_>>()
                            .join(",")
                    ),
                );
                continue;
            }
            let ca: Vec<[f64; 2]> = p.chains[a]
                .iter()
                .filter_map(|id| lm.get(id).copied())
                .collect();
            let cb: Vec<[f64; 2]> = p.chains[b]
                .iter()
                .filter_map(|id| lm.get(id).copied())
                .collect();
            if ca.len() != p.chains[a].len() || cb.len() != p.chains[b].len() {
                continue; // optional landmarks absent -> silent skip
            }
            let shared: HashSet<[u64; 2]> = p.chains[a]
                .iter()
                .filter(|id| p.chains[b].contains(id))
                .map(|id| [lm[id][0].to_bits(), lm[id][1].to_bits()])
                .collect();
            let mut cross = false;
            for i in 0..ca.len().saturating_sub(1) {
                for j in 0..cb.len().saturating_sub(1) {
                    let (a1, a2, b1, b2) = (ca[i], ca[i + 1], cb[j], cb[j + 1]);
                    if [a1, a2, b1, b2]
                        .iter()
                        .any(|pt| shared.contains(&[pt[0].to_bits(), pt[1].to_bits()]))
                    {
                        continue;
                    }
                    if seg_intersect(a1, a2, b1, b2) {
                        cross = true;
                    }
                }
            }
            if cross {
                emit(
                    Severity::Fail,
                    "topology.crossing",
                    &format!("{a}/{b}"),
                    format!("chains '{a}' and '{b}' intersect"),
                );
            }
        }
    }
}

/// Evaluate proportions on a landmark set; report through `emit`.
pub fn eval_proportions(p: &Profile, lm: &LmMap, emit: &mut Emit) {
    if !(lm.contains_key(&p.height_ref[0]) && lm.contains_key(&p.height_ref[1])) {
        return;
    }
    let h = lm[&p.height_ref[0]][1] - lm[&p.height_ref[1]][1];
    if h <= 1e-6 {
        return;
    }
    for pr in &p.proportions {
        let v = if let Some(dy) = &pr.dy {
            if !(lm_ok(&dy[0], lm, p, emit, &pr.name) && lm_ok(&dy[1], lm, p, emit, &pr.name)) {
                continue;
            }
            (lm[&dy[1]][1] - lm[&dy[0]][1]).abs() / h
        } else if let Some(ch) = &pr.chain {
            let chain = &p.chains[ch];
            let missing_req: Vec<&String> = chain
                .iter()
                .filter(|id| {
                    !lm.contains_key(*id)
                        && p.landmarks.get(*id).is_some_and(|l| is_required(l.class))
                })
                .collect();
            if !missing_req.is_empty() {
                emit(
                    Severity::Info,
                    "check.skipped",
                    &format!("prop.{}", pr.name),
                    format!(
                        "skipped: required landmarks not annotated: {}",
                        missing_req
                            .iter()
                            .map(|s| s.as_str())
                            .collect::<Vec<_>>()
                            .join(",")
                    ),
                );
                continue;
            }
            if !chain.iter().all(|id| lm.contains_key(id)) {
                continue;
            }
            chain
                .windows(2)
                .map(|w| {
                    let (a, b) = (lm[&w[0]], lm[&w[1]]);
                    ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt()
                })
                .sum::<f64>()
                / h
        } else {
            continue;
        };
        if v < pr.range[0] || v > pr.range[1] {
            emit(
                Severity::Fail,
                "proportion.range",
                &pr.name,
                format!(
                    "{v:.3} outside [{:.2},{:.2}] of height",
                    pr.range[0], pr.range[1]
                ),
            );
        }
    }
}

/// Foreground mask per the profile canvas rules. Returns (mask, bg_error).
/// `bg_error` Some means `canvas.background` FAIL.
pub fn foreground(img: &Image, p: &Profile) -> (Vec<bool>, Option<String>, [u8; 3]) {
    let (w, h) = (img.w as usize, img.h as usize);
    let n = w * h;
    let fdef = &p.canvas.foreground;
    let block = |bx: usize, by: usize| -> (bool, [u8; 4], bool /*all_transparent*/) {
        let mut minc = [u8::MAX; 4];
        let mut maxc = [u8::MIN; 4];
        let mut any_alpha = false;
        for yy in 0..8 {
            for xx in 0..8 {
                let c = img.px((bx * 8 + xx) as u32, (by * 8 + yy) as u32);
                for ch in 0..4 {
                    minc[ch] = minc[ch].min(c[ch]);
                    maxc[ch] = maxc[ch].max(c[ch]);
                }
                any_alpha |= c[3] > 0;
            }
        }
        let uniform = (0..4).all(|ch| (maxc[ch] - minc[ch]) <= fdef.bg_uniform);
        (uniform, maxc, !any_alpha)
    };
    let corners = [
        (0, 0),
        (w / 8 - 1, 0),
        (0, h / 8 - 1),
        (w / 8 - 1, h / 8 - 1),
    ];
    let blocks: Vec<_> = corners.iter().map(|&(bx, by)| block(bx, by)).collect();
    let has_alpha = img.data.chunks(4).any(|c| c[3] != 255);
    if has_alpha && blocks.iter().all(|b| b.2) {
        // transparent background
        let mask = img
            .data
            .chunks(4)
            .map(|c| c[3] >= fdef.alpha_min)
            .collect::<Vec<bool>>();
        debug_assert_eq!(mask.len(), n);
        return (mask, None, [0, 0, 0]);
    }
    if !blocks.iter().all(|b| b.0) {
        return (
            vec![false; n],
            Some("corner blocks not uniform".into()),
            [0, 0, 0],
        );
    }
    let bg = [
        blocks.iter().map(|b| b.1[0] as u32).sum::<u32>() / 4,
        blocks.iter().map(|b| b.1[1] as u32).sum::<u32>() / 4,
        blocks.iter().map(|b| b.1[2] as u32).sum::<u32>() / 4,
    ];
    let mask = img
        .data
        .chunks(4)
        .map(|c| {
            c[3] >= fdef.alpha_min
                && c.iter()
                    .take(3)
                    .zip(bg.iter())
                    .any(|(pc, bc)| (*pc as u32).abs_diff(*bc) > fdef.bg_delta as u32)
        })
        .collect();
    (mask, None, [bg[0] as u8, bg[1] as u8, bg[2] as u8])
}

/// Components of an fg mask (8-connected).
fn components(mask: &[bool], w: usize, h: usize) -> Vec<usize> {
    let mut seen = vec![false; mask.len()];
    let mut sizes = Vec::new();
    for start in 0..mask.len() {
        if !mask[start] || seen[start] {
            continue;
        }
        let mut stack = vec![start];
        seen[start] = true;
        let mut n = 0usize;
        while let Some(i) = stack.pop() {
            n += 1;
            let (x, y) = (i % w, i / w);
            for dy in -1i64..=1 {
                for dx in -1i64..=1 {
                    let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                    if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 || (dx == 0 && dy == 0)
                    {
                        continue;
                    }
                    let j = ny as usize * w + nx as usize;
                    if mask[j] && !seen[j] {
                        seen[j] = true;
                        stack.push(j);
                    }
                }
            }
        }
        sizes.push(n);
    }
    sizes
}

/// Full validation of one CharacterSource manifest + its image.
pub fn validate_source(
    src_path: &Path,
    src: &CharacterSource,
    profile: &Profile,
    profile_sha: &str,
    repo_root: &Path,
) -> ValidationReport {
    let mut findings: Vec<Finding> = Vec::new();
    let mut emit = |sev: Severity, code: &str, subj: &str, msg: String| {
        findings.push(Finding::new(sev, code, subj, msg));
    };

    // ---- manifest ----
    let src_dir = src_path.parent().unwrap_or(Path::new("."));
    let img_path = src_dir.join(&src.image.path);
    let img_bytes = std::fs::read(&img_path);
    let img_hash = img_bytes
        .as_ref()
        .map(|b| crate::hash::sha256_hex(b))
        .unwrap_or_else(|_| "<unreadable>".into());
    match &img_bytes {
        Ok(_) => {
            if img_hash != src.image.sha256 {
                emit(
                    Severity::Fail,
                    "manifest.image_hash",
                    &src.image.path,
                    format!("image sha256 {img_hash} != manifest {}", src.image.sha256),
                );
            }
        }
        Err(e) => emit(
            Severity::Fail,
            "manifest.image_hash",
            &src.image.path,
            format!("cannot read image: {e}"),
        ),
    }
    if src.annotation.image_sha256 != img_hash {
        emit(
            Severity::Fail,
            "manifest.annotation_stale",
            "annotation.image_sha256",
            format!(
                "annotation hash {} != image hash {img_hash}",
                src.annotation.image_sha256
            ),
        );
    }
    if src.profile.id != profile.id {
        emit(
            Severity::Fail,
            "profile.id_mismatch",
            &src.profile.id,
            format!("source profile '{}' != '{}'", src.profile.id, profile.id),
        );
    }
    if src.profile.revision != profile.revision {
        emit(
            Severity::Warn,
            "profile.revision_mismatch",
            &src.profile.id,
            format!(
                "source revision {} != {}",
                src.profile.revision, profile.revision
            ),
        );
    }
    if !src.provenance.created_valid() {
        emit(
            Severity::Fail,
            "manifest.bad_field",
            "provenance.created",
            format!(
                "'{}' is not a valid YYYY-MM-DD date",
                src.provenance.created
            ),
        );
    }
    if src.provenance.kind.is_generated() {
        match &src.generation {
            None => emit(
                Severity::Fail,
                "manifest.missing_field",
                "generation",
                "kind=generated requires generation{board,prompt}".into(),
            ),
            Some(g) => {
                let ph = crate::hash::sha256_hex(g.prompt.text.as_bytes());
                if ph != g.prompt.sha256 {
                    emit(
                        Severity::Fail,
                        "manifest.prompt_hash",
                        "generation.prompt",
                        format!("prompt sha256 {ph} != declared {}", g.prompt.sha256),
                    );
                }
            }
        }
    }
    if src.annotation.status == AnnotationStatus::Draft {
        emit(
            Severity::Warn,
            "annotation.draft",
            "annotation",
            "annotation status is draft".into(),
        );
    }
    for id in src.annotation.landmarks.keys() {
        if !profile.landmarks.contains_key(id) {
            emit(
                Severity::Fail,
                "landmark.unknown",
                id,
                "landmark not in profile".into(),
            );
        }
    }
    for ad in &src.annotation.accepted_deviations {
        if !profile.landmarks.contains_key(&ad.landmark) {
            emit(
                Severity::Fail,
                "landmark.unknown",
                &ad.landmark,
                "accepted_deviation landmark not in profile".into(),
            );
        }
    }
    let mut sockets_used: HashSet<String> = HashSet::new();
    for ap in &src.annotation.appendages {
        let (ty, sock) = if ap.len() == 2 {
            (ap[0].as_str(), ap[1].as_str())
        } else {
            emit(
                Severity::Fail,
                "appendage.socket",
                &format!("{ap:?}"),
                "appendage must be [type, socket]".into(),
            );
            continue;
        };
        let bad = match profile.sockets.get(sock) {
            None => Some("unknown socket"),
            Some(s) => {
                if !profile.appendage_types.iter().any(|t| t == ty) {
                    Some("unknown appendage type")
                } else if !s.accepts.iter().any(|t| t == ty) {
                    Some("socket does not accept type")
                } else if !sockets_used.insert(sock.to_string()) {
                    Some("socket used twice")
                } else if !src.annotation.landmarks.contains_key(sock) {
                    Some("socket landmark not annotated")
                } else {
                    None
                }
            }
        };
        if let Some(r) = bad {
            emit(
                Severity::Fail,
                "appendage.socket",
                sock,
                format!("{r} for type '{ty}'"),
            );
        }
    }
    for (id, lm) in &profile.landmarks {
        if is_required(lm.class) && !src.annotation.landmarks.contains_key(id) {
            emit(
                Severity::Fail,
                "landmark.missing",
                id,
                "required landmark not annotated".into(),
            );
        }
    }

    // ---- image load + canvas ----
    let (mut img_size, mut canvas_fail) = ([0u32; 2], false);
    let mut mask: Vec<bool> = Vec::new();
    if let Ok(bytes) = &img_bytes {
        match crate::png_io::decode_png(bytes) {
            Ok(img) => {
                img_size = [img.w, img.h];
                if img.w != img.h {
                    canvas_fail = true;
                    emit(
                        Severity::Fail,
                        "canvas.not_square",
                        &format!("{}x{}", img.w, img.h),
                        "canvas must be square".into(),
                    );
                }
                if img.w < profile.canvas.min_px || img.h < profile.canvas.min_px {
                    canvas_fail = true;
                    emit(
                        Severity::Fail,
                        "canvas.too_small",
                        &format!("{}x{}", img.w, img.h),
                        format!("canvas smaller than {} px", profile.canvas.min_px),
                    );
                }
                let (m, bg_err, _bg) = foreground(&img, profile);
                if let Some(e) = bg_err {
                    canvas_fail = true;
                    emit(Severity::Fail, "canvas.background", "image", e);
                } else {
                    mask = m;
                    // silhouette crop
                    let (w, h) = (img.w as usize, img.h as usize);
                    let margin_y = (profile.canvas.crop_margin * h as f64).ceil() as usize;
                    let margin_x = (profile.canvas.crop_margin * w as f64).ceil() as usize;
                    let mut has = [false; 4]; // top,bottom,left,right
                    for y in 0..margin_y.min(h) {
                        for x in 0..w {
                            if mask[y * w + x] {
                                has[0] = true;
                            }
                            if mask[(h - 1 - y) * w + x] {
                                has[1] = true;
                            }
                        }
                    }
                    for x in 0..margin_x.min(w) {
                        for y in 0..h {
                            if mask[y * w + x] {
                                has[2] = true;
                            }
                            if mask[y * w + (w - 1 - x)] {
                                has[3] = true;
                            }
                        }
                    }
                    if has.iter().any(|b| *b) {
                        canvas_fail = true;
                        emit(
                            Severity::Fail,
                            "canvas.cropped",
                            &format!("{:?}", has),
                            "silhouette touches the crop margin".into(),
                        );
                    }
                    // detached components
                    let thresh = (0.0005 * (w * h) as f64) as usize;
                    let big = components(&mask, w, h)
                        .into_iter()
                        .filter(|n| *n > thresh)
                        .count();
                    if big > 1 {
                        canvas_fail = true;
                        emit(
                            Severity::Fail,
                            "topology.detached",
                            "foreground",
                            format!("{big} detached foreground components"),
                        );
                    }
                }
            }
            Err(e) => emit(Severity::Fail, "canvas.background", &src.image.path, e),
        }
    }

    // ---- landmarks ----
    let lm: LmMap = src
        .annotation
        .landmarks
        .iter()
        .map(|(k, v)| (k.clone(), *v))
        .collect();
    if !canvas_fail && !mask.is_empty() {
        let (w, _h) = (img_size[0] as usize, img_size[1] as usize);
        for (id, def) in &profile.landmarks {
            let pos = match lm.get(id) {
                Some(v) => *v,
                None => continue,
            };
            let cls = match def.class {
                LmClass::J => "J",
                LmClass::F => "F",
                LmClass::T => "T",
                LmClass::S => "S",
                LmClass::R => "",
            };
            let check = match def.class {
                LmClass::J | LmClass::F | LmClass::T => true,
                // sockets only checked when an appendage uses them
                LmClass::S => src
                    .annotation
                    .appendages
                    .iter()
                    .any(|a| a.len() == 2 && a[1] == *id),
                LmClass::R => false,
            };
            if check {
                let r = *profile.canvas.on_character.get(cls).unwrap_or(&def.r);
                if !has_fg_near(&mask, img_size, pos, r) {
                    emit(
                        Severity::Fail,
                        "landmark.off_character",
                        id,
                        format!(
                            "({:.3},{:.3}) has no foreground within {:.4}",
                            pos[0], pos[1], r
                        ),
                    );
                }
            }
            if matches!(def.class, LmClass::R | LmClass::J | LmClass::T | LmClass::F) {
                let m = profile.canvas.safe_margin;
                if pos[0] < m || pos[0] > 1.0 - m || pos[1] < m || pos[1] > 1.0 - m {
                    emit(
                        Severity::Fail,
                        "landmark.out_of_margin",
                        id,
                        format!("({:.3},{:.3}) outside safe margin {m}", pos[0], pos[1]),
                    );
                }
            }
        }
        let _ = w;
    }

    // ---- envelope ----
    let accepted: BTreeMap<&str, &str> = src
        .annotation
        .accepted_deviations
        .iter()
        .map(|d| (d.landmark.as_str(), d.reason.as_str()))
        .collect();
    for (id, def) in &profile.landmarks {
        let pos = match lm.get(id) {
            Some(v) => *v,
            None => continue,
        };
        let d = ((pos[0] - def.x).powi(2) + (pos[1] - def.y).powi(2)).sqrt();
        if d > def.r {
            if let Some(reason) = accepted.get(id.as_str()) {
                emit(
                    Severity::Info,
                    "envelope.accepted",
                    id,
                    format!("deviation {d:.4} > {:.3}: {reason}", def.r),
                );
            } else {
                emit(
                    Severity::Warn,
                    "envelope.deviation",
                    id,
                    format!("deviation {d:.4} > {:.3} from dummy", def.r),
                );
            }
        } else if accepted.contains_key(id.as_str()) {
            emit(
                Severity::Warn,
                "envelope.stale_acceptance",
                id,
                format!(
                    "deviation {d:.4} now within {:.3} — remove accepted_deviation",
                    def.r
                ),
            );
        }
    }

    // ---- pose + proportions ----
    eval_pose_constraints(profile, &lm, &mut emit);
    eval_proportions(profile, &lm, &mut emit);

    // ---- clearance (pixel scan; skipped when canvas failed) ----
    if !canvas_fail && !mask.is_empty() && img_size[0] > 0 {
        let w = img_size[0] as usize;
        let h = img_size[1] as usize;
        for c in &profile.clearance {
            if !lm_ok(
                &c.at,
                &lm,
                profile,
                &mut emit,
                &format!("clearance {}", c.name),
            ) || !lm_ok(
                &c.toward,
                &lm,
                profile,
                &mut emit,
                &format!("clearance {}", c.name),
            ) {
                continue;
            }
            let (a, b) = (lm[&c.at], lm[&c.toward]);
            let ya = ((a[1] * h as f64).round() as i64).clamp(0, h as i64 - 1) as usize;
            let xa = ((a[0] * w as f64).round() as i64).clamp(0, w as i64 - 1) as usize;
            let xb = ((b[0] * w as f64).round() as i64).clamp(0, w as i64 - 1) as usize;
            if !mask[ya * w + xa] {
                continue; // `at` off-character: already a landmark FAIL
            }
            let step: i64 = if xb >= xa { 1 } else { -1 };
            let mut x = xa as i64 + step;
            let mut gap = 0usize;
            let mut hit = false;
            // phase 1: skip foreground (still inside the limb)
            while x != xb as i64 && x >= 0 && x < w as i64 {
                if !mask[ya * w + x as usize] {
                    break;
                }
                x += step;
            }
            // phase 2: count background until foreground or target
            while x != xb as i64 && x >= 0 && x < w as i64 {
                if mask[ya * w + x as usize] {
                    hit = true;
                    break;
                }
                gap += 1;
                x += step;
            }
            let _ = hit;
            let g = gap as f64 / w as f64;
            if g < c.fail {
                emit(
                    Severity::Fail,
                    "clearance.gap",
                    &c.name,
                    format!("gap {g:.4} < {:.3}", c.fail),
                );
            } else if g < c.warn {
                emit(
                    Severity::Warn,
                    "clearance.gap",
                    &c.name,
                    format!("gap {g:.4} < {:.3}", c.warn),
                );
            }
        }
    }

    // order + counts
    let (mut f, mut w_, mut i) = (0, 0, 0);
    for x in &findings {
        match x.severity {
            Severity::Fail => f += 1,
            Severity::Warn => w_ += 1,
            Severity::Info => i += 1,
        }
    }
    let source_rel = src_path
        .strip_prefix(repo_root)
        .unwrap_or(src_path)
        .to_string_lossy()
        .replace('\\', "/");
    ValidationReport {
        format: "mascot-character-validation/1",
        source: source_rel,
        character_id: src.character_id.clone(),
        profile: ProfileRef {
            id: profile.id.clone(),
            revision: profile.revision,
            sha256: profile_sha.to_string(),
        },
        image: ImageRef {
            sha256: img_hash,
            size: img_size,
        },
        result: if f > 0 { "FAIL".into() } else { "PASS".into() },
        counts: crate::report::Counts {
            fail: f,
            warn: w_,
            info: i,
        },
        findings,
        tool: crate::report::tool(),
    }
}

fn has_fg_near(mask: &[bool], size: [u32; 2], pos: [f64; 2], r: f64) -> bool {
    let (w, h) = (size[0] as usize, size[1] as usize);
    if w == 0 || h == 0 {
        return false;
    }
    let (cx, cy) = (pos[0] * w as f64, pos[1] * h as f64);
    let rr = r * w as f64;
    let x0 = ((cx - rr).floor() as i64).max(0) as usize;
    let y0 = ((cy - rr).floor() as i64).max(0) as usize;
    let x1 = ((cx + rr).ceil() as i64).min(w as i64 - 1).max(0) as usize;
    let y1 = ((cy + rr).ceil() as i64).min(h as i64 - 1).max(0) as usize;
    for y in y0..=y1 {
        for x in x0..=x1 {
            let dx = x as f64 + 0.5 - cx;
            let dy = y as f64 + 0.5 - cy;
            if dx * dx + dy * dy <= rr * rr && mask[y * w + x] {
                return true;
            }
        }
    }
    false
}

/// Locate `assets/character-templates/<id>/profile.json` walking up from `dir`.
pub fn find_profile_up(dir: &Path, id: &str) -> Option<PathBuf> {
    let mut d = Some(dir.to_path_buf());
    while let Some(dd) = d {
        let cand = dd
            .join("assets")
            .join("character-templates")
            .join(id)
            .join("profile.json");
        if cand.exists() {
            return Some(cand);
        }
        d = dd.parent().map(|p| p.to_path_buf());
    }
    None
}
