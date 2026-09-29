//! Dev-only SVG-subset parser + `generated.rs` emitter for the Lucide sources.
//!
//! Compiled only into the `mascot-icons-gen` binary and into `#[cfg(test)]`
//! builds of the library — never into the shipping library API surface.
//!
//! Supported subset (everything upstream Lucide uses):
//! `<path d>` with M/L/H/V/C/S/Q/T/A/Z in absolute and relative form,
//! `<circle>`, `<ellipse>`, `<rect>` (incl. rx/ry), `<line>`, `<polyline>`,
//! `<polygon>`. All output is normalized to absolute move/line/cubic/close
//! commands in the source 24x24 coordinate space.

use std::fmt::Write as _;
use std::path::Path;

/// Normalized absolute path command (mirrors `crate::Seg`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PSeg {
    M(f32, f32),
    L(f32, f32),
    C(f32, f32, f32, f32, f32, f32),
    Z,
}

#[derive(Debug)]
pub struct ParseError(pub String);

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

type PResult<T> = Result<T, ParseError>;

fn err<T>(msg: impl Into<String>) -> PResult<T> {
    Err(ParseError(msg.into()))
}

// ---------------------------------------------------------------------------
// Element scanner: yields (tag name, attributes) for start/empty tags.
// ---------------------------------------------------------------------------

fn scan_elements(svg: &str, mut f: impl FnMut(&str, &str)) -> PResult<()> {
    let b = svg.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'<' {
            i += 1;
            continue;
        }
        // skip declarations, comments, closing tags
        if b.get(i + 1)
            .is_some_and(|c| matches!(c, b'/' | b'?' | b'!'))
        {
            i += 1;
            continue;
        }
        let name_start = i + 1;
        let mut j = name_start;
        while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'-' || b[j] == b':') {
            j += 1;
        }
        let name = &svg[name_start..j];
        // find the end of this tag, honouring quoted strings
        let mut k = j;
        let mut in_q = None;
        while k < b.len() {
            match (in_q, b[k]) {
                (Some(q), c) if c == q => in_q = None,
                (None, b'"' | b'\'') => in_q = Some(b[k]),
                (None, b'>') => break,
                _ => {}
            }
            k += 1;
        }
        if k >= b.len() {
            return err("unterminated tag");
        }
        f(name, &svg[j..k]);
        i = k + 1;
    }
    Ok(())
}

fn attrs(tag: &str) -> Vec<(String, String)> {
    let b = tag.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        while i < b.len() && (b[i].is_ascii_whitespace() || b[i] == b'/') {
            i += 1;
        }
        let s = i;
        while i < b.len() && (b[i].is_ascii_alphanumeric() || matches!(b[i], b'-' | b'_' | b':')) {
            i += 1;
        }
        if i == s {
            break;
        }
        let name = tag[s..i].to_string();
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= b.len() || b[i] != b'=' {
            out.push((name, String::new()));
            continue;
        }
        i += 1;
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= b.len() || (b[i] != b'"' && b[i] != b'\'') {
            return out;
        }
        let q = b[i];
        i += 1;
        let vs = i;
        while i < b.len() && b[i] != q {
            i += 1;
        }
        out.push((name, tag[vs..i].to_string()));
        i += 1;
    }
    out
}

fn attr<'a>(attrs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| v.as_str())
}

fn num(attrs: &[(String, String)], name: &str, default: f32) -> PResult<f32> {
    match attr(attrs, name) {
        None => Ok(default),
        Some(v) => v
            .trim()
            .parse::<f32>()
            .map_err(|_| ParseError(format!("bad number {name}={v:?}"))),
    }
}

// ---------------------------------------------------------------------------
// Path `d` parser -> normalized absolute segments.
// ---------------------------------------------------------------------------

