//! Deterministic guide rendering: generation dummy + annotated guide + receipt.

use crate::draw::{self, Shape};
use crate::png_io::Image;
use crate::profile::{GuideDef, LmClass, Profile, resolve_pt};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;

pub fn hex(s: &str) -> Result<[u8; 4], String> {
    let s = s.trim_start_matches('#');
    if s.len() != 6 {
        return Err(format!("bad colour #{s}"));
    }
    let v = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| e.to_string());
    Ok([v(0)?, v(2)?, v(4)?, 255])
}

/// Convert one shape spec into a raster `Shape`.
pub fn to_shape(p: &Profile, def: &crate::profile::ShapeDef) -> Result<Shape, String> {
    if let Some(v) = &def.capsule {
        if v.len() != 2 || def.w.is_none() {
            return Err("capsule needs [p,q] + w".into());
        }
        return Ok(Shape::Capsule {
            a: resolve_pt(p, &v[0])?,
            b: resolve_pt(p, &v[1])?,
            r: def.w.unwrap() / 2.0,
            offset: def.offset.unwrap_or([0.0, 0.0]),
        });
    }
    if let Some(v) = &def.circle {
        return Ok(Shape::Circle {
            c: resolve_pt(p, v)?,
            r: def.r.ok_or("circle needs r")?,
        });
    }
    if let Some(v) = &def.ellipse {
        return Ok(Shape::Ellipse {
            c: resolve_pt(p, v)?,
            rx: def.rx.ok_or("ellipse needs rx")?,
            ry: def.ry.ok_or("ellipse needs ry")?,
        });
    }
    if let Some(v) = &def.poly {
        let mut pts = Vec::new();
        for pt in v {
            pts.push(resolve_pt(p, pt)?);
        }
        if pts.len() < 3 {
            return Err("poly needs >=3 points".into());
        }
        return Ok(Shape::Poly { pts });
    }
    if let Some(v) = &def.rrect {
        return Ok(Shape::RRect {
            c: resolve_pt(p, v)?,
            w: def.w.ok_or("rrect needs w")?,
            h: def.h.ok_or("rrect needs h")?,
            r: def.r.ok_or("rrect needs r")?,
        });
    }
    Err("shape spec has no kind".into())
}

/// Render the dummy (guide layers) at `size` px onto `background`.
pub fn render_layers(
    p: &Profile,
    layers: &[crate::profile::LayerDef],
    defaults: &GuideDef,
    size: u32,
    background: [u8; 4],
) -> Result<Image, String> {
    let mut img = Image::new(size, size, background);
    for layer in layers {
        let mut shapes = Vec::new();
        for s in &layer.shapes {
            shapes.push(to_shape(p, s)?);
        }
        let line = hex(layer.line.as_deref().unwrap_or(&defaults.line))?;
        let lw = layer.line_w.unwrap_or(defaults.line_w);
        let fill = hex(&layer.fill)?;
        draw::render_union(&mut img, &shapes, fill, line, lw);
    }
    Ok(img)
}

/// Render the generation guide (no text/dots).
pub fn render_generation(p: &Profile, size: u32) -> Result<Image, String> {
    render_layers(
        p,
        &p.guide.layers,
        &p.guide,
        size,
        hex(&p.guide.background)?,
    )
}

