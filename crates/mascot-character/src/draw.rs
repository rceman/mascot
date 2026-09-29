//! Deterministic CPU SDF rasterizer (f64). Normalized [0,1] coordinates
//! map onto a square-ish canvas: x *= w, y *= h.

use crate::png_io::Image;

/// A point resolved to normalized coordinates.
pub type Pt = [f64; 2];

/// Exact signed distance to an axis-aligned ellipse boundary
/// (Newton iteration on the closest-point problem).
fn ellipse_sd(p: Pt, c: Pt, rx: f64, ry: f64) -> f64 {
    let px = (p[0] - c[0]).abs();
    let py = (p[1] - c[1]).abs();
    if rx <= 0.0 || ry <= 0.0 {
        return (px * px + py * py).sqrt();
    }
    // inside test
    let inside = (px / rx).powi(2) + (py / ry).powi(2) < 1.0;
    // closest point on ellipse via Newton on t in [0, pi/2]
    let mut t = {
        // initial guess from the normalized angle
        (py * rx).atan2(px * ry)
    };
    let mut cx = rx;
    let mut cy = 0.0;
    for _ in 0..16 {
        let (sx, sy) = (rx * t.cos(), ry * t.sin());
        let (ex, ey) = (sx - px, sy - py);
        let (vx, vy) = (-rx * t.sin(), ry * t.cos());
        let (ax_, ay_) = (-rx * t.cos(), -ry * t.sin());
        let f = ex * vx + ey * vy;
        let fp = vx * vx + vy * vy + ex * ax_ + ey * ay_;
        if fp.abs() < 1e-18 {
            break;
        }
        let dt = f / fp;
        t -= dt;
        if dt.abs() < 1e-14 {
            cx = sx;
            cy = sy;
            break;
        }
        cx = sx;
        cy = sy;
    }
    let d = ((cx - px).powi(2) + (cy - py).powi(2)).sqrt();
    if inside { -d } else { d }
}

#[derive(Clone)]
pub enum Shape {
    Capsule {
        a: Pt,
        b: Pt,
        r: f64,
        offset: [f64; 2],
    },
    Circle {
        c: Pt,
        r: f64,
    },
    Ellipse {
        c: Pt,
        rx: f64,
        ry: f64,
    },
    Poly {
        pts: Vec<Pt>,
    },
    RRect {
        c: Pt,
        w: f64,
        h: f64,
        r: f64,
    },
}