struct Cursor<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Cursor<'a> {
    fn new(s: &'a str) -> Self {
        Cursor {
            b: s.as_bytes(),
            i: 0,
        }
    }
    fn skip_ws(&mut self) {
        while self.i < self.b.len()
            && (self.b[self.i].is_ascii_whitespace() || self.b[self.i] == b',')
        {
            self.i += 1;
        }
    }
    fn eof(&mut self) -> bool {
        self.skip_ws();
        self.i >= self.b.len()
    }
    fn peek_cmd(&mut self) -> Option<u8> {
        self.skip_ws();
        self.b
            .get(self.i)
            .copied()
            .filter(|c| c.is_ascii_alphabetic())
    }
    fn number(&mut self) -> PResult<f32> {
        self.skip_ws();
        let s = self.i;
        if self.i < self.b.len() && (self.b[self.i] == b'-' || self.b[self.i] == b'+') {
            self.i += 1;
        }
        let mut seen_digit = false;
        let mut seen_dot = false;
        while self.i < self.b.len() {
            let c = self.b[self.i];
            if c.is_ascii_digit() {
                seen_digit = true;
                self.i += 1;
            } else if c == b'.' && !seen_dot {
                seen_dot = true;
                self.i += 1;
            } else if (c == b'e' || c == b'E') && seen_digit {
                self.i += 1;
                if self.i < self.b.len() && (self.b[self.i] == b'-' || self.b[self.i] == b'+') {
                    self.i += 1;
                }
            } else {
                break;
            }
        }
        if !seen_digit {
            return err(format!(
                "expected number at byte {} in {:?}",
                s,
                String::from_utf8_lossy(self.b)
            ));
        }
        self.b[s..self.i]
            .iter()
            .map(|c| *c as char)
            .collect::<String>()
            .parse::<f32>()
            .map_err(|_| ParseError("bad float".into()))
    }
    fn flag(&mut self) -> PResult<bool> {
        self.skip_ws();
        match self.b.get(self.i) {
            Some(b'0') => {
                self.i += 1;
                Ok(false)
            }
            Some(b'1') => {
                self.i += 1;
                Ok(true)
            }
            _ => err("expected arc flag 0/1"),
        }
    }
}

struct PathBuilder {
    out: Vec<PSeg>,
    cur: (f32, f32),
    start: (f32, f32),
    /// Last cubic control point for S reflection / quadratic control for T.
    prev_c2: Option<(f32, f32)>,
    prev_q: Option<(f32, f32)>,
}

impl PathBuilder {
    fn new() -> Self {
        PathBuilder {
            out: Vec::new(),
            cur: (0.0, 0.0),
            start: (0.0, 0.0),
            prev_c2: None,
            prev_q: None,
        }
    }
    fn moveto(&mut self, x: f32, y: f32) {
        self.cur = (x, y);
        self.start = (x, y);
        self.prev_c2 = None;
        self.prev_q = None;
        self.out.push(PSeg::M(x, y));
    }
    fn lineto(&mut self, x: f32, y: f32) {
        self.cur = (x, y);
        self.prev_c2 = None;
        self.prev_q = None;
        self.out.push(PSeg::L(x, y));
    }
    fn cubicto(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.cur = (x, y);
        self.prev_c2 = Some((x2, y2));
        self.prev_q = None;
        self.out.push(PSeg::C(x1, y1, x2, y2, x, y));
    }
    fn quadto(&mut self, qx: f32, qy: f32, x: f32, y: f32) {
        // elevate quadratic to cubic
        let (x0, y0) = self.cur;
        let c1 = (x0 + 2.0 / 3.0 * (qx - x0), y0 + 2.0 / 3.0 * (qy - y0));
        let c2 = (x + 2.0 / 3.0 * (qx - x), y + 2.0 / 3.0 * (qy - y));
        self.cubicto(c1.0, c1.1, c2.0, c2.1, x, y);
        self.prev_q = Some((qx, qy));
    }
    fn close(&mut self) {
        self.cur = self.start;
        self.prev_c2 = None;
        self.prev_q = None;
        self.out.push(PSeg::Z);
    }