/// Render the annotated guide: mannequin + ground lines + zones + landmark
/// dots coloured by class + id labels.
pub fn render_annotated(p: &Profile, size: u32) -> Result<Image, String> {
    let mut img = render_layers(
        p,
        &p.guide.layers,
        &p.guide,
        size,
        hex(&p.guide.background)?,
    )?;
    let s = size as f64;
    // ground lines (near/far, thin)
    let gy = [
        (p.ground.near, [120u8, 160, 120, 255]),
        (p.ground.far, [160u8, 200, 160, 255]),
    ];
    for (y, c) in gy {
        let py = (y * s).round() as u32;
        for x in 0..img.w {
            img.over(x, py, c, 0.8);
            if py + 1 < img.h {
                img.over(x, py + 1, c, 0.8);
            }
        }
    }
    // zones (thin outlines)
    for z in p.zones.values() {
        let c = [150u8, 120, 200, 255];
        let (x0, y0) = ((z[0] * s).round() as u32, (z[1] * s).round() as u32);
        let (x1, y1) = ((z[2] * s).round() as u32, (z[3] * s).round() as u32);
        for x in x0..=x1.min(img.w - 1) {
            img.over(x, y0, c, 0.9);
            img.over(x, y1.min(img.h - 1), c, 0.9);
        }
        for y in y0..=y1.min(img.h - 1) {
            img.over(x0, y, c, 0.9);
            img.over(x1.min(img.w - 1), y, c, 0.9);
        }
    }
    // landmark dots, then labels with deterministic collision avoidance:
    // candidates right/left/above/below at 1x and 2x distance, first
    // non-intersecting box wins (else the first candidate).
    let scale = (size / 683).max(1) as i32;
    let r = (0.006 * s).max(4.0);
    let mut placed: Vec<[i32; 4]> = Vec::new();
    let mut label_jobs: Vec<(i32, i32, String)> = Vec::new();
    for (id, lm) in &p.landmarks {
        let c = match lm.class {
            LmClass::R => [255u8, 140, 0, 255],
            LmClass::J => [220u8, 30, 30, 255],
            LmClass::T => [40u8, 90, 230, 255],
            LmClass::F => [40u8, 160, 60, 255],
            LmClass::S => [220u8, 30, 220, 255],
        };
        draw::disc(&mut img, [lm.x, lm.y], r + 2.0, [30, 30, 30, 255]);
        draw::disc(&mut img, [lm.x, lm.y], r, c);
        let cx = (lm.x * s).round() as i32;
        let cy = (lm.y * s).round() as i32;
        label_jobs.push((cx, cy, id.clone()));
        let ri = (r + 2.0) as i32;
        placed.push([cx - ri, cy - ri, cx + ri, cy + ri]);
    }
    let text_h = 7 * scale;
    let gap = scale * 2;
    for (cx, cy, id) in &label_jobs {
        let tw = crate::font::Font::width(id, scale);
        let d = (r + 2.0) as i32 + gap;
        let mut candidates: Vec<(i32, i32)> = Vec::new();
        for m in [1i32, 2] {
            let dd = d * m;
            candidates.push((cx + dd, cy - text_h / 2)); // right
            candidates.push((cx - dd - tw, cy - text_h / 2)); // left
            candidates.push((cx - tw / 2, cy - dd - text_h)); // above
            candidates.push((cx - tw / 2, cy + dd)); // below
        }
        let mut chosen = candidates[0];
        for &(x, y) in &candidates {
            let bx = [x - scale, y - scale, x + tw + scale, y + text_h + scale];
            let hit = placed
                .iter()
                .any(|b| bx[0] <= b[2] && bx[2] >= b[0] && bx[1] <= b[3] && bx[3] >= b[1]);
            if !hit {
                chosen = (x, y);
                break;
            }
        }
        placed.push([
            chosen.0 - scale,
            chosen.1 - scale,
            chosen.0 + tw + scale,
            chosen.1 + text_h + scale,
        ]);
        draw::text(
            &mut img,
            chosen.0,
            chosen.1,
            scale,
            id,
            [0, 0, 0, 255],
            Some([255, 255, 255, 255]),
        );
    }
    Ok(img)
}

#[derive(Serialize)]
pub struct GuideReceiptOut {
    pub size: [u32; 2],
    pub sha256: String,
}

#[derive(Serialize)]
pub struct GuideReceipt {
    pub format: &'static str,
    pub profile: crate::report::ProfileRef,
    pub outputs: BTreeMap<String, GuideReceiptOut>,
    pub tool: crate::report::Tool,
}

/// Write `dummy-generation.png`, `dummy-annotated.png`, `guide.json` into `dir`.
pub fn write_guide(p: &Profile, profile_bytes: &[u8], dir: &Path) -> Result<GuideReceipt, String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut outputs = BTreeMap::new();
    for (name, size, img) in [
        (
            "dummy-generation.png",
            p.guide.sizes.generation,
            render_generation(p, p.guide.sizes.generation)?,
        ),
        (
            "dummy-annotated.png",
            p.guide.sizes.annotated,
            render_annotated(p, p.guide.sizes.annotated)?,
        ),
    ] {
        let path = dir.join(name);
        crate::png_io::save_png(&path, &img)?;
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        outputs.insert(
            name.to_string(),
            GuideReceiptOut {
                size: [size, size],
                sha256: crate::hash::sha256_hex(&bytes),
            },
        );
    }
    let receipt = GuideReceipt {
        format: "mascot-guide-receipt/1",
        profile: crate::report::ProfileRef {
            id: p.id.clone(),
            revision: p.revision,
            sha256: crate::hash::sha256_hex(profile_bytes),
        },
        outputs,
        tool: crate::report::tool(),
    };
    let j = serde_json::to_string_pretty(&receipt).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("guide.json"), j + "\n").map_err(|e| e.to_string())?;
    Ok(receipt)
}