impl Shape {
    /// Signed distance in NORMALIZED units (positive outside).
    fn sd(&self, p: Pt) -> f64 {
        match self {
            Shape::Capsule { a, b, r, offset } => {
                let a = [a[0] + offset[0], a[1] + offset[1]];
                let b = [b[0] + offset[0], b[1] + offset[1]];
                let (px, py) = (p[0] - a[0], p[1] - a[1]);
                let (qx, qy) = (b[0] - a[0], b[1] - a[1]);
                let t = ((px * qx + py * qy) / (qx * qx + qy * qy)).clamp(0.0, 1.0);
                ((px - qx * t).powi(2) + (py - qy * t).powi(2)).sqrt() - r
            }
            Shape::Circle { c, r } => ((p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2)).sqrt() - r,
            Shape::Ellipse { c, rx, ry } => ellipse_sd(p, *c, *rx, *ry),
            Shape::Poly { pts } => {
                // even-odd inside test + min distance to any edge
                let mut inside = false;
                let n = pts.len();
                let mut dmin = f64::INFINITY;
                for i in 0..n {
                    let (ax, ay) = (pts[i][0], pts[i][1]);
                    let (bx, by) = (pts[(i + 1) % n][0], pts[(i + 1) % n][1]);
                    // ray crossing (even-odd)
                    if (ay > p[1]) != (by > p[1]) {
                        let xi = ax + (p[1] - ay) * (bx - ax) / (by - ay);
                        if p[0] < xi {
                            inside = !inside;
                        }
                    }
                    let (px, py) = (p[0] - ax, p[1] - ay);
                    let (qx, qy) = (bx - ax, by - ay);
                    let len2 = qx * qx + qy * qy;
                    let t = if len2 > 0.0 {
                        ((px * qx + py * qy) / len2).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    dmin = dmin.min((px - qx * t).powi(2) + (py - qy * t).powi(2));
                }
                let d = dmin.sqrt();
                if inside { -d } else { d }
            }
            Shape::RRect { c, w, h, r } => {
                // exact rounded-rect SDF (IQ)
                let (hx, hy) = (w / 2.0, h / 2.0);
                let qx = (p[0] - c[0]).abs() - hx + r;
                let qy = (p[1] - c[1]).abs() - hy + r;
                let dx = qx.max(0.0);
                let dy = qy.max(0.0);
                (dx * dx + dy * dy).sqrt() + qx.max(qy).min(0.0) - r
            }
        }
    }

    /// Loose bounding box (normalized), including `pad`.
    fn bounds(&self, pad: f64) -> [f64; 4] {
        let mut bb = [
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ];
        let mut grow = |x: f64, y: f64| {
            bb[0] = bb[0].min(x - pad);
            bb[1] = bb[1].min(y - pad);
            bb[2] = bb[2].max(x + pad);
            bb[3] = bb[3].max(y + pad);
        };
        match self {
            Shape::Capsule { a, b, r, offset } => {
                for p in [a, b] {
                    grow(p[0] + offset[0], p[1] + offset[1]);
                }
                bb[0] -= r;
                bb[1] -= r;
                bb[2] += r;
                bb[3] += r;
            }
            Shape::Circle { c, r } => {
                bb = [
                    c[0] - r - pad,
                    c[1] - r - pad,
                    c[0] + r + pad,
                    c[1] + r + pad,
                ];
            }
            Shape::Ellipse { c, rx, ry } => {
                bb = [
                    c[0] - rx - pad,
                    c[1] - ry - pad,
                    c[0] + rx + pad,
                    c[1] + ry + pad,
                ];
            }
            Shape::Poly { pts } => {
                for p in pts {
                    grow(p[0], p[1]);
                }
            }
            Shape::RRect { c, w, h, .. } => {
                bb = [
                    c[0] - w / 2.0 - pad,
                    c[1] - h / 2.0 - pad,
                    c[0] + w / 2.0 + pad,
                    c[1] + h / 2.0 + pad,
                ];
            }
        }
        bb
    }
}

/// Rasterize `shapes` as a union: outline of width `line_w` in `line`,
/// then fill in `fill`, composited onto `img`. `lw`/`fill`/`line` are RGBA.
/// `size` is the px per normalized unit (square canvas = img.w used for x,
/// img.h for y; use square callers).
pub fn render_union(img: &mut Image, shapes: &[Shape], fill: [u8; 4], line: [u8; 4], line_w: f64) {
    if shapes.is_empty() {
        return;
    }
    let sx = img.w as f64;
    let sy = img.h as f64;
    let mut bb = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for s in shapes {
        let b = s.bounds(line_w + 2.0 / sx);
        for i in 0..4 {
            if i < 2 {
                bb[i] = bb[i].min(b[i]);
            } else {
                bb[i] = bb[i].max(b[i]);
            }
        }
    }
    let x0 = ((bb[0] * sx).floor() as i64).clamp(0, img.w as i64) as u32;
    let y0 = ((bb[1] * sy).floor() as i64).clamp(0, img.h as i64) as u32;
    let x1 = ((bb[2] * sx).ceil() as i64).clamp(0, img.w as i64) as u32;
    let y1 = ((bb[3] * sy).ceil() as i64).clamp(0, img.h as i64) as u32;
    for y in y0..y1 {
        for x in x0..x1 {
            let px = [(x as f64 + 0.5) / sx, (y as f64 + 0.5) / sy];
            let mut sd = f64::INFINITY;
            for s in shapes {
                sd = sd.min(s.sd(px));
            }
            let sd_px = sd * sx; // distance in x-px (square canvases)
            let fcov = (0.5 - sd_px).clamp(0.0, 1.0);
            let ocov = (0.5 - (sd_px - line_w * sx)).clamp(0.0, 1.0);
            // outline band sits on the outside edge; fill covers inside
            img.over(x, y, line, ocov);
            img.over(x, y, fill, fcov);
        }
    }
}

/// Draw a small filled circle (for landmark dots).
pub fn disc(img: &mut Image, c: Pt, r_px: f64, color: [u8; 4]) {
    let sx = img.w as f64;
    let sy = img.h as f64;
    let x0 = (((c[0] - r_px / sx) * sx).floor() as i64).clamp(0, img.w as i64) as u32;
    let y0 = (((c[1] - r_px / sy) * sy).floor() as i64).clamp(0, img.h as i64) as u32;
    let x1 = (((c[0] + r_px / sx) * sx).ceil() as i64).clamp(0, img.w as i64) as u32;
    let y1 = (((c[1] + r_px / sy) * sy).ceil() as i64).clamp(0, img.h as i64) as u32;
    for y in y0..y1 {
        for x in x0..x1 {
            let dx = x as f64 + 0.5 - c[0] * sx;
            let dy = y as f64 + 0.5 - c[1] * sy;
            let cov = (r_px + 0.5 - (dx * dx + dy * dy).sqrt()).clamp(0.0, 1.0);
            img.over(x, y, color, cov);
        }
    }
}

/// Draw text in `img` (top-left at `x`,`y` px) with per-pixel over-compositing.
pub fn text(
    img: &mut Image,
    x: i32,
    y: i32,
    scale: i32,
    s: &str,
    color: [u8; 4],
    halo: Option<[u8; 4]>,
) {
    // collect glyph pixels first so the halo never overwrites lit pixels
    let mut px: Vec<(i32, i32)> = Vec::new();
    let mut cx = x;
    for ch in s.chars() {
        let g = crate::font::Font::glyph(ch);
        for (row, bits) in g.iter().enumerate() {
            for col in 0..5 {
                if bits & (1 << (4 - col)) != 0 {
                    for sy_ in 0..scale {
                        for sx_ in 0..scale {
                            px.push((cx + col * scale + sx_, y + row as i32 * scale + sy_));
                        }
                    }
                }
            }
        }
        cx += crate::font::Font::ADVANCE * scale;
    }
    if let Some(hc) = halo {
        for &(gx, gy) in &px {
            for hy in -1..=1 {
                for hx in -1..=1 {
                    let (xx, yy) = (gx + hx, gy + hy);
                    if xx >= 0 && yy >= 0 && xx < img.w as i32 && yy < img.h as i32 {
                        img.over(xx as u32, yy as u32, hc, 1.0);
                    }
                }
            }
        }
    }
    for (gx, gy) in px {
        if gx >= 0 && gy >= 0 && gx < img.w as i32 && gy < img.h as i32 {
            img.over(gx as u32, gy as u32, color, 1.0);
        }
    }
}