    /// SVG endpoint arc -> cubics (spec F.6.5/F.6.4).
    #[allow(clippy::too_many_arguments)]
    fn arcto(
        &mut self,
        mut rx: f32,
        mut ry: f32,
        rot_deg: f32,
        large: bool,
        sweep: bool,
        x: f32,
        y: f32,
    ) {
        let (x1, y1) = self.cur;
        let (x2, y2) = (x, y);
        if rx == 0.0 || ry == 0.0 || (x1 == x2 && y1 == y2) {
            self.lineto(x2, y2);
            return;
        }
        rx = rx.abs();
        ry = ry.abs();
        let phi = rot_deg.to_radians();
        let (cos_p, sin_p) = (phi.cos(), phi.sin());
        let dx = (x1 - x2) / 2.0;
        let dy = (y1 - y2) / 2.0;
        let x1p = cos_p * dx + sin_p * dy;
        let y1p = -sin_p * dx + cos_p * dy;
        // radius correction
        let lam = x1p * x1p / (rx * rx) + y1p * y1p / (ry * ry);
        if lam > 1.0 {
            let s = lam.sqrt();
            rx *= s;
            ry *= s;
        }
        let num = rx * rx * ry * ry - rx * rx * y1p * y1p - ry * ry * x1p * x1p;
        let den = rx * rx * y1p * y1p + ry * ry * x1p * x1p;
        let coef = (num.max(0.0) / den).sqrt() * if large == sweep { -1.0 } else { 1.0 };
        let cxp = coef * rx * y1p / ry;
        let cyp = -coef * ry * x1p / rx;
        let cx = cos_p * cxp - sin_p * cyp + (x1 + x2) / 2.0;
        let cy = sin_p * cxp + cos_p * cyp + (y1 + y2) / 2.0;
        let ang = |ux: f32, uy: f32, vx: f32, vy: f32| -> f32 {
            let n = (ux * ux + uy * uy).sqrt() * (vx * vx + vy * vy).sqrt();
            let c = ((ux * vx + uy * vy) / n).clamp(-1.0, 1.0);
            let s = ux * vy - uy * vx;
            if s < 0.0 { -c.acos() } else { c.acos() }
        };
        let ux = (x1p - cxp) / rx;
        let uy = (y1p - cyp) / ry;
        let mut theta1 = ang(1.0, 0.0, ux, uy);
        let mut dtheta = ang(ux, uy, (-x1p - cxp) / rx, (-y1p - cyp) / ry);
        if !sweep && dtheta > 0.0 {
            dtheta -= std::f32::consts::TAU;
        } else if sweep && dtheta < 0.0 {
            dtheta += std::f32::consts::TAU;
        }
        let nseg = ((dtheta.abs() / (std::f32::consts::FRAC_PI_2)).ceil() as usize).max(1);
        let seg = dtheta / nseg as f32;
        for _ in 0..nseg {
            let theta2 = theta1 + seg;
            let t = 4.0 / 3.0 * (seg / 4.0).tan();
            let e1 = (theta1.cos(), theta1.sin());
            let e2 = (theta2.cos(), theta2.sin());
            let map = |ex: f32, ey: f32| {
                (
                    cx + rx * (cos_p * ex - sin_p * ey),
                    cy + ry * (sin_p * ex + cos_p * ey),
                )
            };
            let p1 = (e1.0 - t * e1.1, e1.1 + t * e1.0);
            let p2 = (e2.0 + t * e2.1, e2.1 - t * e2.0);
            let (c1, c2, p3) = (map(p1.0, p1.1), map(p2.0, p2.1), map(e2.0, e2.1));
            self.cubicto(c1.0, c1.1, c2.0, c2.1, p3.0, p3.1);
            theta1 = theta2;
        }
        self.prev_c2 = None; // arcs break S-reflection chains
    }
}

pub fn parse_path(d: &str) -> PResult<Vec<PSeg>> {
    let mut c = Cursor::new(d);
    let mut b = PathBuilder::new();
    let mut cmd = 0u8;
    while !c.eof() {
        if let Some(nc) = c.peek_cmd() {
            cmd = nc;
            c.i += 1;
            if cmd == b'z' || cmd == b'Z' {
                b.close();
                cmd = 0;
                continue;
            }
        } else if cmd == 0 {
            return err("path data missing command");
        }
        // after the first M/m pair, subsequent pairs are implicit L/l
        let rel = cmd.is_ascii_lowercase();
        let up = cmd.to_ascii_uppercase();
        let off = |b: &PathBuilder, x: f32, y: f32| {
            if rel {
                (x + b.cur.0, y + b.cur.1)
            } else {
                (x, y)
            }
        };
        match up {
            b'M' => {
                let (x, y) = (c.number()?, c.number()?);
                let (x, y) = off(&b, x, y);
                b.moveto(x, y);
                cmd = if rel { b'l' } else { b'L' };
            }
            b'L' => loop {
                let (x, y) = (c.number()?, c.number()?);
                let (x, y) = off(&b, x, y);
                b.lineto(x, y);
                if c.eof() || c.peek_cmd().is_some() {
                    break;
                }
            },
            b'H' => loop {
                let x = c.number()? + if rel { b.cur.0 } else { 0.0 };
                b.lineto(x, b.cur.1);
                if c.eof() || c.peek_cmd().is_some() {
                    break;
                }
            },
            b'V' => loop {
                let y = c.number()? + if rel { b.cur.1 } else { 0.0 };
                b.lineto(b.cur.0, y);
                if c.eof() || c.peek_cmd().is_some() {
                    break;
                }
            },
            b'C' => loop {
                let (x1, y1) = (c.number()?, c.number()?);
                let (x2, y2) = (c.number()?, c.number()?);
                let (x, y) = (c.number()?, c.number()?);
                let (x1, y1) = off(&b, x1, y1);
                let (x2, y2) = off(&b, x2, y2);
                let (x, y) = off(&b, x, y);
                b.cubicto(x1, y1, x2, y2, x, y);
                if c.eof() || c.peek_cmd().is_some() {
                    break;
                }
            },
            b'S' => loop {
                let refl = b
                    .prev_c2
                    .map(|(x2, y2)| (2.0 * b.cur.0 - x2, 2.0 * b.cur.1 - y2))
                    .unwrap_or(b.cur);
                let (x2, y2) = (c.number()?, c.number()?);
                let (x, y) = (c.number()?, c.number()?);
                let (x2, y2) = off(&b, x2, y2);
                let (x, y) = off(&b, x, y);
                b.cubicto(refl.0, refl.1, x2, y2, x, y);
                if c.eof() || c.peek_cmd().is_some() {
                    break;
                }
            },
            b'Q' => loop {
                let (qx, qy) = (c.number()?, c.number()?);
                let (x, y) = (c.number()?, c.number()?);
                let (qx, qy) = off(&b, qx, qy);
                let (x, y) = off(&b, x, y);
                b.quadto(qx, qy, x, y);
                if c.eof() || c.peek_cmd().is_some() {
                    break;
                }
            },
            b'T' => loop {
                let refl = b
                    .prev_q
                    .map(|(qx, qy)| (2.0 * b.cur.0 - qx, 2.0 * b.cur.1 - qy))
                    .unwrap_or(b.cur);
                let (x, y) = (c.number()?, c.number()?);
                let (x, y) = off(&b, x, y);
                b.quadto(refl.0, refl.1, x, y);
                if c.eof() || c.peek_cmd().is_some() {
                    break;
                }
            },
            b'A' => loop {
                let rx = c.number()?;
                let ry = c.number()?;
                let rot = c.number()?;
                let large = c.flag()?;
                let sweep = c.flag()?;
                let (x, y) = (c.number()?, c.number()?);
                let (x, y) = off(&b, x, y);
                b.arcto(rx, ry, rot, large, sweep, x, y);
                if c.eof() || c.peek_cmd().is_some() {
                    break;
                }
            },
            other => return err(format!("unsupported path command {}", other as char)),
        }
    }
    Ok(b.out)
}

// ---------------------------------------------------------------------------
// Shape elements -> path segments.
// ---------------------------------------------------------------------------

/// Cubic circle approximation constant k = 4/3 * tan(pi/8).
const KAPPA: f32 = 0.552_284_8;

fn ellipse_segs(cx: f32, cy: f32, rx: f32, ry: f32) -> Vec<PSeg> {
    let (kx, ky) = (KAPPA * rx, KAPPA * ry);
    vec![
        PSeg::M(cx, cy - ry),
        PSeg::C(cx + kx, cy - ry, cx + rx, cy - ky, cx + rx, cy),
        PSeg::C(cx + rx, cy + ky, cx + kx, cy + ry, cx, cy + ry),
        PSeg::C(cx - kx, cy + ry, cx - rx, cy + ky, cx - rx, cy),
        PSeg::C(cx - rx, cy - ky, cx - kx, cy - ry, cx, cy - ry),
        PSeg::Z,
    ]
}

fn rect_segs(x: f32, y: f32, w: f32, h: f32, rx: f32, ry: f32) -> Vec<PSeg> {
    let rx = rx.min(w / 2.0);
    let ry = ry.min(h / 2.0);
    if rx <= 0.0 || ry <= 0.0 {
        return vec![
            PSeg::M(x, y),
            PSeg::L(x + w, y),
            PSeg::L(x + w, y + h),
            PSeg::L(x, y + h),
            PSeg::Z,
        ];
    }
    let (kx, ky) = (KAPPA * rx, KAPPA * ry);
    vec![
        PSeg::M(x + rx, y),
        PSeg::L(x + w - rx, y),
        PSeg::C(x + w - rx + kx, y, x + w, y + ry - ky, x + w, y + ry),
        PSeg::L(x + w, y + h - ry),
        PSeg::C(
            x + w,
            y + h - ry + ky,
            x + w - rx + kx,
            y + h,
            x + w - rx,
            y + h,
        ),
        PSeg::L(x + rx, y + h),
        PSeg::C(x + rx - kx, y + h, x, y + h - ry + ky, x, y + h - ry),
        PSeg::L(x, y + ry),
        PSeg::C(x, y + ry - ky, x + rx - kx, y, x + rx, y),
        PSeg::Z,
    ]
}

fn parse_points(v: &str) -> PResult<Vec<(f32, f32)>> {
    let mut c = Cursor::new(v);
    let mut out = Vec::new();
    while !c.eof() {
        out.push((c.number()?, c.number()?));
    }
    Ok(out)
}

/// Parses one Lucide SVG into normalized path segments (24x24 space).
pub fn parse_svg(svg: &str) -> PResult<Vec<PSeg>> {
    let mut out = Vec::new();
    let mut view_box_ok = false;
    scan_elements(svg, |name, attrs_src| {
        let a = attrs(attrs_src);
        match name {
            "svg" => {
                if let Some(vb) = attr(&a, "viewBox") {
                    let nums: Vec<&str> = vb.split_whitespace().collect();
                    view_box_ok = nums == ["0", "0", "24", "24"];
                }
            }
            "path" => {
                if let Some(d) = attr(&a, "d") {
                    match parse_path(d) {
                        Ok(mut segs) => out.append(&mut segs),
                        Err(e) => panic!("path {d:?}: {e}"),
                    }
                }
            }
            "circle" => {
                let (cx, cy, r) = (
                    num(&a, "cx", 0.0).unwrap(),
                    num(&a, "cy", 0.0).unwrap(),
                    num(&a, "r", 0.0).unwrap(),
                );
                out.extend(ellipse_segs(cx, cy, r, r));
            }
            "ellipse" => {
                let (cx, cy, rx, ry) = (
                    num(&a, "cx", 0.0).unwrap(),
                    num(&a, "cy", 0.0).unwrap(),
                    num(&a, "rx", 0.0).unwrap(),
                    num(&a, "ry", 0.0).unwrap(),
                );
                out.extend(ellipse_segs(cx, cy, rx, ry));
            }
            "rect" => {
                let (x, y, w, h, rx, ry) = (
                    num(&a, "x", 0.0).unwrap(),
                    num(&a, "y", 0.0).unwrap(),
                    num(&a, "width", 0.0).unwrap(),
                    num(&a, "height", 0.0).unwrap(),
                    num(&a, "rx", 0.0).unwrap(),
                    num(&a, "ry", 0.0).unwrap(),
                );
                // SVG: specifying only one of rx/ry copies it to the other.
                let has_rx = attr(&a, "rx").is_some();
                let has_ry = attr(&a, "ry").is_some();
                let (rx, ry) = match (has_rx, has_ry) {
                    (true, false) => (rx, rx),
                    (false, true) => (ry, ry),
                    _ => (rx, ry),
                };
                out.extend(rect_segs(x, y, w, h, rx, ry));
            }
            "line" => {
                let (x1, y1, x2, y2) = (
                    num(&a, "x1", 0.0).unwrap(),
                    num(&a, "y1", 0.0).unwrap(),
                    num(&a, "x2", 0.0).unwrap(),
                    num(&a, "y2", 0.0).unwrap(),
                );
                out.extend([PSeg::M(x1, y1), PSeg::L(x2, y2)]);
            }
            "polyline" | "polygon" => {
                if let Some(v) = attr(&a, "points") {
                    let pts = parse_points(v).unwrap();
                    if let Some(first) = pts.first() {
                        out.push(PSeg::M(first.0, first.1));
                        out.extend(pts[1..].iter().map(|p| PSeg::L(p.0, p.1)));
                        if name == "polygon" {
                            out.push(PSeg::Z);
                        }
                    }
                }
            }
            _ => {}
        }
    })?;
    if !view_box_ok {
        return err("missing or unexpected viewBox (expected \"0 0 24 24\")");
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// generated.rs emitter.
// ---------------------------------------------------------------------------

fn fmt_num(v: f32) -> String {
    let v = if v == 0.0 { 0.0 } else { v }; // normalize -0
    let mut s = format!("{v:.4}");
    while s.contains('.') && s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.push('0');
    }
    s
}

fn fmt_seg(s: &PSeg) -> String {
    match *s {
        PSeg::M(x, y) => format!("Seg::M({}, {})", fmt_num(x), fmt_num(y)),
        PSeg::L(x, y) => format!("Seg::L({}, {})", fmt_num(x), fmt_num(y)),
        PSeg::C(x1, y1, x2, y2, x, y) => format!(
            "Seg::C({}, {}, {}, {}, {}, {})",
            fmt_num(x1),
            fmt_num(y1),
            fmt_num(x2),
            fmt_num(y2),
            fmt_num(x),
            fmt_num(y)
        ),
        PSeg::Z => "Seg::Z".to_string(),
    }
}

/// Emits the complete contents of `src/generated.rs` for `icons`
/// (`(const_name, lucide_file_stem)` pairs in enum order).
pub fn generate(icons: &[(&str, &str)], source_dir: &Path) -> Result<String, String> {
    let mut out = String::new();
    out.push_str("//! GENERATED by `cargo run -p mascot-icons --bin mascot-icons-gen`.\n");
    out.push_str("//! Do not edit; regenerated from crates/mascot-icons/source/*.svg (see PROVENANCE.md).\n\n");
    out.push_str("use crate::Seg;\n");
    for (const_name, file) in icons {
        let path = source_dir.join(format!("{file}.svg"));
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let segs = parse_svg(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        if segs.is_empty() {
            return Err(format!("{}: no drawable elements", path.display()));
        }
        let _ = write!(out, "\npub(crate) static {const_name}: &[Seg] = &[");
        for (i, s) in segs.iter().enumerate() {
            if i % 3 == 0 {
                out.push_str("\n    ");
            }
            out.push_str(&fmt_seg(s));
            out.push_str(", ");
        }
        out.push_str("\n];\n");
    }
    Ok(out)
}
