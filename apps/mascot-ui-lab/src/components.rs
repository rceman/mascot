//! `components [--capture DIR] [--allow-dirty] [--references DIR]` — the
//! Tier A component gallery.
//!
//! Every cell is drawn by the *production* painters in
//! `mascot_ui_win32::components` (an `impl Painter` block) or by the whole
//! `App::render_offscreen` path for cells that need a real windowless
//! RichEdit (composer, response). Sheets are CPU-composited from the
//! painters' bitmap output — every component cell is a fixed box
//! (component rect + CELL_PAD), never ink-cropped, flattened over its
//! theme's surface colour.
//!
//! Theme and sizes sheets render at 100% (1 px = 1 DIP). The shadcn
//! comparison sheets render native cells at 200% — the references are
//! captured at DPR 2, so both sides are the same effective scale.
//!
//! Determinism: WARP device, fixed fixtures, no clock in the output other
//! than provenance timestamps written by the capture script.

use mascot_render_win32::image::RgbaImage;
use mascot_render_win32::renderer::{DeviceKind, Renderer};
use mascot_ui::component::{
    BadgeVariant, ButtonSize, ButtonVariant, ControlVisual, IconButtonKind, TextStyle,
    badge_colors, badge_size, button_colors, button_paint, button_size, icon_button_colors,
    icon_button_paint, tooltip_colors, tooltip_size,
};
use mascot_ui::geom::Rect;
use mascot_ui::layout::composer_height;
use mascot_ui::theme::{Palette, Theme, tokens};
use mascot_ui_win32::app::App;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::core::Interface;
use windows::core::w;
use windows_numerics::Vector2;

use crate::inventory::{COMPONENTS, Entry, Sizing, Status, by_key};
use crate::presets;

// sheet layout constants (device px)
const MARGIN: u32 = 24;
const SECTION_GAP: u32 = 30;
const ROW_GAP: u32 = 14;
const CELL_GAP: u32 = 16;
const CELL_PAD: f32 = 10.0; // DIP of room around a control cell (focus ring)
const MAX_SHEET_W: u32 = 2600;
const MAX_SHEET_H: u32 = 4000;
/// Bubble crop margin (keeps a slice of the drop shadow around the bubble).
const BUBBLE_CROP_PAD: f32 = 12.0;
/// Native cells on the comparison sheets render at this scale — the shadcn
/// references are DPR 2, so both sides sit at the same effective scale.
const REF_SCALE: f32 = 2.0;
/// Deviation bullet text wraps at this width in DIP (= ~1100 device px at 2x).
const DEV_WRAP_DIP: f32 = 550.0;
/// Safety cap before a ref image is integer-downscaled to fit a column.
const REF_MAX_W: u32 = 2000;

pub fn run() -> Result<(), String> {
    let capture_dir = crate::get_arg("--capture").map(PathBuf::from);
    let allow_dirty = crate::has_flag("--allow-dirty");
    let dirty = crate::capture::git_dirty();
    if capture_dir.is_some() && dirty && !allow_dirty {
        return Err("working tree is dirty; pass --allow-dirty for dev iterations".into());
    }

    let rig = crate::find_rig()?;
    let mut app = App::new(DeviceKind::Warp, rig, Default::default(), 1.0)
        .map_err(|e| format!("app: {e}"))?;
    app.response_delay_ms = 0;

    let mut rep = Report::default();
    let light = theme_sheet(&mut app, Theme::Light, &mut rep)?;
    let dark = theme_sheet(&mut app, Theme::Dark, &mut rep)?;
    let sizes = sizes_sheet(&mut app, &mut rep)?;
    let dpi = dpi_sheet(&mut app, &mut rep)?;

    let ref_dir = crate::get_arg("--references")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::capture::repo_root().join("tools/ui-reference/shadcn"));
    let have_refs = ref_dir.join("provenance.json").exists();

    if let Some(dir) = capture_dir {
        if !have_refs {
            return Err(format!(
                "shadcn reference captures missing: {} — run tools/ui-reference/capture-shadcn.ps1 or pass --references DIR",
                ref_dir.display()
            ));
        }
        let (ref1, ref2, ref3, ref4, ref5) = shadcn_sheets(&mut app, &ref_dir, &mut rep)?;
        let (motion, motion_json) = motion_sheet(&mut app, &ref_dir)?;

        for (n, s) in [
            ("component-gallery-light.png", &light),
            ("component-gallery-dark.png", &dark),
            ("component-gallery-sizes.png", &sizes),
            ("component-gallery-dpi.png", &dpi),
            ("component-gallery-shadcn-reference.png", &ref1),
            ("component-gallery-shadcn-reference-2.png", &ref2),
            ("component-gallery-shadcn-reference-3.png", &ref3),
            ("component-gallery-shadcn-reference-4.png", &ref4),
            ("component-gallery-shadcn-reference-5.png", &ref5),
        ] {
            rep.sheets.push((n.to_string(), s.width, s.height));
        }
        for (n, s) in &motion {
            rep.sheets.push((n.to_string(), s.width, s.height));
        }
        validate(&rep)?;
        write_capture(
            &dir,
            &app,
            Sheets {
                light,
                dark,
                sizes,
                dpi,
                motion,
                motion_json,
                ref1,
                ref2,
                ref3,
                ref4,
                ref5,
            },
            &rep,
            &ref_dir,
            dirty,
        )
    } else {
        // dev preview: write the sheets under target/ first, print paths,
        // then show the light sheet in a window
        let out = crate::capture::repo_root().join("target/mascot-ui-components");
        std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
        let mut saved = vec![
            ("component-gallery-light.png", &light),
            ("component-gallery-dark.png", &dark),
            ("component-gallery-sizes.png", &sizes),
            ("component-gallery-dpi.png", &dpi),
        ];
        if let Ok((ms, _)) = motion_sheet(&mut app, &ref_dir) {
            for (n, m) in &ms {
                let p = out.join(n);
                m.save(&p).map_err(|e| e.to_string())?;
                println!("wrote {}", p.display());
            }
        }
        let mut refs = Vec::new();
        if have_refs {
            let (r1, r2, r3, r4, r5) = shadcn_sheets(&mut app, &ref_dir, &mut rep)?;
            refs.push(("component-gallery-shadcn-reference.png", r1));
            refs.push(("component-gallery-shadcn-reference-2.png", r2));
            refs.push(("component-gallery-shadcn-reference-3.png", r3));
            refs.push(("component-gallery-shadcn-reference-4.png", r4));
            refs.push(("component-gallery-shadcn-reference-5.png", r5));
        }
        for (n, s) in &refs {
            saved.push((*n, s));
        }
        for (n, s) in saved {
            let p = out.join(n);
            s.save(&p).map_err(|e| e.to_string())?;
            println!("wrote {}", p.display());
        }
        preview(&mut app, &light)
    }
}

// ---------------------------------------------------------------- reporting

/// What the run produced — feeds self-validation and the receipt.
#[derive(Default)]
struct Report {
    /// (component key, theme) -> cells rendered on the theme sheets.
    theme_cells: BTreeMap<(String, String), usize>,
    /// component key -> the exact widths (DIP) the stretchable rows used.
    stretch_widths: BTreeMap<String, Vec<f32>>,
    /// rendered bubble widths (App::layout check).
    bubble_req_vs_got: Vec<(f32, f32)>,
    /// component key -> distinct content widths rendered.
    content_widths: BTreeMap<String, BTreeSet<u32>>,
    /// component key -> fixed sizes used.
    fixed_sizes: BTreeMap<String, Vec<f32>>,
    /// widest tooltip actually produced (must equal TOOLTIP_MAX_W).
    tooltip_max: f32,
    /// composer layout heights keyed by cell ("multiline", "overflow").
    composer_h: BTreeMap<String, f32>,
    /// overflow cell: last line bottom within one line height of the editor
    /// bottom edge.
    overflow_caret_in_editor: bool,
    /// component keys whose analogue refs actually landed on the sheet.
    ref_keys_used: BTreeSet<String>,
    /// ref PNG names used in comparison cells (receipt).
    ref_files: Vec<String>,
    /// measured native-cell fields per ref PNG (stem-theme.png -> fields).
    native_specs: BTreeMap<String, Vec<(String, String)>>,
    /// themeVars from provenance.json (for the comparison table).
    theme_vars: serde_json::Value,
    /// (file, w, h) of every sheet written.
    sheets: Vec<(String, u32, u32)>,
    /// caption rects that intersected a cell rect (must stay 0).
    caption_violations: u32,
    /// clamped tooltip cells whose text ink lacks the >= 10 DIP pill inset.
    tooltip_pad_violations: u32,
    /// (label, ref CSS px, native DIP) for pairs whose size must match.
    size_pairs: Vec<(String, f64, f64)>,
    /// surface cells that painted no visible drop shadow.
    surface_shadow_violations: u32,
}

fn report_theme_cell(rep: &mut Report, key: &str, theme: Theme) {
    *rep.theme_cells
        .entry((key.to_string(), theme.name().to_string()))
        .or_default() += 1;
}

/// Every self-validation rule from the gallery spec — fails the whole run.
fn validate(rep: &Report) -> Result<(), String> {
    let mut errs: Vec<String> = Vec::new();
    for e in COMPONENTS.iter().filter(|e| e.tier == 'A') {
        for t in Theme::ALL {
            let n = rep
                .theme_cells
                .get(&(e.key.to_string(), t.name().to_string()))
                .copied()
                .unwrap_or(0);
            if n == 0 {
                errs.push(format!("{} has no cell on the {} sheet", e.name, t.name()));
            }
        }
        match e.sizing {
            Sizing::Stretchable => {
                let got = rep.stretch_widths.get(e.key).cloned().unwrap_or_default();
                let want = [tokens::BUBBLE_W_MIN, tokens::BUBBLE_W, tokens::BUBBLE_W_MAX];
                for w in want {
                    if !got.iter().any(|g| (*g - w).abs() < 0.01) {
                        errs.push(format!("{} stretchable row missing width {w}", e.name));
                    }
                }
            }
            Sizing::ContentSized => {
                let n = rep.content_widths.get(e.key).map(|s| s.len()).unwrap_or(0);
                if n < 3 {
                    errs.push(format!(
                        "{} content-sized: {n} distinct widths, want >= 3",
                        e.name
                    ));
                }
            }
            Sizing::Fixed => {
                let sizes = rep.fixed_sizes.get(e.key).cloned().unwrap_or_default();
                if sizes.is_empty() {
                    errs.push(format!("{} fixed: no fixed-size cell rendered", e.name));
                }
            }
        }
        if e.shadcn.is_some() {
            if !rep.ref_keys_used.contains(e.key) {
                errs.push(format!("{}: no shadcn reference PNG landed", e.name));
            }
        } else if e.no_analogue_reason.is_empty() {
            errs.push(format!("{}: no shadcn analogue and no reason", e.name));
        }
    }
    for (req, got) in &rep.bubble_req_vs_got {
        if (req - got).abs() > 0.01 {
            errs.push(format!("layout().bubble.w {got} != requested {req}"));
        }
    }
    if (rep.tooltip_max - tokens::TOOLTIP_MAX_W).abs() > 0.01 {
        errs.push(format!(
            "long tooltip produced {:.1} DIP, want TOOLTIP_MAX_W {}",
            rep.tooltip_max,
            tokens::TOOLTIP_MAX_W
        ));
    }
    if let Some(h) = rep.composer_h.get("overflow")
        && (*h - tokens::COMPOSER_MAX_H).abs() > 0.5
    {
        errs.push(format!(
            "overflow composer h {h}, want COMPOSER_MAX_H {}",
            tokens::COMPOSER_MAX_H
        ));
    }
    if rep.composer_h.contains_key("overflow") && !rep.overflow_caret_in_editor {
        errs.push("overflow cell: last line bottom not anchored at the editor bottom".into());
    }
    if let Some(h) = rep.composer_h.get("multiline") {
        let want = composer_height(4.0 * tokens::BODY_LINE);
        if (*h - want).abs() > tokens::BODY_LINE / 2.0 {
            errs.push(format!(
                "multiline composer h {h}, want ~{want} (4 lines + insets)"
            ));
        }
    }
    for (f, w, h) in &rep.sheets {
        if *w > MAX_SHEET_W || *h > MAX_SHEET_H {
            errs.push(format!(
                "{f} is {w}x{h} — over {MAX_SHEET_W}x{MAX_SHEET_H} cap"
            ));
        }
    }
    for e in COMPONENTS.iter().filter(|e| e.tier != 'A') {
        if !matches!(e.status, Status::Planned | Status::Deferred) {
            errs.push(format!(
                "{}: tier {} has status {}",
                e.name,
                e.tier,
                e.status.name()
            ));
        }
    }
    if rep.caption_violations > 0 {
        errs.push(format!(
            "{} caption rect(s) intersect a cell rect",
            rep.caption_violations
        ));
    }
    if rep.tooltip_pad_violations > 0 {
        errs.push("clamped tooltip: text ink touches the pill edge (< 10 DIP inset)".into());
    }
    if rep.surface_shadow_violations > 0 {
        errs.push("surface cell: no drop shadow 4 DIP below the surface edge".into());
    }
    for (what, css_px, native_dip) in &rep.size_pairs {
        // native DIP rendered at REF_SCALE 2x: px = DIP * 2 must equal the
        // ref element rect px * 2 within 2 px (<= 1 DIP)
        if (native_dip - css_px).abs() > 1.0 {
            errs.push(format!(
                "{what}: native {native_dip:.1} DIP vs ref {css_px:.1} CSS px — must match"
            ));
        }
    }
    if errs.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "gallery self-validation failed:\n{}",
            errs.join("\n")
        ))
    }
}

// ------------------------------------------------------------ cell plumbing

fn cf0(c: [f32; 4]) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: c[0],
        g: c[1],
        b: c[2],
        a: c[3],
    }
}

/// Renders `draw` into a transparent `w`x`h` DIP bitmap at `scale`. Does NOT
/// crop — component cells are fixed boxes (component rect + CELL_PAD).
fn raw_img(
    r: &Renderer,
    w: f32,
    h: f32,
    scale: f32,
    draw: impl FnOnce(&ID2D1DeviceContext) -> windows::core::Result<()>,
) -> Result<RgbaImage, String> {
    let px = [
        (w * scale).ceil().max(1.0) as u32,
        (h * scale).ceil().max(1.0) as u32,
    ];
    let bmp = r.create_target_bitmap(px).map_err(|e| e.to_string())?;
    let ctx = &r.ctx;
    unsafe {
        ctx.SetTarget(&bmp.cast::<ID2D1Image>().map_err(|e| e.to_string())?);
        ctx.SetDpi(96.0 * scale, 96.0 * scale);
        ctx.BeginDraw();
        ctx.Clear(Some(&cf0([0.0, 0.0, 0.0, 0.0])));
        let res = draw(ctx);
        ctx.EndDraw(None, None).map_err(|e| e.to_string())?;
        ctx.SetTarget(None);
        ctx.SetDpi(96.0, 96.0);
        res.map_err(|e| e.to_string())?;
    }
    r.read_back(&bmp, px).map_err(|e| e.to_string())
}

/// A component cell: fixed box, flattened over the theme's surface colour so
/// transparent exteriors (shadows, unfilled ghosts) read correctly wherever
/// the cell lands.
fn cell_img(
    app: &App,
    pal: &Palette,
    w: f32,
    h: f32,
    scale: f32,
    draw: impl FnOnce(&ID2D1DeviceContext) -> windows::core::Result<()>,
) -> Result<RgbaImage, String> {
    let img = raw_img(&app.renderer, w, h, scale, draw)?;
    let s = rgb8(pal.surface);
    Ok(img.over(s))
}

/// Rightmost non-transparent pixel column + 1 (ink width in px).
fn ink_w(img: &RgbaImage) -> u32 {
    let mut last = 0u32;
    for y in 0..img.height {
        for x in (0..img.width).rev() {
            if img.data[((y * img.width + x) * 4 + 3) as usize] > 0 {
                last = last.max(x + 1);
                break;
            }
        }
    }
    last
}

/// Measured single-line text at `scale`, straight alpha (un-flattened).
/// NO_WRAP: the layout gets the measured width + slack so nothing ever
/// wraps/clips (the "defaul"/"hove" truncation bug). Self-check: rendered
/// ink width >= 0.9 * measured width. Used directly for captions (blended,
/// no painted band) or flattened via [`text_img`].
fn text_img_alpha(
    app: &App,
    text: &str,
    fmt: &IDWriteTextFormat,
    color: [f32; 4],
    scale: f32,
) -> Result<RgbaImage, String> {
    let fonts = &app.painter.fonts;
    let (tw, th) = fonts
        .measure(text, f32::MAX, fmt)
        .map_err(|e| e.to_string())?;
    let w_dip = tw.ceil() + 4.0;
    let h_dip = th.ceil() + 4.0;
    let wtext: Vec<u16> = text.encode_utf16().collect();
    let img = raw_img(&app.renderer, w_dip, h_dip, scale, |ctx| unsafe {
        let tl = fonts.dwrite.CreateTextLayout(&wtext, fmt, w_dip, h_dip)?;
        tl.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
        let brush = ctx.CreateSolidColorBrush(&cf0(color), None)?;
        ctx.DrawTextLayout(
            Vector2 { X: 1.0, Y: 1.0 },
            &tl,
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
        );
        Ok(())
    })?;
    let got = ink_w(&img) as f32;
    if got < tw * scale * 0.9 {
        return Err(format!(
            "caption '{text}' clipped: ink {got:.0}px < 0.9 x measured {:.0}px",
            tw * scale
        ));
    }
    Ok(img)
}

/// `text_img_alpha` flattened over `bg` (block images, headings).
fn text_img(
    app: &App,
    text: &str,
    fmt: &IDWriteTextFormat,
    color: [f32; 4],
    scale: f32,
    bg: [u8; 3],
) -> Result<RgbaImage, String> {
    Ok(text_img_alpha(app, text, fmt, color, scale)?.over(bg))
}

/// Wrapped text at a fixed DIP width (deviation bullets, typography cells).
fn text_img_wrap(
    app: &App,
    text: &str,
    w_dip: f32,
    fmt: &IDWriteTextFormat,
    color: [f32; 4],
    scale: f32,
    bg: [u8; 3],
) -> Result<RgbaImage, String> {
    let fonts = &app.painter.fonts;
    let wtext: Vec<u16> = text.encode_utf16().collect();
    let (_tw, th) = fonts.measure(text, w_dip, fmt).map_err(|e| e.to_string())?;
    let h_dip = th.ceil() + 4.0;
    let img = raw_img(&app.renderer, w_dip + 4.0, h_dip, scale, |ctx| unsafe {
        let tl = fonts.dwrite.CreateTextLayout(&wtext, fmt, w_dip, h_dip)?;
        let brush = ctx.CreateSolidColorBrush(&cf0(color), None)?;
        ctx.DrawTextLayout(
            Vector2 { X: 1.0, Y: 1.0 },
            &tl,
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
        );
        Ok(())
    })?;
    Ok(img.over(bg))
}

fn rgb8(c: [f32; 4]) -> [u8; 3] {
    [
        (c[0] * 255.0).round() as u8,
        (c[1] * 255.0).round() as u8,
        (c[2] * 255.0).round() as u8,
    ]
}

fn hex(c: [f32; 4]) -> String {
    let s = rgb8(c);
    let a = (c[3] * 255.0).round() as u8;
    if a < 255 {
        format!("#{:02X}{:02X}{:02X}{:02X}", s[0], s[1], s[2], a)
    } else {
        format!("#{:02X}{:02X}{:02X}", s[0], s[1], s[2])
    }
}

// ---------------------------------------------------------- sheet compositor

struct Cell {
    cap: String,
    img: RgbaImage,
}

enum Line {
    Gap(u32),
    /// Pre-rendered heading image.
    Heading(RgbaImage),
    /// A row of captioned cells; `sub` is a pre-rendered row label.
    Row {
        sub: String,
        cells: Vec<Cell>,
    },
    /// Same as Row but captions align to the cell's left edge.
    RowLeft {
        sub: String,
        cells: Vec<Cell>,
    },
    /// A standalone pre-rendered image line (deviation bullets).
    Block(RgbaImage),
}

/// Stacks lines vertically. `text_scale` multiplies the caption/label DPI —
/// 1.0 on the 100% sheets, 2.0 on the comparison sheet so labels stay
/// readable next to DPR-2 references. Captions are alpha text (no painted
/// band) placed strictly below the cell box with a `4 * text_scale` px gap;
/// each placement is recorded in `rep.caption_violations` if a caption rect
/// intersects its cell rect.
fn compose(
    app: &App,
    rep: &mut Report,
    lines: &[Line],
    bg: [u8; 4],
    muted: [f32; 4],
    text_scale: f32,
) -> Result<RgbaImage, String> {
    let fonts = &app.painter.fonts;
    let cap_gap = (4.0 * text_scale).ceil() as u32;
    struct PrRow {
        sub: Option<RgbaImage>,
        left: bool,
        cells: Vec<(RgbaImage, RgbaImage, u32)>, // img, cap, slot w
    }
    enum Pr {
        Blank(u32),
        Img(RgbaImage),
        R(PrRow),
    }
    let mut pr: Vec<Pr> = Vec::new();
    let mut total_w = 0u32;
    for line in lines {
        match line {
            Line::Gap(h) => pr.push(Pr::Blank(*h)),
            Line::Heading(img) | Line::Block(img) => {
                total_w = total_w.max(img.width);
                pr.push(Pr::Img(img.clone()));
            }
            Line::Row { sub, cells } | Line::RowLeft { sub, cells } => {
                let left = matches!(line, Line::RowLeft { .. });
                let sub_img = if sub.is_empty() {
                    None
                } else {
                    Some(text_img_alpha(
                        app,
                        sub,
                        &fonts.caption.clone(),
                        muted,
                        text_scale,
                    )?)
                };
                let sub_w = sub_img.as_ref().map(|i| i.width + 12).unwrap_or(0);
                // wrap cells into continuation visual rows at the sheet cap
                let wrap_w = MAX_SHEET_W - 2 * MARGIN;
                let mut prc: Vec<(RgbaImage, RgbaImage, u32)> = Vec::new();
                let mut row_w = sub_w;
                let mut first = true;
                for c in cells {
                    let cap =
                        text_img_alpha(app, &c.cap, &fonts.caption.clone(), muted, text_scale)?;
                    let slot = c.img.width.max(cap.width);
                    if row_w + slot + CELL_GAP > wrap_w && !prc.is_empty() {
                        pr.push(Pr::R(PrRow {
                            sub: if first { sub_img.clone() } else { None },
                            left,
                            cells: std::mem::take(&mut prc),
                        }));
                        first = false;
                        row_w = 0;
                    }
                    row_w += slot + CELL_GAP;
                    total_w = total_w.max(row_w);
                    prc.push((c.img.crop(0, 0, c.img.width, c.img.height), cap, slot));
                }
                if !prc.is_empty() {
                    pr.push(Pr::R(PrRow {
                        sub: if first { sub_img.clone() } else { None },
                        left,
                        cells: prc,
                    }));
                }
            }
        }
    }
    // heights
    let mut total_h = 2 * MARGIN;
    let mut row_heights: Vec<u32> = Vec::new();
    for it in &pr {
        match it {
            Pr::Blank(h) => {
                row_heights.push(0);
                total_h += h;
            }
            Pr::Img(img) => {
                row_heights.push(0);
                total_h += img.height + 6;
            }
            Pr::R(row) => {
                let h = row
                    .cells
                    .iter()
                    .map(|(i, c, _)| i.height + cap_gap + c.height)
                    .max()
                    .unwrap_or(0);
                row_heights.push(h);
                total_h += h + ROW_GAP;
            }
        }
    }
    let sheet_w = (total_w + 2 * MARGIN).max(200);
    let mut sheet = RgbaImage::new(sheet_w, total_h);
    sheet.fill_rect(0, 0, sheet_w, total_h, bg);

    let mut y = MARGIN;
    for (i, it) in pr.iter().enumerate() {
        match it {
            Pr::Blank(h) => y += h,
            Pr::Img(img) => {
                sheet.blit(img, MARGIN, y + 3);
                y += img.height + 6;
            }
            Pr::R(row) => {
                let rh = row_heights[i];
                let cell_h = row
                    .cells
                    .iter()
                    .map(|(i, _, _)| i.height)
                    .max()
                    .unwrap_or(0);
                let mut x = MARGIN;
                if let Some(s) = &row.sub {
                    sheet.blend_over(s, x, y + cell_h.saturating_sub(s.height) / 2);
                    x += s.width + 12;
                }
                for (img, cap, slot) in &row.cells {
                    let cx = x + (slot - img.width) / 2;
                    let cy = y + (cell_h - img.height) / 2;
                    // cells are opaque (flattened over their theme surface)
                    sheet.blit(img, cx, cy);
                    // caption strictly below the cell box — alpha text only
                    let cap_y = y + cell_h + cap_gap;
                    let cap_x = if row.left {
                        x
                    } else {
                        x + (slot - cap.width) / 2
                    };
                    // self-check: caption rect must not intersect the cell rect
                    if cap_y < cy + img.height {
                        rep.caption_violations += 1;
                    }
                    sheet.blend_over(cap, cap_x, cap_y);
                    x += slot + CELL_GAP;
                }
                y += rh + ROW_GAP;
            }
        }
    }
    Ok(sheet)
}

fn heading_img(
    app: &App,
    text: &str,
    scale: f32,
    bg: [u8; 3],
    fg: [f32; 4],
) -> Result<RgbaImage, String> {
    let fmt = app.painter.fonts.label.clone();
    text_img(app, text, &fmt, fg, scale, bg)
}

// ------------------------------------------------------------- cell builders

fn icon_button_cell(
    app: &App,
    pal: &Palette,
    kind: IconButtonKind,
    icon: mascot_icons::Icon,
    v: ControlVisual,
    scale: f32,
) -> Result<RgbaImage, String> {
    let e = kind.edge();
    let (p, r) = (&app.painter, &app.renderer);
    let _ = r;
    cell_img(
        app,
        pal,
        e + 2.0 * CELL_PAD,
        e + 2.0 * CELL_PAD,
        scale,
        |ctx| {
            p.icon_button(
                ctx,
                pal,
                Rect::new(CELL_PAD, CELL_PAD, e, e),
                icon,
                icon_button_paint(pal, kind, v),
            )
        },
    )
}

fn button_cell(
    app: &App,
    pal: &Palette,
    label: &str,
    variant: ButtonVariant,
    size: ButtonSize,
    v: ControlVisual,
    scale: f32,
) -> Result<RgbaImage, String> {
    let tw = app.painter.text_width(label, TextStyle::Label);
    let s = button_size(tw, size);
    let p = &app.painter;
    cell_img(
        app,
        pal,
        s.w + 2.0 * CELL_PAD,
        s.h + 2.0 * CELL_PAD,
        scale,
        |ctx| {
            p.button(
                ctx,
                pal,
                Rect::new(CELL_PAD, CELL_PAD, s.w, s.h),
                label,
                button_paint(pal, variant, v),
            )
        },
    )
}

fn badge_cell(
    app: &App,
    pal: &Palette,
    label: &str,
    variant: BadgeVariant,
    scale: f32,
) -> Result<RgbaImage, String> {
    let tw = app.painter.text_width(label, TextStyle::Caption);
    let s = badge_size(tw);
    let p = &app.painter;
    cell_img(app, pal, s.w + 8.0, s.h + 8.0, scale, |ctx| {
        p.badge(ctx, pal, Rect::new(4.0, 4.0, s.w, s.h), label, variant)
    })
}

fn tooltip_cell(
    app: &App,
    pal: &Palette,
    text: &str,
    scale: f32,
) -> Result<(RgbaImage, f32), String> {
    let tw = app.painter.text_width(text, TextStyle::Muted);
    let s = tooltip_size(tw);
    let p = &app.painter;
    let img = cell_img(app, pal, s.w + 4.0, s.h + 4.0, scale, |ctx| {
        p.tooltip(ctx, pal, Rect::new(2.0, 2.0, s.w, s.h), text)
    })?;
    Ok((img, s.w))
}

/// Surface cell WITH the production drop shadow: `prepare` builds the
/// painter's shadow bitmap for this cell's pixel size, `shadow` draws it,
/// then `surface` paints the bubble on top. The box is padded 12 DIP on the
/// sides and 20 DIP below so the shadow (sigma 4 + 4 DIP y-offset) stays
/// inside the cell. The cached shadow is cleared afterwards so later App
/// cells re-prepare their own.
fn surface_cell(
    app: &mut App,
    pal: &Palette,
    w: f32,
    h: f32,
    focused: bool,
    scale: f32,
) -> Result<RgbaImage, String> {
    let pad = 12.0f32;
    let cell_w = w + 2.0 * pad;
    let cell_h = h + 2.0 * pad + 8.0;
    let surf = Rect::new(pad, pad, w, h);
    let cell_px = [
        (cell_w * scale).round() as u32,
        (cell_h * scale).round() as u32,
    ];
    app.painter
        .prepare(
            &app.renderer.ctx,
            pal,
            Some(surf),
            tokens::RADIUS_XL,
            scale,
            cell_px,
        )
        .map_err(|e| e.to_string())?;
    let img = {
        let p = &app.painter;
        cell_img(app, pal, cell_w, cell_h, scale, |ctx| {
            p.shadow(ctx, Rect::new(0.0, 0.0, cell_w, cell_h))?;
            p.surface(ctx, pal, surf, tokens::RADIUS_XL, scale, focused)
        })?
    };
    app.painter
        .prepare(&app.renderer.ctx, pal, None, 0.0, 1.0, [0, 0])
        .map_err(|e| e.to_string())?;
    Ok(img)
}

fn separator_cell(app: &App, pal: &Palette, w: f32, scale: f32) -> Result<RgbaImage, String> {
    let p = &app.painter;
    cell_img(app, pal, w, 4.0, scale, |ctx| {
        p.separator(ctx, pal, Rect::new(0.0, 1.0, w, tokens::SEPARATOR_H))
    })
}

fn icon_cell(
    app: &App,
    pal: &Palette,
    icon: mascot_icons::Icon,
    scale: f32,
) -> Result<RgbaImage, String> {
    let p = &app.painter;
    cell_img(
        app,
        pal,
        tokens::ICON_SIZE + 4.0,
        tokens::ICON_SIZE + 4.0,
        scale,
        |ctx| {
            p.icon(
                ctx,
                icon,
                Rect::new(2.0, 2.0, tokens::ICON_SIZE, tokens::ICON_SIZE),
                pal.foreground,
            )
        },
    )
}

/// A whole-App cell (composer/response need the real RichEdit). The mascot
/// sprite is removed for component cells and restored afterwards — the
/// gallery shows the bubble, not the mascot. Crops to the layout bubble ∪
/// tooltip, flattens over the theme surface.
fn app_cell(
    app: &mut App,
    rep: &mut Report,
    theme: Theme,
    stage: impl FnOnce(&mut App),
    bubble_w: f32,
) -> Result<RgbaImage, String> {
    stage(app);
    app.set_bubble_w(bubble_w);
    app.state.theme = theme;
    app.apply_theme();
    let sprite = app.painter.sprite.take();
    let res = app.render_offscreen();
    app.painter.sprite = sprite;
    let img = res.map_err(|e| format!("app cell: {e}"))?;
    if let Some(b) = app.layout.bubble {
        rep.bubble_req_vs_got.push((bubble_w, b.w));
    }
    let lay = &app.layout;
    let mut rr = lay.bubble.unwrap_or(lay.mascot);
    if let Some(t) = lay.tooltip {
        rr = rr.union(t);
    }
    let rr = rr.grow(BUBBLE_CROP_PAD);
    let s = app.scale;
    let x = rr.x.max(0.0).min(lay.window.right());
    let y = rr.y.max(0.0).min(lay.window.bottom());
    let w = (rr.right().min(lay.window.right()) - x).max(1.0);
    let h = (rr.bottom().min(lay.window.bottom()) - y).max(1.0);
    let img = img.crop(
        (x * s).round() as u32,
        (y * s).round() as u32,
        (w * s).round() as u32,
        (h * s).round() as u32,
    );
    let pal = theme.palette();
    Ok(img.over(rgb8(pal.surface)))
}

/// Composer staging with a real re-measure: text changes must go through
/// `process_editor_events` + relayout, not a bare `set_text` (stale height).
fn composer_stage(a: &mut App, text: &str, focused: bool) {
    presets::apply(a, "composer-empty", false);
    if !text.is_empty() {
        let _ = a.editor.set_text(text);
        a.state.editor_empty = false;
    }
    if focused {
        a.focus_editor();
        let len = a.editor.text().encode_utf16().count() as i32;
        a.editor.set_selection(len, len);
    }
    a.process_editor_events();
    let _ = a.relayout();
    // scroll to the end: EM_SCROLLCARET is a no-op for the windowless host
    // (caret is a -32000 sentinel) and EM_SETSCROLLPOS is ignored outright —
    // EM_LINESCROLL (the path mouse-wheel scrolling takes) is what moves the
    // viewport. Lines needed = ceil((last-line bottom - client bottom) /
    // line_h), measured from real char positions.
    a.editor.send(EM_SCROLLCARET, 0, 0);
    let text16: Vec<u16> = a.editor.text().encode_utf16().collect();
    let mut pt = windows::Win32::Foundation::POINTL { x: 0, y: 0 };
    a.editor.send(
        EM_POSFROMCHAR,
        &mut pt as *mut _ as usize,
        text16.len() as isize - 1,
    );
    // last line's real height: y(last-line start) - y(prev-line start);
    // line starts come from EM_LINEINDEX (richedit uses CR breaks, not '\n')
    let last_line = a.editor.send(EM_LINEFROMCHAR, text16.len() - 1, 0);
    if last_line < 1 {
        panic!("overflow composer: expected >= 2 lines to scroll");
    }
    let n0 = a.editor.send(EM_LINEINDEX, last_line as usize - 1, 0);
    let n1 = a.editor.send(EM_LINEINDEX, last_line as usize, 0);
    let mut p0 = windows::Win32::Foundation::POINTL { x: 0, y: 0 };
    let mut p1 = windows::Win32::Foundation::POINTL { x: 0, y: 0 };
    a.editor
        .send(EM_POSFROMCHAR, &mut p0 as *mut _ as usize, n0);
    a.editor
        .send(EM_POSFROMCHAR, &mut p1 as *mut _ as usize, n1);
    let line_h = (p1.y - p0.y).max(1);
    let client_h = a
        .layout
        .editor
        .map(|ed| (ed.h * a.scale).round() as i32)
        .unwrap_or(0);
    if pt.y + line_h > client_h {
        let lines_to_scroll = ((pt.y + line_h) - client_h + line_h - 1) / line_h;
        a.editor.send(EM_LINESCROLL, 0, lines_to_scroll as isize);
    }
    a.dirty = true;
}

/// `EM_SCROLLCARET` — asks RichEdit to make the caret line visible.
const EM_SCROLLCARET: u32 = 0x00B7;
/// `EM_POSFROMCHAR` — client-space position of a char (wParam=&POINTL,
/// lParam=char index).
const EM_POSFROMCHAR: u32 = 0x00D6;
/// `EM_LINESCROLL` — scrolls vertically by lines (lParam).
const EM_LINESCROLL: u32 = 0x00B6;
/// `EM_LINEINDEX` — char index of a line's start (wParam = line).
const EM_LINEINDEX: u32 = 0x00BB;
/// `EM_LINEFROMCHAR` — line index of a char (wParam = char index).
const EM_LINEFROMCHAR: u32 = 0x00C9;

// ------------------------------------------------------------- theme sheets

fn theme_sheet(app: &mut App, theme: Theme, rep: &mut Report) -> Result<RgbaImage, String> {
    let pal = theme.palette();
    let bg3 = rgb8(pal.surface);
    let vis = |hover: bool, pressed: bool, focus: bool, dis: bool| ControlVisual {
        hover,
        pressed,
        focus_visible: focus,
        disabled: dis,
    };
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::Heading(heading_img(
        app,
        &format!(
            "{} — Tier A vocabulary — 100 % (1 px = 1 DIP)",
            theme.name()
        ),
        1.0,
        bg3,
        pal.foreground,
    )?));

    // ---- Surface -------------------------------------------------------
    lines.push(Line::Gap(SECTION_GAP));
    lines.push(Line::Heading(heading_img(
        app,
        "Surface / Bubble — radius xl",
        1.0,
        bg3,
        pal.foreground,
    )?));
    {
        let plain = surface_cell(app, &pal, 220.0, 56.0, false, 1.0)?;
        if theme == Theme::Light {
            // self-check: the production shadow must be painted — the pixel
            // 4 DIP below the surface bottom edge (horizontal centre) is
            // darker than the band surface
            let pad = 12.0f32;
            let x = (pad + 110.0) as u32;
            let y = (pad + 56.0 + 4.0) as u32;
            let i = ((y * plain.width + x) * 4) as usize;
            let px = &plain.data[i..i + 3];
            let s = rgb8(pal.surface);
            let darker = px
                .iter()
                .zip(s.iter())
                .any(|(a, b)| *b as i32 - *a as i32 > 8);
            if !darker {
                rep.surface_shadow_violations += 1;
            }
        }
        lines.push(Line::Row {
            sub: String::new(),
            cells: vec![
                Cell {
                    cap: "plain".into(),
                    img: plain,
                },
                Cell {
                    cap: "focused border".into(),
                    img: surface_cell(app, &pal, 220.0, 56.0, true, 1.0)?,
                },
            ],
        });
        report_theme_cell(rep, "surface", theme);
        report_theme_cell(rep, "surface", theme);
    }

    // ---- Typography ----------------------------------------------------
    lines.push(Line::Gap(SECTION_GAP));
    lines.push(Line::Heading(heading_img(
        app,
        "Typography / Label",
        1.0,
        bg3,
        pal.foreground,
    )?));
    {
        let mut cells = Vec::new();
        for (name, text, style) in [
            ("body 14/400", "Refactor the rig loader", TextStyle::Body),
            ("muted 12/400", "Mascot · idle", TextStyle::Muted),
            ("label 14/500", "Apply changes", TextStyle::Label),
            ("caption 12/500", "Waiting for provider", TextStyle::Caption),
        ] {
            // cell hugs the text: measured width + CELL_PAD, left-aligned
            let tw = app.painter.text_width(text, style);
            rep.content_widths
                .entry("typography".into())
                .or_default()
                .insert(tw.round() as u32);
            let fmt = app.painter.fonts.format(style).clone();
            let color = match style {
                TextStyle::Muted => pal.muted_fg,
                _ => pal.foreground,
            };
            let img = text_img_wrap(app, text, tw + CELL_PAD, &fmt, color, 1.0, bg3)?;
            cells.push(Cell {
                cap: name.to_string(),
                img,
            });
            report_theme_cell(rep, "typography", theme);
        }
        let (tip, _w) = tooltip_cell(app, &pal, "Send", 1.0)?;
        cells.push(Cell {
            cap: "tooltip text 12/400".into(),
            img: tip,
        });
        report_theme_cell(rep, "typography", theme);
        lines.push(Line::RowLeft {
            sub: String::new(),
            cells,
        });
    }

    // ---- Icon ----------------------------------------------------------
    lines.push(Line::Gap(SECTION_GAP));
    lines.push(Line::Heading(heading_img(
        app,
        "Icon — Lucide geometry, 16 DIP",
        1.0,
        bg3,
        pal.foreground,
    )?));
    {
        let mut cells = Vec::new();
        for ic in mascot_icons::Icon::ALL {
            cells.push(Cell {
                cap: ic.name().to_string(),
                img: icon_cell(app, &pal, *ic, 1.0)?,
            });
            report_theme_cell(rep, "icon", theme);
        }
        lines.push(Line::Row {
            sub: String::new(),
            cells,
        });
    }

    // ---- IconButton --------------------------------------------------
    lines.push(Line::Gap(SECTION_GAP));
    lines.push(Line::Heading(heading_img(
        app,
        "IconButton — fixed 32/28 DIP edge",
        1.0,
        bg3,
        pal.foreground,
    )?));
    for (sub, icon, kind, states) in [
        (
            "primary · Send (32)",
            mascot_icons::Icon::ArrowUp,
            IconButtonKind::Primary,
            vec![
                ("default", vis(false, false, false, false)),
                ("hover", vis(true, false, false, false)),
                ("pressed", vis(false, true, false, false)),
                ("focus-visible", vis(false, false, true, false)),
                ("disabled", vis(false, false, false, true)),
            ],
        ),
        (
            "primary · Stop (32)",
            mascot_icons::Icon::Square,
            IconButtonKind::Primary,
            vec![
                ("default", vis(false, false, false, false)),
                ("hover", vis(true, false, false, false)),
            ],
        ),
        (
            "ghost · Copy (28)",
            mascot_icons::Icon::Copy,
            IconButtonKind::Ghost,
            vec![
                ("default", vis(false, false, false, false)),
                ("hover", vis(true, false, false, false)),
                ("pressed", vis(false, true, false, false)),
                ("focus-visible", vis(false, false, true, false)),
            ],
        ),
        (
            "ghost · Copy (28) copied",
            mascot_icons::Icon::Check,
            IconButtonKind::Ghost,
            vec![("copied (Check)", vis(false, false, false, false))],
        ),
    ] {
        let mut cells = Vec::new();
        for (cap, v) in states {
            cells.push(Cell {
                cap: cap.to_string(),
                img: icon_button_cell(app, &pal, kind, icon, v, 1.0)?,
            });
            report_theme_cell(rep, "icon-button", theme);
        }
        lines.push(Line::Row {
            sub: sub.into(),
            cells,
        });
    }

    // ---- Button (Sm) ---------------------------------------------------
    lines.push(Line::Gap(SECTION_GAP));
    lines.push(Line::Heading(heading_img(
        app,
        "Button — Sm (h-8), content-sized",
        1.0,
        bg3,
        pal.foreground,
    )?));
    for (name, variant) in [
        ("Default", ButtonVariant::Default),
        ("Secondary", ButtonVariant::Secondary),
        ("Ghost", ButtonVariant::Ghost),
    ] {
        let mut cells = Vec::new();
        for (cap, v) in [
            ("default", vis(false, false, false, false)),
            ("hover", vis(true, false, false, false)),
            ("pressed", vis(false, true, false, false)),
            ("focus-visible", vis(false, false, true, false)),
            ("disabled", vis(false, false, false, true)),
        ] {
            cells.push(Cell {
                cap: cap.to_string(),
                img: button_cell(app, &pal, "Button", variant, ButtonSize::Sm, v, 1.0)?,
            });
            report_theme_cell(rep, "button", theme);
        }
        lines.push(Line::Row {
            sub: name.into(),
            cells,
        });
    }

    // ---- Tooltip -------------------------------------------------------
    lines.push(Line::Gap(SECTION_GAP));
    lines.push(Line::Heading(heading_img(
        app,
        "Tooltip — single line, bounded",
        1.0,
        bg3,
        pal.foreground,
    )?));
    {
        let mut cells = Vec::new();
        for cap in ["Send", "Copy", "Copied"] {
            let (img, _w) = tooltip_cell(app, &pal, cap, 1.0)?;
            cells.push(Cell {
                cap: format!("\"{cap}\""),
                img,
            });
            report_theme_cell(rep, "tooltip", theme);
        }
        lines.push(Line::Row {
            sub: String::new(),
            cells,
        });
    }

    // ---- Composer / Response — real App frames -------------------------
    lines.push(Line::Gap(SECTION_GAP));
    lines.push(Line::Heading(heading_img(
        app,
        "Native Text Input / Composer — default 380 DIP",
        1.0,
        bg3,
        pal.foreground,
    )?));
    {
        let stages: Vec<Stage> = vec![
            (
                "empty / placeholder",
                Box::new(|a| {
                    presets::apply(a, "composer-empty", false);
                }),
            ),
            (
                "focused",
                Box::new(|a| {
                    presets::apply(a, "composer-focused", false);
                }),
            ),
            (
                "text",
                Box::new(|a| {
                    presets::apply(a, "composer-text", false);
                }),
            ),
            (
                "selection",
                Box::new(|a| {
                    presets::apply(a, "composer-selection", false);
                }),
            ),
            (
                "multiline (4)",
                Box::new(|a| {
                    presets::apply(a, "composer-multiline", false);
                }),
            ),
            (
                "overflow (9→6)",
                Box::new(|a| {
                    composer_stage(
                        a,
                        "line one of the request\nline two\nline three\nline four\nline five\nline six\nline seven\nline eight\nline nine",
                        true,
                    )
                }),
            ),
            (
                "submitting",
                Box::new(|a| {
                    presets::apply(a, "submitting", false);
                }),
            ),
        ];
        let mut cells = Vec::new();
        for (cap, stage) in stages {
            let img = app_cell(app, rep, theme, |a| stage(a), tokens::BUBBLE_W)?;
            if let Some(c) = app.layout.composer {
                rep.composer_h
                    .insert(cap.split(' ').next().unwrap_or("").to_string(), c.h);
            }
            cells.push(Cell {
                cap: cap.to_string(),
                img,
            });
            report_theme_cell(rep, "composer", theme);
            if cap.starts_with("overflow") {
                // caret() returns the -32000 sentinel in the windowless host;
                // the visible check is the last LINE's client-space bottom —
                // it must sit within one line height of the editor bottom
                // (client_h - line_h <= bottom <= client_h).
                let text16: Vec<u16> = app.editor.text().encode_utf16().collect();
                let mut pt = windows::Win32::Foundation::POINTL { x: 0, y: 0 };
                app.editor.send(
                    EM_POSFROMCHAR,
                    &mut pt as *mut _ as usize,
                    text16.len() as isize - 1,
                );
                let last_line = app.editor.send(EM_LINEFROMCHAR, text16.len() - 1, 0);
                if last_line < 1 {
                    rep.overflow_caret_in_editor = false;
                    return Err("overflow cell: cannot measure line height".into());
                }
                let n0 = app.editor.send(EM_LINEINDEX, last_line as usize - 1, 0);
                let n1 = app.editor.send(EM_LINEINDEX, last_line as usize, 0);
                let mut p0 = windows::Win32::Foundation::POINTL { x: 0, y: 0 };
                let mut p1 = windows::Win32::Foundation::POINTL { x: 0, y: 0 };
                app.editor
                    .send(EM_POSFROMCHAR, &mut p0 as *mut _ as usize, n0);
                app.editor
                    .send(EM_POSFROMCHAR, &mut p1 as *mut _ as usize, n1);
                let line_h = (p1.y - p0.y).max(1);
                rep.overflow_caret_in_editor = app
                    .layout
                    .editor
                    .map(|ed| {
                        let client_h = (ed.h * app.scale).round() as i32;
                        let bottom = pt.y + line_h;
                        client_h - line_h <= bottom && bottom <= client_h
                    })
                    .unwrap_or(false);
            }
        }
        lines.push(Line::Row {
            sub: String::new(),
            cells,
        });
    }
    // ---- Separator -----------------------------------------------------
    lines.push(Line::Gap(SECTION_GAP));
    lines.push(Line::Heading(heading_img(
        app,
        "Separator",
        1.0,
        bg3,
        pal.foreground,
    )?));
    {
        report_theme_cell(rep, "separator", theme);
        lines.push(Line::Row {
            sub: String::new(),
            cells: vec![Cell {
                cap: "240 DIP hairline".into(),
                img: separator_cell(app, &pal, 240.0, 1.0)?,
            }],
        });
    }

    // ---- Response ----------------------------------------------------
    lines.push(Line::Gap(SECTION_GAP));
    lines.push(Line::Heading(heading_img(
        app,
        "Response / Content Surface — default 380 DIP",
        1.0,
        bg3,
        pal.foreground,
    )?));
    {
        let stages: Vec<Stage> = vec![
            (
                "default",
                Box::new(|a| {
                    presets::apply(a, "response", false);
                }),
            ),
            (
                "copied",
                Box::new(|a| {
                    presets::apply(a, "copy-copied", false);
                }),
            ),
            (
                "follow-up text",
                Box::new(|a| {
                    presets::apply(a, "response-followup", false);
                }),
            ),
        ];
        let mut cells = Vec::new();
        for (cap, stage) in stages {
            let img = app_cell(app, rep, theme, |a| stage(a), tokens::BUBBLE_W)?;
            cells.push(Cell {
                cap: cap.to_string(),
                img,
            });
            report_theme_cell(rep, "response", theme);
        }
        lines.push(Line::Row {
            sub: String::new(),
            cells,
        });
    }

    // ---- Badge ---------------------------------------------------------
    lines.push(Line::Gap(SECTION_GAP));
    lines.push(Line::Heading(heading_img(
        app,
        "Badge / Status Pill — content-sized, neutral",
        1.0,
        bg3,
        pal.foreground,
    )?));
    {
        let mut cells = Vec::new();
        for (name, variant) in [
            ("default", BadgeVariant::Default),
            ("secondary", BadgeVariant::Secondary),
            ("outline", BadgeVariant::Outline),
        ] {
            cells.push(Cell {
                cap: name.to_string(),
                img: badge_cell(app, &pal, "Idle", variant, 1.0)?,
            });
            report_theme_cell(rep, "badge", theme);
        }
        lines.push(Line::Row {
            sub: String::new(),
            cells,
        });
    }

    compose(
        app,
        rep,
        &lines,
        [
            (pal.surface[0] * 255.0) as u8,
            (pal.surface[1] * 255.0) as u8,
            (pal.surface[2] * 255.0) as u8,
            255,
        ],
        pal.muted_fg,
        1.0,
    )
}

/// A named staging closure applied to the App before `render_offscreen`.
type Stage = (&'static str, Box<dyn Fn(&mut App)>);

// ------------------------------------------------------------- sizes sheet
// section order = inventory order (same as the theme sheets)

fn sizes_sheet(app: &mut App, rep: &mut Report) -> Result<RgbaImage, String> {
    let theme = Theme::Light;
    let pal = theme.palette();
    let bg3 = rgb8(pal.surface);
    let ws = [tokens::BUBBLE_W_MIN, tokens::BUBBLE_W, tokens::BUBBLE_W_MAX];
    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::Heading(heading_img(
        app,
        "Sizes — 100 % (1 px = 1 DIP) — light",
        1.0,
        bg3,
        pal.foreground,
    )?));

    let section = |lines: &mut Vec<Line>, title: &str, app: &App| -> Result<(), String> {
        lines.push(Line::Gap(SECTION_GAP));
        lines.push(Line::Heading(heading_img(
            app,
            title,
            1.0,
            bg3,
            pal.foreground,
        )?));
        Ok(())
    };

    // ---- Surface / Bubble — stretchable 320/380/440
    section(&mut lines, "Surface / Bubble — stretchable", app)?;
    {
        let mut cells = Vec::new();
        for w in ws {
            rep.stretch_widths
                .entry("surface".into())
                .or_default()
                .push(w);
            cells.push(Cell {
                cap: format!("{w:.0} DIP"),
                img: surface_cell(app, &pal, w, 56.0, false, 1.0)?,
            });
        }
        lines.push(Line::Row {
            sub: String::new(),
            cells,
        });
    }

    // ---- Typography / Label — content-sized
    section(&mut lines, "Typography / Label — content-sized", app)?;
    {
        let mut cells = Vec::new();
        for (text, style) in [
            ("OK", TextStyle::Label),
            ("Refactor the rig loader", TextStyle::Body),
            (
                "The quick brown fox jumps over the lazy dog",
                TextStyle::Body,
            ),
        ] {
            let tw = app.painter.text_width(text, style);
            rep.content_widths
                .entry("typography".into())
                .or_default()
                .insert(tw.round() as u32);
            let fmt = app.painter.fonts.format(style).clone();
            let img = text_img_wrap(app, text, tw + CELL_PAD, &fmt, pal.foreground, 1.0, bg3)?;
            cells.push(Cell {
                cap: format!("{tw:.0} DIP"),
                img,
            });
        }
        lines.push(Line::RowLeft {
            sub: String::new(),
            cells,
        });
    }

    // ---- Icon — fixed
    section(&mut lines, "Icon — fixed", app)?;
    {
        rep.fixed_sizes
            .entry("icon".into())
            .or_default()
            .push(tokens::ICON_SIZE);
        lines.push(Line::Row {
            sub: String::new(),
            cells: vec![Cell {
                cap: format!("{} DIP — fixed — single supported size", tokens::ICON_SIZE),
                img: icon_cell(app, &pal, mascot_icons::Icon::ArrowUp, 1.0)?,
            }],
        });
    }

    // ---- IconButton — fixed
    section(&mut lines, "IconButton — fixed", app)?;
    {
        rep.fixed_sizes
            .entry("icon-button".into())
            .or_default()
            .extend([tokens::PRIMARY_BUTTON, tokens::GHOST_BUTTON]);
        lines.push(Line::Row {
            sub: String::new(),
            cells: vec![
                Cell {
                    cap: format!("primary {} DIP — fixed", tokens::PRIMARY_BUTTON),
                    img: icon_button_cell(
                        app,
                        &pal,
                        IconButtonKind::Primary,
                        mascot_icons::Icon::ArrowUp,
                        ControlVisual::default(),
                        1.0,
                    )?,
                },
                Cell {
                    cap: format!("ghost {} DIP — fixed", tokens::GHOST_BUTTON),
                    img: icon_button_cell(
                        app,
                        &pal,
                        IconButtonKind::Ghost,
                        mascot_icons::Icon::Copy,
                        ControlVisual::default(),
                        1.0,
                    )?,
                },
            ],
        });
    }

    // ---- Button — content-sized
    section(&mut lines, "Button — content-sized", app)?;
    for (size, sub) in [
        (ButtonSize::Sm, "Sm (32)"),
        (ButtonSize::Default, "Default (36)"),
    ] {
        let mut cells = Vec::new();
        for label in ["OK", "Try again", "Open settings folder"] {
            let tw = app.painter.text_width(label, TextStyle::Label);
            let sz = button_size(tw, size);
            rep.content_widths
                .entry("button".into())
                .or_default()
                .insert(tw.round() as u32);
            cells.push(Cell {
                cap: format!("\"{label}\" · {:.0} DIP", sz.w),
                img: button_cell(
                    app,
                    &pal,
                    label,
                    ButtonVariant::Default,
                    size,
                    ControlVisual::default(),
                    1.0,
                )?,
            });
        }
        lines.push(Line::Row {
            sub: sub.into(),
            cells,
        });
    }

    // ---- Tooltip — content-sized
    section(&mut lines, "Tooltip — content-sized", app)?;
    {
        let mut cells = Vec::new();
        for label in [
            "Send",
            "Copy response to clipboard",
            "The quick brown fox jumps over the lazy dog and keeps going",
        ] {
            let tw = app.painter.text_width(label, TextStyle::Muted);
            rep.content_widths
                .entry("tooltip".into())
                .or_default()
                .insert(tw.round() as u32);
            let (img, w) = tooltip_cell(app, &pal, label, 1.0)?;
            rep.tooltip_max = rep.tooltip_max.max(w);
            if w >= tokens::TOOLTIP_MAX_W {
                // clamped tooltip must keep its horizontal padding: text ink
                // starts >= 10 DIP inside the pill on both sides (the pill is
                // foreground fill, the text background)
                let pill = rgb8(tooltip_colors(&pal).0);
                // pill bbox first (foreground fill)
                let mut px = [u32::MAX, 0u32]; // pill x range
                let mut py = [u32::MAX, 0u32];
                for (i, p) in img.data.chunks_exact(4).enumerate() {
                    let x = i as u32 % img.width;
                    let y = i as u32 / img.width;
                    if close3([p[0], p[1], p[2]], pill) {
                        px[0] = px[0].min(x);
                        px[1] = px[1].max(x);
                        py[0] = py[0].min(y);
                        py[1] = py[1].max(y);
                    }
                }
                // text ink = columns inside the pill bbox with >= 3 pixels
                // matching the text colour closely (glyph strokes; pill-edge
                // antialias is a mid-tone, not the fg colour)
                let fg = rgb8(tooltip_colors(&pal).1);
                let mut ix = [u32::MAX, 0u32];
                if px[0] != u32::MAX {
                    // only the pill's vertical centre band — the rounded
                    // corner AA at the top/bottom edge would otherwise read
                    // as near-fg pixels across the full width
                    let qh = (py[1] - py[0]) / 4;
                    for x in px[0]..=px[1] {
                        let mut n = 0u32;
                        for y in (py[0] + qh)..=(py[1] - qh) {
                            let i = ((y * img.width + x) * 4) as usize;
                            let d: [u8; 3] = img.data[i..i + 3].try_into().unwrap();
                            let diff = d
                                .iter()
                                .zip(fg.iter())
                                .map(|(a, b)| (*a as i32 - *b as i32).abs())
                                .max()
                                .unwrap_or(0);
                            if diff < 60 {
                                n += 1;
                            }
                        }
                        if n >= 3 {
                            ix[0] = ix[0].min(x);
                            ix[1] = ix[1].max(x);
                        }
                    }
                }
                if !(px[0] != u32::MAX
                    && ix[0] != u32::MAX
                    && ix[0] >= px[0] + 10
                    && px[1] >= ix[1] + 10)
                {
                    rep.tooltip_pad_violations += 1;
                }
            }
            cells.push(Cell {
                cap: if w >= tokens::TOOLTIP_MAX_W {
                    format!("long — clamps to {w:.0} DIP")
                } else {
                    format!("\"{label}\" · {w:.0} DIP")
                },
                img,
            });
        }
        lines.push(Line::Row {
            sub: String::new(),
            cells,
        });
    }

    // ---- Native Text Input / Composer — stretchable
    section(
        &mut lines,
        "Native Text Input / Composer — stretchable",
        app,
    )?;
    {
        let mut cells = Vec::new();
        for w in ws {
            rep.stretch_widths
                .entry("composer".into())
                .or_default()
                .push(w);
            let img = app_cell(
                app,
                rep,
                theme,
                |a| {
                    presets::apply(a, "composer-text", false);
                },
                w,
            )?;
            cells.push(Cell {
                cap: format!("{w:.0} DIP"),
                img,
            });
        }
        lines.push(Line::Row {
            sub: String::new(),
            cells,
        });
    }

    // ---- Separator — stretchable
    section(&mut lines, "Separator — stretchable", app)?;
    {
        let mut cells = Vec::new();
        for w in ws {
            rep.stretch_widths
                .entry("separator".into())
                .or_default()
                .push(w);
            cells.push(Cell {
                cap: format!("{w:.0} DIP"),
                img: separator_cell(app, &pal, w, 1.0)?,
            });
        }
        lines.push(Line::Row {
            sub: String::new(),
            cells,
        });
    }

    // ---- Response / Content Surface — stretchable
    section(&mut lines, "Response / Content Surface — stretchable", app)?;
    {
        let mut cells = Vec::new();
        for w in ws {
            rep.stretch_widths
                .entry("response".into())
                .or_default()
                .push(w);
            let img = app_cell(
                app,
                rep,
                theme,
                |a| {
                    presets::apply(a, "response", false);
                },
                w,
            )?;
            cells.push(Cell {
                cap: format!("{w:.0} DIP"),
                img,
            });
        }
        lines.push(Line::Row {
            sub: String::new(),
            cells,
        });
    }

    // ---- Badge / Status Pill — content-sized
    section(&mut lines, "Badge / Status Pill — content-sized", app)?;
    {
        let mut cells = Vec::new();
        for label in ["Idle", "Thinking", "Waiting for provider"] {
            let tw = app.painter.text_width(label, TextStyle::Caption);
            rep.content_widths
                .entry("badge".into())
                .or_default()
                .insert(tw.round() as u32);
            let sz = badge_size(tw);
            cells.push(Cell {
                cap: format!("\"{label}\" · {:.0} DIP", sz.w),
                img: badge_cell(app, &pal, label, BadgeVariant::Secondary, 1.0)?,
            });
        }
        lines.push(Line::Row {
            sub: String::new(),
            cells,
        });
    }

    compose(
        app,
        rep,
        &lines,
        [bg3[0], bg3[1], bg3[2], 255],
        pal.muted_fg,
        1.0,
    )
}

// ---------------------------------------------------------------- DPI sheet

fn dpi_sheet(app: &mut App, rep: &mut Report) -> Result<RgbaImage, String> {
    let scales = [1.0f32, 1.25, 1.5, 2.0];
    let light = Theme::Light.palette();
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::Heading(heading_img(
        app,
        "DPI — 100% / 125% / 150% / 200%, native pixels (no resampling)",
        1.0,
        rgb8(light.surface),
        light.foreground,
    )?));
    for theme in Theme::ALL {
        let pal = theme.palette();
        // one band per theme: the theme's surface strip so dark cells sit on
        // dark (same structure as the reference sheets)
        let mut theme_lines: Vec<Line> = Vec::new();
        // painter cells at each scale
        for (sub, make) in [
            (format!("{} iconbtn focus", theme.name()), 0u8),
            (format!("{} button sm", theme.name()), 1u8),
            (format!("{} badge sec", theme.name()), 2u8),
            (format!("{} tooltip", theme.name()), 3u8),
        ] {
            let mut cells = Vec::new();
            for sc in scales {
                let img = match make {
                    0 => icon_button_cell(
                        app,
                        &pal,
                        IconButtonKind::Primary,
                        mascot_icons::Icon::ArrowUp,
                        ControlVisual {
                            focus_visible: true,
                            ..ControlVisual::default()
                        },
                        sc,
                    )?,
                    1 => button_cell(
                        app,
                        &pal,
                        "OK",
                        ButtonVariant::Default,
                        ButtonSize::Sm,
                        ControlVisual::default(),
                        sc,
                    )?,
                    2 => badge_cell(app, &pal, "Idle", BadgeVariant::Secondary, sc)?,
                    _ => tooltip_cell(app, &pal, "Send", sc)?.0,
                };
                cells.push(Cell {
                    cap: format!("{}%", (sc * 100.0) as u32),
                    img,
                });
            }
            theme_lines.push(Line::Row { sub, cells });
        }
        // composer text via App at each scale
        {
            let mut cells = Vec::new();
            for sc in scales {
                app.scale = sc;
                app.editor.set_scale(sc).ok();
                let _ = app.rebuild_sprite();
                let img = app_cell(
                    app,
                    rep,
                    *theme,
                    |a| {
                        presets::apply(a, "composer-text", false);
                    },
                    tokens::BUBBLE_W,
                )?;
                cells.push(Cell {
                    cap: format!("{}%", (sc * 100.0) as u32),
                    img,
                });
            }
            app.scale = 1.0;
            app.editor.set_scale(1.0).ok();
            let _ = app.rebuild_sprite();
            theme_lines.push(Line::Row {
                sub: format!("{} composer", theme.name()),
                cells,
            });
        }
        // the band is a nested compose on the theme's surface with its own
        // muted-fg captions
        let band = compose(
            app,
            rep,
            &theme_lines,
            [bg3(&pal)[0], bg3(&pal)[1], bg3(&pal)[2], 255],
            pal.muted_fg,
            1.0,
        )?;
        lines.push(Line::Gap(SECTION_GAP));
        lines.push(Line::Block(band));
    }
    compose(
        app,
        rep,
        &lines,
        [bg3(&light)[0], bg3(&light)[1], bg3(&light)[2], 255],
        light.muted_fg,
        1.0,
    )
}

fn bg3(p: &Palette) -> [u8; 3] {
    rgb8(p.surface)
}

/// within ~16 of the target rgb (antialiasing noise).
fn close3(c: [u8; 3], t: [u8; 3]) -> bool {
    c.iter()
        .zip(t.iter())
        .all(|(a, b)| (*a as i32 - *b as i32).abs() < 16)
}

// ----------------------------------------------------- shadcn comparison sheet

struct ProvStyle {
    height: String,
    width: String,
    padding: String,
    border_radius: String,
    border_width: String,
    border_color: String,
    border_color_hex: String,
    background_color: String,
    background_color_hex: String,
    color: String,
    color_hex: String,
    font_family: String,
    font_size: String,
    font_weight: String,
    line_height: String,
    box_shadow: String,
}

struct ProvEntry {
    file: String,
    component: String,
    example: String,
    docs_url: String,
    theme: String,
    state: String,
    data_variant: String,
    data_size: String,
    element_text: String,
    /// Border-box CSS px of the element the style was read from.
    rect_w: f64,
    rect_h: f64,
    style_element: String,
    computed_style: ProvStyle,
}

struct Prov {
    entries: Vec<ProvEntry>,
    /// `{ "--primary": {raw, hex}, ... }` per theme (raw serde value).
    theme_vars: serde_json::Value,
}

fn jstr(v: &serde_json::Value, key: &str) -> String {
    v.get(key)
        .and_then(|s| s.as_str())
        .unwrap_or("")
        .to_string()
}

fn jnum(v: &serde_json::Value, key: &str) -> f64 {
    v.get(key).and_then(|s| s.as_f64()).unwrap_or(0.0)
}

fn load_prov(ref_dir: &Path) -> Result<Prov, String> {
    let raw = std::fs::read_to_string(ref_dir.join("provenance.json"))
        .map_err(|e| format!("provenance.json: {e}"))?;
    let v: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("provenance.json: {e}"))?;
    let theme_vars = v
        .get("themeVars")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let arr = v
        .get("captures")
        .and_then(|c| c.as_array())
        .ok_or_else(|| "provenance.json: no captures array".to_string())?;
    let entries = arr
        .iter()
        .map(|e| {
            let cs = e.get("computedStyle").cloned().unwrap_or_default();
            let rc = e.get("rect").cloned().unwrap_or_default();
            ProvEntry {
                file: jstr(e, "file"),
                component: jstr(e, "component"),
                example: jstr(e, "example"),
                docs_url: jstr(e, "docsUrl"),
                theme: jstr(e, "theme"),
                state: jstr(e, "state"),
                data_variant: jstr(e, "dataVariant"),
                data_size: jstr(e, "dataSize"),
                element_text: jstr(e, "elementText"),
                rect_w: jnum(&rc, "width"),
                rect_h: jnum(&rc, "height"),
                style_element: jstr(e, "styleElement"),
                computed_style: ProvStyle {
                    height: jstr(&cs, "height"),
                    width: jstr(&cs, "width"),
                    padding: jstr(&cs, "padding"),
                    border_radius: jstr(&cs, "borderRadius"),
                    border_width: jstr(&cs, "borderWidth"),
                    border_color: jstr(&cs, "borderColor"),
                    border_color_hex: jstr(&cs, "borderColorHex"),
                    background_color: jstr(&cs, "backgroundColor"),
                    background_color_hex: jstr(&cs, "backgroundColorHex"),
                    color: jstr(&cs, "color"),
                    color_hex: jstr(&cs, "colorHex"),
                    font_family: jstr(&cs, "fontFamily"),
                    font_size: jstr(&cs, "fontSize"),
                    font_weight: jstr(&cs, "fontWeight"),
                    line_height: jstr(&cs, "lineHeight"),
                    box_shadow: jstr(&cs, "boxShadow"),
                },
            }
        })
        .collect();
    Ok(Prov {
        entries,
        theme_vars,
    })
}

fn load_ref(ref_dir: &Path, stem: &str, theme: Theme) -> Result<RgbaImage, String> {
    let f = ref_dir.join(format!("{stem}-{}.png", theme.name()));
    RgbaImage::load(&f).map_err(|e| format!("reference {}: {e}", f.display()))
}

/// Guard: downscale only an outlier that would blow the column out.
fn fit_ref(img: RgbaImage) -> RgbaImage {
    if img.width > REF_MAX_W {
        img.downscale(img.width.div_ceil(REF_MAX_W))
    } else {
        img
    }
}

/// A native comparison cell: the flattened image plus the *measured* fields
/// that produced it (rect DIP, padding, radius, fill/fg hex, font) — these go
/// into shadcn-comparison.md, not approximations.
struct NativeCell {
    img: RgbaImage,
    fields: Vec<(String, String)>,
}

impl NativeCell {
    fn f(mut self, k: &str, v: impl Into<String>) -> Self {
        self.fields.push((k.to_string(), v.into()));
        self
    }
}

/// DIP padding around a native comparison cell — matches the ref crop's 16
/// CSS px at the same effective scale.
const REF_CELL_PAD: f32 = 16.0;

/// One comparison row: which ref stem pairs with which native cell spec.
struct RefRow {
    label: &'static str,
    stem: &'static str,
    native: &'static str,
    /// Label text the native cell renders (same-content rule).
    ntext: &'static str,
}

fn ref_rows(key: &str) -> Vec<RefRow> {
    let r = |label, stem, native, ntext| RefRow {
        label,
        stem,
        native,
        ntext,
    };
    match key {
        "icon-button" => vec![
            r("default", "button-icon", "iconbutton-dual-default", ""),
            r("hover", "button-icon-hover", "iconbutton-dual-hover", ""),
            r(
                "focus-visible",
                "button-icon-focus-visible",
                "iconbutton-dual-focus",
                "",
            ),
        ],
        "button" => vec![
            r("default", "button-default", "button-default", "Button"),
            r("hover", "button-hover", "button-hover", "Button"),
            r(
                "focus-visible",
                "button-focus-visible",
                "button-focus",
                "Button",
            ),
            r("disabled", "button-disabled", "button-disabled", "Button"),
            r(
                "secondary",
                "button-secondary",
                "button-secondary",
                "Secondary",
            ),
            r(
                "secondary hover",
                "button-secondary-hover",
                "button-secondary-hover",
                "Secondary",
            ),
            r("ghost", "button-ghost", "button-ghost", "Ghost"),
            r(
                "ghost hover",
                "button-ghost-hover",
                "button-ghost-hover",
                "Ghost",
            ),
        ],
        "tooltip" => vec![r("open", "tooltip-open", "tooltip", "Add to library")],
        "badge" => vec![
            r("default", "badge-default", "badge-default", "Badge"),
            r(
                "secondary",
                "badge-secondary",
                "badge-secondary",
                "Secondary",
            ),
            r("outline", "badge-outline", "badge-outline", "Outline"),
        ],
        "separator" => vec![r("horizontal", "separator", "separator", "")],
        "surface" => vec![r("card", "card", "surface-plain", "")],
        "typography" => vec![
            r("p", "typography-p", "typo-p", ""),
            r("muted", "typography-muted", "typo-muted", ""),
            r("small", "typography-small", "typo-small", ""),
        ],
        "composer" => vec![
            r("default", "textarea-default", "composer-empty", ""),
            r(
                "focus-visible",
                "textarea-focus-visible",
                "composer-focused",
                "",
            ),
            r(
                "disabled (native: submitting)",
                "textarea-disabled",
                "composer-submitting",
                "",
            ),
        ],
        "response" => vec![r("default", "", "response", "")],
        "icon" => vec![r("set", "", "icon-set", "")],
        _ => vec![],
    }
}

/// Native side of a comparison cell at REF_SCALE, flattened over the band
/// theme's surface, returning measured values alongside the image.
fn native_ref_cell(
    app: &mut App,
    rep: &mut Report,
    spec: &str,
    ntext: &str,
    theme: Theme,
    prov: Option<&ProvEntry>,
) -> Result<NativeCell, String> {
    // most cells don't need provenance; the ones that measure against the
    // reference rect unwrap it
    let need_prov = |spec: &str| -> Result<&ProvEntry, String> {
        prov.ok_or_else(|| format!("{spec}: needs reference provenance"))
    };
    let pal = theme.palette();
    let v = |hover: bool, pressed: bool, focus: bool, dis: bool| ControlVisual {
        hover,
        pressed,
        focus_visible: focus,
        disabled: dis,
    };
    match spec {
        // IconButton: ref is outline/size-icon — Mascot has no outline icon
        // button, so the native cell shows ghost 28 + primary 32 side by side
        s if s.starts_with("iconbutton-dual") => {
            let vv = match s {
                "iconbutton-dual-hover" => v(true, false, false, false),
                "iconbutton-dual-focus" => v(false, false, true, false),
                _ => v(false, false, false, false),
            };
            let g = tokens::GHOST_BUTTON;
            let p_edge = tokens::PRIMARY_BUTTON;
            let w = REF_CELL_PAD + g + 12.0 + p_edge + REF_CELL_PAD;
            let h = p_edge + 2.0 * REF_CELL_PAD;
            let p = &app.painter;
            let img = cell_img(app, &pal, w, h, REF_SCALE, |ctx| {
                p.icon_button(
                    ctx,
                    &pal,
                    Rect::new(REF_CELL_PAD, REF_CELL_PAD + (p_edge - g) / 2.0, g, g),
                    mascot_icons::Icon::Copy,
                    icon_button_paint(&pal, IconButtonKind::Ghost, vv),
                )?;
                p.icon_button(
                    ctx,
                    &pal,
                    Rect::new(REF_CELL_PAD + g + 12.0, REF_CELL_PAD, p_edge, p_edge),
                    mascot_icons::Icon::ArrowUp,
                    icon_button_paint(&pal, IconButtonKind::Primary, vv),
                )
            })?;
            // measured per-control values: ghost 28 (Copy) + primary 32 (Send)
            let fill_g = icon_button_colors(&pal, IconButtonKind::Ghost, vv).0;
            let fill_p = icon_button_colors(&pal, IconButtonKind::Primary, vv).0;
            Ok(NativeCell {
                img,
                fields: vec![],
            }
            .f(
                "native variant",
                "ghost 28 + primary 32 (no outline icon button)",
            )
            .f("width", format!("{g} / {p_edge} DIP"))
            .f("height", format!("{g} / {p_edge} DIP"))
            .f("borderRadius", format!("{} DIP", tokens::RADIUS_MD))
            .f(
                "backgroundColor",
                format!("ghost {} / primary {}", hex(fill_g), hex(fill_p)),
            )
            .f("borderWidth", "none (1 DIP border only on focus-visible)")
            .f(
                "ring",
                if vv.focus_visible {
                    format!(
                        "{} DIP at ring 50 % + 1 DIP {}",
                        tokens::FOCUS_RING_W,
                        hex(pal.ring)
                    )
                } else {
                    "none".into()
                },
            )
            .f(
                "color",
                format!(
                    "ghost {} / primary {}",
                    hex(pal.foreground),
                    hex(pal.primary_fg)
                ),
            )
            .f("fontFamily", "icon geometry (Lucide paths)"))
        }
        s if s.starts_with("button-") => {
            let (variant, vv) = match s {
                "button-hover" => (ButtonVariant::Default, v(true, false, false, false)),
                "button-focus" => (ButtonVariant::Default, v(false, false, true, false)),
                "button-disabled" => (ButtonVariant::Default, v(false, false, false, true)),
                "button-secondary" => (ButtonVariant::Secondary, v(false, false, false, false)),
                "button-secondary-hover" => {
                    (ButtonVariant::Secondary, v(true, false, false, false))
                }
                "button-ghost" => (ButtonVariant::Ghost, v(false, false, false, false)),
                "button-ghost-hover" => (ButtonVariant::Ghost, v(true, false, false, false)),
                _ => (ButtonVariant::Default, v(false, false, false, false)),
            };
            let tw = app.painter.text_width(ntext, TextStyle::Label);
            let sz = button_size(tw, ButtonSize::Default);
            let p = &app.painter;
            let (w2, h2) = (sz.w, sz.h);
            let img = cell_img(
                app,
                &pal,
                w2 + 2.0 * REF_CELL_PAD,
                h2 + 2.0 * REF_CELL_PAD,
                REF_SCALE,
                |ctx| {
                    p.button(
                        ctx,
                        &pal,
                        Rect::new(REF_CELL_PAD, REF_CELL_PAD, w2, h2),
                        ntext,
                        button_paint(&pal, variant, vv),
                    )
                },
            )?;
            Ok(NativeCell {
                img,
                fields: vec![],
            }
            .f("width", format!("{w2} DIP"))
            .f("height", format!("{h2} DIP"))
            .f("padding", format!("0 {} DIP", tokens::BUTTON_PAD_X))
            .f("borderRadius", format!("{} DIP", tokens::RADIUS_MD))
            .f(
                "borderWidth",
                if vv.focus_visible {
                    "ring 1.5 DIP"
                } else {
                    "none"
                },
            )
            .f("backgroundColor", hex(button_colors(&pal, variant, vv).0))
            .f("color", hex(button_colors(&pal, variant, vv).1))
            .f("fontFamily", app.painter.fonts.family.clone())
            .f("fontSize", format!("{} DIP", tokens::BODY_SIZE))
            .f("fontWeight", "500")
            .f("boxShadow", "none"))
        }
        "tooltip" => {
            let tw = app.painter.text_width(ntext, TextStyle::Muted);
            let ts = tooltip_size(tw);
            let p = &app.painter;
            let img = cell_img(
                app,
                &pal,
                ts.w + 2.0 * REF_CELL_PAD,
                ts.h + 2.0 * REF_CELL_PAD,
                REF_SCALE,
                |ctx| {
                    p.tooltip(
                        ctx,
                        &pal,
                        Rect::new(REF_CELL_PAD, REF_CELL_PAD, ts.w, ts.h),
                        ntext,
                    )
                },
            )?;
            Ok(NativeCell {
                img,
                fields: vec![],
            }
            .f("width", format!("{} DIP", ts.w))
            .f("height", format!("{} DIP", ts.h))
            .f(
                "padding",
                format!("{} x {} DIP", tokens::TOOLTIP_PAD_Y, tokens::TOOLTIP_PAD_X),
            )
            .f("borderRadius", format!("{} DIP", tokens::RADIUS_MD))
            .f("backgroundColor", hex(tooltip_colors(&pal).0))
            .f("color", hex(tooltip_colors(&pal).1))
            .f("fontFamily", app.painter.fonts.family.clone())
            .f("fontSize", format!("{} DIP", tokens::SMALL_SIZE))
            .f("fontWeight", "400")
            .f("boxShadow", "none"))
        }
        s if s.starts_with("badge-") => {
            let variant = match s {
                "badge-secondary" => BadgeVariant::Secondary,
                "badge-outline" => BadgeVariant::Outline,
                _ => BadgeVariant::Default,
            };
            let tw = app.painter.text_width(ntext, TextStyle::Caption);
            let bs = badge_size(tw);
            let p = &app.painter;
            let img = cell_img(
                app,
                &pal,
                bs.w + 2.0 * REF_CELL_PAD,
                bs.h + 2.0 * REF_CELL_PAD,
                REF_SCALE,
                |ctx| {
                    p.badge(
                        ctx,
                        &pal,
                        Rect::new(REF_CELL_PAD, REF_CELL_PAD, bs.w, bs.h),
                        ntext,
                        variant,
                    )
                },
            )?;
            let (fill, fg_c, border) = badge_colors(&pal, variant);
            let (border_label, border_hex) = match border {
                Some(b) => ("border", hex(b)),
                None => ("transparent", "transparent".to_string()),
            };
            Ok(NativeCell {
                img,
                fields: vec![],
            }
            .f("width", format!("{} DIP", bs.w))
            .f("height", format!("{} DIP", bs.h))
            .f(
                "padding",
                format!("{} x {} DIP", tokens::BADGE_PAD_Y, tokens::BADGE_PAD_X),
            )
            .f("borderRadius", format!("{} DIP (pill)", bs.h / 2.0))
            .f("borderWidth", format!("1 DIP ({})", border_label))
            .f("borderColor", border_hex)
            .f("backgroundColor", hex(fill))
            .f("color", hex(fg_c))
            .f("fontFamily", app.painter.fonts.family.clone())
            .f("fontSize", format!("{} DIP", tokens::SMALL_SIZE))
            .f("fontWeight", "500")
            .f("boxShadow", "none"))
        }
        "separator" => {
            let prov = need_prov(spec)?;
            // same length as the ref element; >= 16 DIP vertical padding so
            // the hairline is visible in context
            let w = (prov.rect_w as f32).max(1.0);
            rep.size_pairs
                .push(("separator w".into(), prov.rect_w, w as f64));
            let p = &app.painter;
            let img = cell_img(app, &pal, w, 1.0 + 2.0 * REF_CELL_PAD, REF_SCALE, |ctx| {
                p.separator(
                    ctx,
                    &pal,
                    Rect::new(0.0, REF_CELL_PAD, w, tokens::SEPARATOR_H),
                )
            })?;
            Ok(NativeCell {
                img,
                fields: vec![],
            }
            .f("width", format!("{w} DIP"))
            .f("height", format!("{} DIP", tokens::SEPARATOR_H))
            .f("backgroundColor", hex(pal.border))
            .f("boxShadow", "none"))
        }
        "surface-plain" => {
            let prov = need_prov(spec)?;
            // render the native Surface at the ref card's CSS border box so
            // radius/border/shadow compare at equal scale
            let w = prov.rect_w as f32;
            let h = prov.rect_h as f32;
            rep.size_pairs
                .push(("surface w".into(), prov.rect_w, w as f64));
            rep.size_pairs
                .push(("surface h".into(), prov.rect_h, h as f64));
            let img = surface_cell(app, &pal, w, h, false, REF_SCALE)?;
            Ok(NativeCell {
                img,
                fields: vec![],
            }
            .f("width", format!("{w} DIP"))
            .f("height", format!("{h} DIP"))
            .f("borderRadius", format!("{} DIP", tokens::RADIUS_XL))
            .f("borderWidth", "1 DIP")
            .f("borderColor", hex(pal.border))
            .f("backgroundColor", hex(pal.surface))
            .f("boxShadow", "composited drop shadow"))
        }
        s if s.starts_with("typo-") => {
            let prov = need_prov(spec)?;
            // same text, wrapped at the reference's CSS width.
            // mapping: shadcn p -> native body 14/400, muted -> muted 12/400,
            // small -> label 14/500 (shadcn `small` renders bolded small text)
            let ref_w = prov.rect_w as f32;
            let (style, fname) = match s {
                "typo-muted" => (TextStyle::Muted, "muted 12/400"),
                "typo-small" => (TextStyle::Label, "label 14/500"),
                _ => (TextStyle::Body, "body 14/400"),
            };
            let fmt = app.painter.fonts.format(style).clone();
            let color = match style {
                TextStyle::Muted => pal.muted_fg,
                _ => pal.foreground,
            };
            rep.size_pairs
                .push(("typography wrap w".into(), prov.rect_w, ref_w as f64));
            let text = prov.element_text.as_str();
            let (size, weight) = match style {
                TextStyle::Label => (tokens::BODY_SIZE, "500"),
                TextStyle::Muted => (tokens::SMALL_SIZE, "400"),
                _ => (tokens::BODY_SIZE, "400"),
            };
            let (_tw, th) = app
                .painter
                .fonts
                .measure(text, ref_w, &fmt)
                .map_err(|e| e.to_string())?;
            let img = text_img_wrap(app, text, ref_w, &fmt, color, REF_SCALE, rgb8(pal.surface))?;
            Ok(NativeCell {
                img,
                fields: vec![],
            }
            .f("native style", fname)
            .f("width", format!("{ref_w} DIP (ref wrap width)"))
            .f("height", format!("{:.0} DIP", th))
            .f("color", hex(color))
            .f("fontFamily", app.painter.fonts.family.clone())
            .f("fontSize", format!("{size} DIP"))
            .f("fontWeight", weight))
        }
        "composer-empty" | "composer-focused" | "composer-submitting" => {
            let prov = need_prov(spec)?;
            rep.size_pairs
                .push(("composer w".into(), prov.rect_w, tokens::BUBBLE_W as f64));
            let preset = match spec {
                "composer-empty" => "composer-empty",
                "composer-focused" => "composer-focused",
                _ => "submitting",
            };
            Ok(app_ref_cell(app, rep, theme, preset)?)
        }
        "response" => app_ref_cell(app, rep, theme, "response"),
        "icon-set" => {
            let gap = 6.0;
            let n = mascot_icons::Icon::ALL.len() as f32;
            let w = n * tokens::ICON_SIZE + (n - 1.0) * gap + 2.0 * REF_CELL_PAD;
            let h = tokens::ICON_SIZE + 2.0 * REF_CELL_PAD;
            let p = &app.painter;
            let img = cell_img(app, &pal, w, h, REF_SCALE, |ctx| {
                let mut x = REF_CELL_PAD;
                for ic in mascot_icons::Icon::ALL {
                    p.icon(
                        ctx,
                        *ic,
                        Rect::new(x, REF_CELL_PAD, tokens::ICON_SIZE, tokens::ICON_SIZE),
                        pal.foreground,
                    )?;
                    x += tokens::ICON_SIZE + gap;
                }
                Ok(())
            })?;
            Ok(NativeCell {
                img,
                fields: vec![],
            }
            .f("height", format!("{} DIP", tokens::ICON_SIZE))
            .f("color", hex(pal.foreground)))
        }
        other => Err(format!("no native cell spec {other}")),
    }
}

/// App-frame comparison cell at REF_SCALE (scale set around the render),
/// with measured layout values for comparison.md.
fn app_ref_cell(
    app: &mut App,
    rep: &mut Report,
    theme: Theme,
    preset: &str,
) -> Result<NativeCell, String> {
    app.scale = REF_SCALE;
    app.editor.set_scale(REF_SCALE).ok();
    let _ = app.rebuild_sprite();
    let img = app_cell(
        app,
        rep,
        theme,
        |a| {
            presets::apply(a, preset, false);
        },
        tokens::BUBBLE_W,
    )?;
    let lay = app.layout;
    app.scale = 1.0;
    app.editor.set_scale(1.0).ok();
    let _ = app.rebuild_sprite();
    let pal = theme.palette();
    let composer = lay
        .composer
        .map(|c| format!("{}x{} DIP", c.w, c.h))
        .unwrap_or_else(|| "—".into());
    let composer_h = lay.composer.map(|c| c.h).unwrap_or(0.0);
    Ok(NativeCell {
        img,
        fields: vec![],
    }
    .f("width", format!("{} DIP", tokens::BUBBLE_W))
    .f(
        "height",
        format!("{composer_h:.0} DIP (composer, measured)"),
    )
    .f("composer", composer)
    .f(
        "padding",
        format!(
            "editor insets {} DIP x / {} DIP y; send edge {} DIP",
            tokens::BUBBLE_PAD_X,
            tokens::EDITOR_PAD_Y,
            tokens::COMPOSER_EDGE
        ),
    )
    .f(
        "borderRadius",
        format!("{} DIP (bubble)", tokens::RADIUS_XL),
    )
    .f("borderWidth", "1 DIP")
    .f("borderColor", hex(pal.border))
    .f("backgroundColor", hex(pal.surface))
    .f("color", hex(pal.foreground))
    .f("fontFamily", app.painter.fonts.family.clone())
    .f("fontSize", format!("{} DIP", tokens::BODY_SIZE))
    .f("fontWeight", "400")
    .f("lineHeight", format!("{} DIP", tokens::BODY_LINE)))
}

// ---------------------------------------------------------- theme band rows

/// One row inside a theme band: label + optional ref image + native image.
struct BandRow {
    label: RgbaImage,
    reference: Option<RgbaImage>,
    native: Option<RgbaImage>,
}

/// Builds a full-width theme band image: surface-coloured strip, column
/// headers, then one row per matched pair with fixed column x positions
/// (widest cell per column in the block + 48 px gap). Cells v-center on the
/// row's max height. All text is alpha-blended; nothing paints a band.
/// Builds a full-width theme band image: surface-coloured strip, column
/// headers, then one row per matched pair with fixed column x positions
/// (widest of cell/header per column + 48 px gap). Cells v-center on the
/// row's max height. All text is alpha-blended; nothing paints a band.
/// Header/cell intersection is checked into `rep.caption_violations`.
fn build_band(
    app: &App,
    rep: &mut Report,
    theme: Theme,
    rows: Vec<BandRow>,
) -> Result<RgbaImage, String> {
    let pal = theme.palette();
    let has_ref = rows.iter().any(|r| r.reference.is_some());
    let pad = 24u32;
    let col_gap = 48u32;
    let row_gap = 20u32;

    // headers first — the column must fit both the widest cell AND the
    // header text, otherwise "shadcn reference" collides with "native"
    let fonts = &app.painter.fonts;
    let hdr_fmt = fonts.caption.clone();
    let ref_hdr = if has_ref {
        Some(text_img_alpha(
            app,
            "shadcn reference",
            &hdr_fmt,
            pal.muted_fg,
            REF_SCALE,
        )?)
    } else {
        None
    };
    let nat_hdr = text_img_alpha(app, "native", &hdr_fmt, pal.muted_fg, REF_SCALE)?;
    let hdr_h = nat_hdr.height + 8;

    // column widths: widest cell or header per column in this block
    let label_w = rows.iter().map(|r| r.label.width).max().unwrap_or(0) + 24;
    let ref_w = rows
        .iter()
        .filter_map(|r| r.reference.as_ref().map(|i| i.width))
        .max()
        .unwrap_or(0)
        .max(ref_hdr.as_ref().map(|h| h.width).unwrap_or(0));
    let nat_w = rows
        .iter()
        .filter_map(|r| r.native.as_ref().map(|i| i.width))
        .max()
        .unwrap_or(0)
        .max(nat_hdr.width);

    let label_x = pad;
    let ref_x = label_x + label_w + col_gap;
    let nat_x = if has_ref {
        ref_x + ref_w + col_gap
    } else {
        ref_x
    };
    let band_w = nat_x + nat_w + pad;

    let mut band_h = pad + hdr_h;
    for r in &rows {
        let rh = r
            .reference
            .as_ref()
            .map(|i| i.height)
            .unwrap_or(0)
            .max(r.native.as_ref().map(|i| i.height).unwrap_or(0))
            .max(r.label.height);
        band_h += rh + row_gap;
    }

    // header positions must not intersect each other or the first row's cells
    if let Some(h) = &ref_hdr {
        if ref_x + h.width > nat_x {
            rep.caption_violations += 1;
        }
        if pad + h.height > pad + hdr_h {
            rep.caption_violations += 1;
        }
    }

    let mut band = RgbaImage::new(band_w, band_h);
    let s = rgb8(pal.surface);
    band.fill_rect(0, 0, band_w, band_h, [s[0], s[1], s[2], 255]);

    // headers (alpha text over the band surface — no band of their own)
    let mut y = pad;
    if let Some(h) = &ref_hdr {
        band.blend_over(h, ref_x, y);
    }
    band.blend_over(&nat_hdr, nat_x, y);
    y += hdr_h;

    for r in &rows {
        let ref_h = r.reference.as_ref().map(|i| i.height).unwrap_or(0);
        let nat_h = r.native.as_ref().map(|i| i.height).unwrap_or(0);
        let rh = ref_h.max(nat_h).max(r.label.height);
        // label vertically centred in its own column — never over a cell
        band.blend_over(&r.label, label_x, y + (rh - r.label.height) / 2);
        if let Some(img) = &r.reference {
            band.blit(img, ref_x, y + (rh - img.height) / 2);
        }
        if let Some(img) = &r.native {
            band.blit(img, nat_x, y + (rh - img.height) / 2);
        }
        // label column must not reach into the ref column
        if label_x + r.label.width > ref_x {
            rep.caption_violations += 1;
        }
        y += rh + row_gap;
    }
    Ok(band)
}

// ----------------------------------------------------------- sheet assembly

fn shadcn_sheets(
    app: &mut App,
    ref_dir: &Path,
    rep: &mut Report,
) -> Result<(RgbaImage, RgbaImage, RgbaImage, RgbaImage, RgbaImage), String> {
    let prov = load_prov(ref_dir)?;
    rep.theme_vars = prov.theme_vars.clone();
    let page_bg = [0xFFu8, 0xFF, 0xFF]; // neutral page; bands carry theme surface
    let fg = Theme::Light.palette().foreground;
    let muted = Theme::Light.palette().muted_fg;

    // split so each sheet stays under the 2600x4000 cap
    let groups: [(&str, &[&str]); 5] = [
        (
            "shadcn reference vs native — refs DPR 2, native 200 %",
            &["icon-button", "tooltip", "badge"],
        ),
        (
            "shadcn reference vs native — refs DPR 2, native 200 % (part 2)",
            &["button"],
        ),
        (
            "shadcn reference vs native — refs DPR 2, native 200 % (part 3)",
            &["separator", "surface"],
        ),
        (
            "shadcn reference vs native — refs DPR 2, native 200 % (part 4)",
            &["typography", "composer"],
        ),
        (
            "shadcn reference vs native — refs DPR 2, native 200 % (part 5)",
            &["response", "icon"],
        ),
    ];

    let mut sheets = Vec::new();
    for (title, group) in groups {
        let mut lines: Vec<Line> = Vec::new();
        lines.push(Line::Heading(heading_img(
            app, title, REF_SCALE, page_bg, fg,
        )?));

        for key in group {
            let e = by_key(key).ok_or_else(|| format!("inventory: no key {key}"))?;
            let head = match e.shadcn {
                Some(slug) => {
                    let url = prov
                        .entries
                        .iter()
                        .find(|p| p.docs_url.ends_with(slug))
                        .map(|p| p.docs_url.clone())
                        .unwrap_or_else(|| format!("https://ui.shadcn.com/docs/components/{slug}"));
                    format!("{} — shadcn: {} — {}", e.name, slug, url)
                }
                None => format!("{} — {}", e.name, e.no_analogue_reason),
            };
            lines.push(Line::Gap(SECTION_GAP));
            lines.push(Line::Heading(heading_img(
                app, &head, REF_SCALE, page_bg, fg,
            )?));

            // per-theme bands: rows of (label, ref, native)
            let mut bands: Vec<RgbaImage> = Vec::new();
            for theme in Theme::ALL {
                let pal = theme.palette();
                let mut rows: Vec<BandRow> = Vec::new();
                for row in ref_rows(key) {
                    let (reference, pe) = if row.stem.is_empty() {
                        // no analogue — no placeholder cell; native only
                        (None, None)
                    } else {
                        let stem_theme = format!("{}-{}.png", row.stem, theme.name());
                        let pe = prov
                            .entries
                            .iter()
                            .find(|p| p.file == stem_theme)
                            .ok_or_else(|| format!("provenance missing {stem_theme}"))?;
                        rep.ref_files.push(stem_theme);
                        rep.ref_keys_used.insert(key.to_string());
                        (
                            Some(fit_ref(load_ref(ref_dir, row.stem, *theme)?)),
                            Some(pe),
                        )
                    };
                    let cell = native_ref_cell(app, rep, row.native, row.ntext, *theme, pe)?;
                    if let Some(pe) = pe {
                        rep.native_specs
                            .insert(pe.file.clone(), cell.fields.clone());
                    }
                    let mk_label = |text: &str| -> Result<RgbaImage, String> {
                        text_img_alpha(
                            app,
                            text,
                            &app.painter.fonts.caption.clone(),
                            pal.muted_fg,
                            REF_SCALE,
                        )
                    };
                    rows.push(BandRow {
                        label: mk_label(row.label)?,
                        reference,
                        native: Some(cell.img),
                    });
                }
                bands.push(build_band(app, rep, *theme, rows)?);
            }
            // pad both bands to the same width so the block reads as one
            // uniform card (light band over dark band)
            let bw = bands.iter().map(|b| b.width).max().unwrap_or(0);
            for (i, band) in bands.drain(..).enumerate() {
                let pal = Theme::ALL[i].palette();
                let s = rgb8(pal.surface);
                let mut full = RgbaImage::new(bw, band.height);
                full.fill_rect(0, 0, bw, band.height, [s[0], s[1], s[2], 255]);
                full.blit(&band, 0, 0);
                lines.push(Line::Gap(12));
                lines.push(Line::Block(full));
            }

            // deviations — wrapped bullet list under the block
            for d in e.deviations {
                let img = text_img_wrap(
                    app,
                    &format!("- {d}"),
                    DEV_WRAP_DIP,
                    &app.painter.fonts.caption.clone(),
                    muted,
                    REF_SCALE,
                    page_bg,
                )?;
                lines.push(Line::Block(img));
            }
        }
        let sheet = compose(
            app,
            rep,
            &lines,
            [page_bg[0], page_bg[1], page_bg[2], 255],
            muted,
            REF_SCALE,
        )?;
        sheets.push(sheet);
    }
    Ok((
        sheets.remove(0),
        sheets.remove(0),
        sheets.remove(0),
        sheets.remove(0),
        sheets.remove(0),
    ))
}

// ------------------------------------------------------- shadcn-comparison.md

/// shadcn theme var -> the native palette slot used for the same role.
type ThemeVarMap = Vec<(&'static str, &'static str, fn(&Palette) -> [f32; 4])>;
fn theme_var_map() -> ThemeVarMap {
    vec![
        ("--background", "surface", |p| p.surface),
        ("--foreground", "foreground", |p| p.foreground),
        ("--card", "surface", |p| p.surface),
        ("--card-foreground", "foreground", |p| p.foreground),
        ("--popover", "surface", |p| p.surface),
        ("--primary", "primary", |p| p.primary),
        ("--primary-foreground", "primary_fg", |p| p.primary_fg),
        ("--secondary", "secondary", |p| p.secondary),
        ("--secondary-foreground", "secondary_fg", |p| p.secondary_fg),
        ("--muted", "muted", |p| p.muted),
        ("--muted-foreground", "muted_fg", |p| p.muted_fg),
        ("--accent", "hover", |p| p.hover),
        ("--accent-foreground", "foreground", |p| p.foreground),
        ("--border", "border", |p| p.border),
        ("--input", "border", |p| p.border),
        ("--ring", "ring", |p| p.ring),
    ]
}

fn var_hex(vars: &serde_json::Value, theme: &str, name: &str) -> String {
    vars.get(theme)
        .and_then(|t| t.get(name))
        .and_then(|v| v.get("hex"))
        .and_then(|h| h.as_str())
        .unwrap_or("—")
        .to_string()
}

fn var_raw(vars: &serde_json::Value, theme: &str, name: &str) -> String {
    vars.get(theme)
        .and_then(|t| t.get(name))
        .and_then(|v| v.get("raw"))
        .and_then(|h| h.as_str())
        .unwrap_or("")
        .to_string()
}

/// `shadcn-comparison.md` — theme-var mapping table, then one section per
/// reference PNG: ref computed style (raw + hex + rect) vs the MEASURED
/// values of the native cell drawn for that pair.
fn write_comparison(out: &Path, prov: &Prov, rep: &Report) -> Result<(), String> {
    let mut md = String::new();
    md.push_str("# shadcn computed styles vs native tokens\n\n");
    md.push_str(
        "Ref values are getComputedStyle on the captured element (raw CSS + sRGB hex via canvas normalisation).\n",
    );
    md.push_str("Native values are measured from the rect/palette that drew the gallery cell.\n\n");

    // theme-var table: ref hex per theme vs native palette token
    md.push_str("## Theme variables — shadcn root vars vs native palette\n\n");
    md.push_str("| shadcn var | light ref | native light | dark ref | native dark |\n|---|---|---|---|---|\n");
    for (var, token, get) in theme_var_map() {
        let lr = var_hex(&prov.theme_vars, "light", var);
        let dr = var_hex(&prov.theme_vars, "dark", var);
        let ln = hex(get(&Theme::Light.palette()));
        let dn = hex(get(&Theme::Dark.palette()));
        md.push_str(&format!(
            "| `{var}` | `{lr}` | `{token}` `{ln}` | `{dr}` | `{token}` `{dn}` |\n"
        ));
    }
    md.push_str(&format!(
        "\nRadius: `--radius` ref `{}` / `{}` — native uses radius tokens sm 6 / md 8 / lg 10 / xl 14 DIP.\n",
        var_raw(&prov.theme_vars, "light", "--radius"),
        var_raw(&prov.theme_vars, "dark", "--radius")
    ));

    for e in &prov.entries {
        let s = &e.computed_style;
        md.push_str(&format!(
            "\n## {} — {} · `{}` — {} ({}/{}) — {}\n\n",
            e.file, e.component, e.example, e.state, e.data_variant, e.data_size, e.theme
        ));
        md.push_str(&format!(
            "ref rect: {:.0} x {:.0} CSS px (element `{}`)\n\n",
            e.rect_w, e.rect_h, e.style_element
        ));
        md.push_str("| field | reference | native (measured) |\n|---|---|---|\n");
        let refv: Vec<(&str, String)> = vec![
            ("width", s.width.clone()),
            ("height", s.height.clone()),
            ("padding", s.padding.clone()),
            ("borderRadius", s.border_radius.clone()),
            ("borderWidth", s.border_width.clone()),
            (
                "borderColor",
                format!("{} {}", s.border_color, s.border_color_hex),
            ),
            (
                "backgroundColor",
                format!("{} {}", s.background_color, s.background_color_hex),
            ),
            ("color", format!("{} {}", s.color, s.color_hex)),
            ("fontFamily", s.font_family.clone()),
            ("fontSize", s.font_size.clone()),
            ("fontWeight", s.font_weight.clone()),
            ("lineHeight", s.line_height.clone()),
            ("boxShadow", s.box_shadow.clone()),
        ];
        let nat = rep.native_specs.get(&e.file).cloned().unwrap_or_default();
        for (k, rv) in refv {
            let nv = nat
                .iter()
                .find(|(nk, _)| nk == k)
                .map(|(_, v)| v.as_str())
                .unwrap_or("—");
            md.push_str(&format!("| {k} | `{rv}` | `{nv}` |\n"));
        }
        // extra native-only rows (variant/style/composer)
        for (k, v) in nat.iter().filter(|(k, _)| {
            !matches!(
                k.as_str(),
                "width"
                    | "height"
                    | "padding"
                    | "borderRadius"
                    | "borderWidth"
                    | "borderColor"
                    | "backgroundColor"
                    | "color"
                    | "fontFamily"
                    | "fontSize"
                    | "fontWeight"
                    | "lineHeight"
                    | "boxShadow"
            )
        }) {
            md.push_str(&format!("| {k} | — | `{v}` |\n"));
        }
    }
    std::fs::write(out.join("shadcn-comparison.md"), md).map_err(|e| e.to_string())
}

// -------------------------------------------------------------- capture out

struct Sheets {
    light: RgbaImage,
    dark: RgbaImage,
    sizes: RgbaImage,
    dpi: RgbaImage,
    motion: Vec<(&'static str, RgbaImage)>,
    motion_json: serde_json::Value,
    ref1: RgbaImage,
    ref2: RgbaImage,
    ref3: RgbaImage,
    ref4: RgbaImage,
    ref5: RgbaImage,
}

fn write_capture(
    out: &Path,
    app: &App,
    sheets: Sheets,
    rep: &Report,
    ref_dir: &Path,
    dirty: bool,
) -> Result<(), String> {
    let tmp = out.with_file_name(format!(
        "{}.partial",
        out.file_name().unwrap_or_default().to_string_lossy()
    ));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(tmp.join("reference")).map_err(|e| e.to_string())?;

    let Sheets {
        light,
        dark,
        sizes,
        dpi,
        motion,
        motion_json,
        ref1,
        ref2,
        ref3,
        ref4,
        ref5,
    } = sheets;
    let mut files: Vec<String> = Vec::new();
    for (name, img) in [
        ("component-gallery-light.png", light),
        ("component-gallery-dark.png", dark),
        ("component-gallery-sizes.png", sizes),
        ("component-gallery-dpi.png", dpi),
        ("component-gallery-shadcn-reference.png", ref1),
        ("component-gallery-shadcn-reference-2.png", ref2),
        ("component-gallery-shadcn-reference-3.png", ref3),
        ("component-gallery-shadcn-reference-4.png", ref4),
        ("component-gallery-shadcn-reference-5.png", ref5),
    ]
    .into_iter()
    .chain(motion.iter().map(|(n, i)| (*n, i.clone())))
    {
        img.save(&tmp.join(name)).map_err(|e| e.to_string())?;
        files.push(name.to_string());
    }

    // reference/ = the full captured set + provenance (dev evidence only)
    let mut stack = vec![ref_dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).map_err(|e| e.to_string())? {
            let f = e.map_err(|e| e.to_string())?.path();
            if f.is_dir() {
                stack.push(f);
                continue;
            }
            let rel = f
                .strip_prefix(ref_dir)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            let dst = tmp.join("reference").join(&rel);
            if let Some(pd) = dst.parent() {
                std::fs::create_dir_all(pd).map_err(|e| e.to_string())?;
            }
            std::fs::copy(&f, &dst).map_err(|e| format!("{}: {e}", dst.display()))?;
            files.push(format!("reference/{rel}"));
        }
    }

    // component-inventory.json
    let entries: Vec<serde_json::Value> = COMPONENTS.iter().map(inventory_entry_json).collect();
    std::fs::write(
        tmp.join("component-inventory.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "spec": "docs/MASCOT_UI_COMPONENT_INVENTORY_V0.1.md",
            "tiers": {
                "A": "implemented — shown on the sheets",
                "B": "planned — listed, not implemented",
                "C": "deferred — out of scope",
            },
            "components": entries,
            "loading": {
                "status": "deferred",
                "reason": "No loader exists in the current product: Submitting swaps Send for Stop and dims the submitted text; the mascot is static.",
                "rule": "docs/MASCOT_NATIVE_UI_DESIGN_SYSTEM_V0.1.md §2 Busy / loading visual rule",
                "shadcn_reference": "https://github.com/shadcn-ui/ui/blob/db2db460a26fa84fb65c8d903b213925fbdee9ed/apps/v4/registry/new-york-v4/ui/spinner.tsx",
            },
        }))
        .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    files.push("component-inventory.json".into());

    // motion.json — durations, easings, sampled frame values, provenance
    std::fs::write(
        tmp.join("motion.json"),
        serde_json::to_string_pretty(&motion_json).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    files.push("motion.json".into());

    // shadcn-comparison.md (needs provenance.json from ref_dir)
    let prov = load_prov(ref_dir)?;
    write_comparison(&tmp, &prov, rep)?;
    files.push("shadcn-comparison.md".into());

    // receipt
    let mut file_objs = Vec::new();
    let paths: Vec<PathBuf> = files.iter().map(|f| tmp.join(f)).collect();
    let blobs = crate::capture::git_blobs(&paths);
    for ((f, p), blob) in files.iter().zip(&paths).zip(&blobs) {
        let len = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
        file_objs.push(serde_json::json!({
            "path": f,
            "bytes": len,
            "git_blob": blob,
        }));
    }
    let sheets_json: Vec<serde_json::Value> = rep
        .sheets
        .iter()
        .map(|(n, w, h)| serde_json::json!({"file": n, "width_px": w, "height_px": h}))
        .collect();
    let receipt = serde_json::json!({
        "code_head": crate::capture::git(&["rev-parse", "HEAD"]),
        "code_dirty": dirty,
        "evidence_head": serde_json::Value::Null,
        "evidence_head_rule": "Set by `mascot-ui-lab stamp-evidence` in the commit that follows the evidence commit. evidence_head is the commit that added this evidence; the stamp commit is the final branch head and changes only evidence_head fields.",
        "tool": "mascot-ui-lab components",
        "tool_version": env!("CARGO_PKG_VERSION"),
        "device": "warp",
        "os_build": crate::capture::os_build(),
        "font_family": app.painter.fonts.family,
        "painters": "mascot_ui_win32::components (production) + App::render_offscreen",
        "scale": "theme/sizes sheets 100 % (1 px = 1 DIP); comparison sheets 200 % (refs DPR 2)",
        "reference_provenance": "reference/provenance.json",
        "reference_files_used": rep.ref_files,
        "sheets": sheets_json,
        "files": file_objs,
    });
    std::fs::write(
        tmp.join("receipt.json"),
        serde_json::to_string_pretty(&receipt).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    files.push("receipt.json".into());

    if out.exists() {
        std::fs::remove_dir_all(out).map_err(|e| e.to_string())?;
    }
    std::fs::rename(&tmp, out).map_err(|e| e.to_string())?;
    println!("components: {} files -> {}", files.len(), out.display());
    Ok(())
}

fn inventory_entry_json(e: &Entry) -> serde_json::Value {
    serde_json::json!({
        "key": e.key,
        "name": e.name,
        "tier": e.tier.to_string(),
        "status": e.status.name(),
        "sizing": e.sizing.name(),
        "theme_coverage": ["light", "dark"],
        "states": e.states,
        "shadcn_url": e.shadcn.map(|s| format!("https://ui.shadcn.com/docs/components/{s}")),
        "no_analogue_reason": e.no_analogue_reason,
        "deviations": e.deviations,
        "motion": e.motion,
    })
}

// ------------------------------------------------------------------ preview

/// Non-capture mode: the sheets were already written to
/// target/mascot-ui-components/ — this shows the light sheet in a window.
fn preview(app: &mut App, sheet: &RgbaImage) -> Result<(), String> {
    use windows::Win32::UI::WindowsAndMessaging::*;
    unsafe {
        let class = w!("mascot-ui-gallery-preview");
        let _ = mascot_ui_win32::window::register_class(class, Some(preview_wndproc))
            .map_err(|e| e.to_string())?;
        let w = sheet.width.min(1400);
        let h = sheet.height.min(860);
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class,
            w!("Mascot UI component gallery"),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            w as i32 + 40,
            h as i32 + 60,
            None,
            None,
            windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
                .map(|h| Some(h.into()))
                .map_err(|e| e.to_string())?,
            None,
        )
        .map_err(|e| e.to_string())?;
        // upload sheet -> bitmap
        let bgra = sheet.to_premultiplied_bgra();
        let bmp = app
            .renderer
            .ctx
            .CreateBitmap(
                D2D_SIZE_U {
                    width: sheet.width,
                    height: sheet.height,
                },
                Some(bgra.as_ptr() as *const _),
                sheet.width * 4,
                &mascot_render_win32::renderer::bitmap_props(D2D1_BITMAP_OPTIONS(0)),
            )
            .map_err(|e| e.to_string())?;
        PREVIEW.with(|p| *p.borrow_mut() = Some(bmp));
        let mut surf = mascot_ui_win32::window::CompSurface::new(&app.renderer, hwnd, [w, h], 1.0)
            .map_err(|e| e.to_string())?;
        surf.present(&app.renderer, |ctx| {
            PREVIEW.with(|p| {
                if let Some(b) = &*p.borrow() {
                    ctx.Clear(Some(&cf0([0.93, 0.93, 0.93, 1.0])));
                    ctx.DrawBitmap(
                        &b.cast::<ID2D1Bitmap>().unwrap(),
                        None,
                        1.0,
                        D2D1_INTERPOLATION_MODE_LINEAR,
                        None,
                        None,
                    );
                }
            });
            Ok(())
        })
        .map_err(|e| e.to_string())?;
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        PREVIEW.with(|p| *p.borrow_mut() = None);
        Ok(())
    }
}

thread_local! {
    static PREVIEW: std::cell::RefCell<Option<ID2D1Bitmap1>> = const { std::cell::RefCell::new(None) };
}

unsafe extern "system" fn preview_wndproc(
    hwnd: windows::Win32::Foundation::HWND,
    msg: u32,
    w: windows::Win32::Foundation::WPARAM,
    l: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::Foundation::LRESULT;
    use windows::Win32::UI::WindowsAndMessaging::*;
    unsafe {
        match msg {
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, w, l),
        }
    }
}

// ------------------------------------------------------------- motion sheet

/// Strip timestamps (ms) — 7 frames over the 150 ms shadcn transition.
const MOTION_TIMES_MS: [f64; 7] = [0.0, 25.0, 50.0, 75.0, 100.0, 125.0, 150.0];

/// A motion-strip row: what animates, whether reduced, and the optional
/// shadcn reference strip key (reference/motion/<key>-<theme>-t<ms>.png).
#[derive(Clone, Copy)]
struct MotionRow {
    label: &'static str,
    kind: &'static str,
    reduced: bool,
    ref_key: Option<&'static str>,
}

const MOTION_ROWS: &[MotionRow] = &[
    MotionRow {
        label: "Button default idle → hover",
        kind: "button-hover",
        reduced: false,
        ref_key: Some("button-default"),
    },
    MotionRow {
        label: "IconButton primary idle → hover (Send)",
        kind: "iconbutton-primary-hover",
        reduced: false,
        ref_key: None,
    },
    MotionRow {
        label: "IconButton ghost idle → hover (Copy)",
        kind: "iconbutton-ghost-hover",
        reduced: false,
        ref_key: Some("button-ghost"),
    },
    MotionRow {
        label: "IconButton focus-visible ring 0→1",
        kind: "iconbutton-focus-ring",
        reduced: false,
        ref_key: Some("button-focus"),
    },
    MotionRow {
        label: "Tooltip open (fade+zoom-in-95+slide-in-from-bottom-2)",
        kind: "tooltip-open",
        reduced: false,
        ref_key: Some("tooltip-open"),
    },
    MotionRow {
        label: "Tooltip close (fade-out-0+zoom-out-95)",
        kind: "tooltip-close",
        reduced: false,
        ref_key: Some("tooltip-close"),
    },
    MotionRow {
        label: "Reduced motion: IconButton primary idle → hover",
        kind: "iconbutton-primary-hover",
        reduced: true,
        ref_key: None,
    },
    MotionRow {
        label: "Reduced motion: Tooltip open",
        kind: "tooltip-open",
        reduced: true,
        ref_key: None,
    },
];

/// What a motion frame cell paints: resolved colours for a control, or a
/// tooltip transform frame.
enum MotionDraw {
    Button(mascot_ui::motion::ControlColors),
    IconButton(mascot_icons::Icon, mascot_ui::motion::ControlColors),
    Tooltip(mascot_ui::motion::TooltipFrame),
}

/// Per-row fixed cell box: `(w, h)` DIP and the control's rect inside it.
fn motion_cell_box(row_kind: &str, app: &App) -> (f32, f32, Rect) {
    use mascot_ui::motion::TOOLTIP_SLIDE_DIP;
    match row_kind {
        "button-hover" => {
            let tw = app.painter.text_width("Button", TextStyle::Label);
            let s = button_size(tw, ButtonSize::Default);
            (s.w + 24.0, s.h + 24.0, Rect::new(12.0, 12.0, s.w, s.h))
        }
        "iconbutton-primary-hover" | "iconbutton-ghost-hover" | "iconbutton-focus-ring" => {
            let e = tokens::PRIMARY_BUTTON;
            (e + 24.0, e + 24.0, Rect::new(12.0, 12.0, e, e))
        }
        _ => {
            // tooltip: bottom headroom for the +8 DIP enter slide
            let tw = app.painter.text_width("Send", TextStyle::Muted);
            let s = tooltip_size(tw);
            (
                s.w + 8.0,
                s.h + TOOLTIP_SLIDE_DIP + 8.0,
                Rect::new(4.0, 4.0, s.w, s.h),
            )
        }
    }
}

/// Resolve what frame `t` draws for a row (and its `ControlColors` — for
/// tooltip rows `ring` carries the frame opacity so the JSON has one shape).
fn motion_frame(
    row: &MotionRow,
    pal: &Palette,
    t: f64,
) -> (MotionDraw, mascot_ui::motion::ControlColors) {
    use mascot_ui::component::ControlVisual;
    use mascot_ui::motion::{TOOLTIP_IDENTITY, Tween, tooltip_close_frame, tooltip_open_frame};
    let idle = ControlVisual::default();
    let hover = ControlVisual {
        hover: true,
        ..Default::default()
    };
    let focus = ControlVisual {
        focus_visible: true,
        ..Default::default()
    };
    let reduced = row.reduced;
    match row.kind {
        "button-hover" => {
            let mut tw = Tween::settled(button_paint(pal, ButtonVariant::Default, idle));
            tw.retarget(
                button_paint(pal, ButtonVariant::Default, hover),
                0.0,
                reduced,
            );
            let c = tw.value(t, reduced);
            (MotionDraw::Button(c), c)
        }
        "iconbutton-primary-hover" | "iconbutton-ghost-hover" | "iconbutton-focus-ring" => {
            let (kind, to_v) = match row.kind {
                "iconbutton-primary-hover" => (IconButtonKind::Primary, hover),
                "iconbutton-ghost-hover" => (IconButtonKind::Ghost, hover),
                _ => (IconButtonKind::Primary, focus),
            };
            let icon = if kind == IconButtonKind::Primary {
                mascot_icons::Icon::ArrowUp
            } else {
                mascot_icons::Icon::Copy
            };
            let mut tw = Tween::settled(icon_button_paint(pal, kind, idle));
            tw.retarget(icon_button_paint(pal, kind, to_v), 0.0, reduced);
            let c = tw.value(t, reduced);
            (MotionDraw::IconButton(icon, c), c)
        }
        "tooltip-open" | "tooltip-close" => {
            let f = match (row.kind, reduced) {
                ("tooltip-open", false) => tooltip_open_frame(t),
                ("tooltip-open", true) => TOOLTIP_IDENTITY,
                ("tooltip-close", false) => tooltip_close_frame(TOOLTIP_IDENTITY, t),
                _ => TooltipFrame {
                    opacity: 0.0,
                    scale: mascot_ui::motion::TOOLTIP_SCALE_FROM,
                    dy: 0.0,
                },
            };
            let (fill, fg) = tooltip_colors(pal);
            (
                MotionDraw::Tooltip(f),
                mascot_ui::motion::ControlColors {
                    fill,
                    fg,
                    ring: f.opacity,
                },
            )
        }
        _ => unreachable!(),
    }
}

use mascot_ui::motion::TooltipFrame;

/// Render one frame cell (flattened over the band surface).
fn motion_frame_cell(
    app: &App,
    pal: &Palette,
    row: &MotionRow,
    t: f64,
) -> Result<RgbaImage, String> {
    let (w, h, rect) = motion_cell_box(row.kind, app);
    let (draw, _c) = motion_frame(row, pal, t);
    cell_img(app, pal, w, h, 2.0, |ctx| match draw {
        MotionDraw::Button(c) => app.painter.button(ctx, pal, rect, "Button", c),
        MotionDraw::IconButton(icon, c) => app.painter.icon_button(ctx, pal, rect, icon, c),
        MotionDraw::Tooltip(f) => app.painter.tooltip_with(ctx, pal, rect, "Send", f),
    })
}

/// The static endpoint cell (idle / target) drawn through the same box —
/// the byte-identity reference for the strip self-checks.
fn motion_static_cell(
    app: &App,
    pal: &Palette,
    row: &MotionRow,
    target: bool,
) -> Result<RgbaImage, String> {
    use mascot_ui::component::ControlVisual;
    use mascot_ui::motion::TOOLTIP_IDENTITY;
    let (w, h, rect) = motion_cell_box(row.kind, app);
    let idle = ControlVisual::default();
    let hover = ControlVisual {
        hover: true,
        ..Default::default()
    };
    let focus = ControlVisual {
        focus_visible: true,
        ..Default::default()
    };
    cell_img(app, pal, w, h, 2.0, |ctx| match row.kind {
        "button-hover" => app.painter.button(
            ctx,
            pal,
            rect,
            "Button",
            button_paint(
                pal,
                ButtonVariant::Default,
                if target { hover } else { idle },
            ),
        ),
        "tooltip-open" => {
            if target {
                app.painter
                    .tooltip_with(ctx, pal, rect, "Send", TOOLTIP_IDENTITY)
            } else {
                Ok(()) // t=0: opacity 0 — empty cell
            }
        }
        "tooltip-close" => {
            if target {
                Ok(()) // t=150: opacity 0 — empty cell
            } else {
                app.painter
                    .tooltip_with(ctx, pal, rect, "Send", TOOLTIP_IDENTITY)
            }
        }
        _ => {
            let (kind, v) = match row.kind {
                "iconbutton-primary-hover" => {
                    (IconButtonKind::Primary, if target { hover } else { idle })
                }
                "iconbutton-ghost-hover" => {
                    (IconButtonKind::Ghost, if target { hover } else { idle })
                }
                _ => (IconButtonKind::Primary, if target { focus } else { idle }),
            };
            let icon = if kind == IconButtonKind::Primary {
                mascot_icons::Icon::ArrowUp
            } else {
                mascot_icons::Icon::Copy
            };
            app.painter
                .icon_button(ctx, pal, rect, icon, icon_button_paint(pal, kind, v))
        }
    })
}

fn hex_rgba(c: [f32; 4]) -> String {
    format!(
        "#{:02X}{:02X}{:02X}{:02X}",
        (c[0] * 255.0).round() as u32,
        (c[1] * 255.0).round() as u32,
        (c[2] * 255.0).round() as u32,
        (c[3] * 255.0).round() as u32
    )
}

/// Ink metrics for a strip frame vs the empty and full cells:
/// (coverage = pixels differing from empty, weight = mean per-pixel
/// progress toward the full cell across the pixels that the full cell
/// changes — i.e. opacity progress in the pill region).
fn strip_ink(frame: &RgbaImage, empty: &RgbaImage, full: &RgbaImage) -> (usize, f64) {
    let mut cov = 0usize;
    let (mut num, mut den) = (0.0f64, 0.0f64);
    for i in (0..frame.data.len()).step_by(4) {
        let (p, e, f) = (
            &frame.data[i..i + 4],
            &empty.data[i..i + 4],
            &full.data[i..i + 4],
        );
        let df: f64 = (0..4).map(|c| (f[c] as f64 - e[c] as f64).abs()).sum();
        if df > 0.5 {
            // inside the pill region: coverage = this px carries ink now
            if p != e {
                cov += 1;
            }
            let dp: f64 = (0..4).map(|c| (p[c] as f64 - e[c] as f64).abs()).sum();
            num += (dp / df).min(1.0);
            den += 1.0;
        }
    }
    (cov, if den > 0.0 { num / den } else { 0.0 })
}

/// One composed strip: row label, optional ref strip, native frames + caps.
struct MotionStrip {
    label: &'static str,
    kind: &'static str,
    reduced: bool,
    theme_name: &'static str,
    ref_key: Option<&'static str>,
    label_img: RgbaImage,
    ref_label: Option<RgbaImage>,
    ref_imgs: Vec<RgbaImage>,
    native_label: RgbaImage,
    frames: Vec<RgbaImage>,
    caps: Vec<RgbaImage>,
    fjson: Vec<serde_json::Value>,
    /// placement in the final sheet, filled during composition
    sheet_px: Option<(u32, u32, u32, u32)>,
    frames_px: Vec<(u32, u32, u32, u32)>,
    ref_frames_px: Vec<(u32, u32, u32, u32)>,
}

/// (file name, image) per motion sheet, plus the motion.json document.
type MotionSheets = (Vec<(&'static str, RgbaImage)>, serde_json::Value);

/// Build the motion sheets: `component-gallery-motion.png` (rows 1-4) and
/// `component-gallery-motion-2.png` (rows 5-8); per sheet a light band then a
/// dark band; per row a 7-frame strip (t = 0..150 ms) with timestamp captions;
/// when a shadcn motion reference strip exists it is drawn directly above the
/// native strip with the same captions. Returns the sheets and motion.json.
fn motion_sheet(app: &mut App, ref_dir: &Path) -> Result<MotionSheets, String> {
    use mascot_ui::motion::{TOOLTIP_ANIM_MS, TRANSITION_MS};
    const PAD: u32 = 24;
    const GAP: u32 = 8;
    const ROW_GAP: u32 = 18;
    const CAP_GAP: u32 = 4;
    const COLS: usize = 2; // strips per visual row
    const SHEET_FILES: [&str; 2] = [
        "component-gallery-motion.png",
        "component-gallery-motion-2.png",
    ];

    // ---- build strips ----------------------------------------------------
    // bands: (sheet index, theme, strips)
    let mut bands: Vec<(usize, Theme, Vec<MotionStrip>)> = Vec::new();
    for (si, lo) in [(0usize, 0usize), (1usize, 4usize)] {
        for theme in [Theme::Light, Theme::Dark] {
            let pal = theme.palette();
            let theme_name = if theme == Theme::Light {
                "light"
            } else {
                "dark"
            };
            let mut strips: Vec<MotionStrip> = Vec::new();
            for row in &MOTION_ROWS[lo..lo + 4] {
                let mut frames = Vec::new();
                let mut fjson = Vec::new();
                for t in MOTION_TIMES_MS {
                    let img = motion_frame_cell(app, &pal, row, t)?;
                    let (draw, c) = motion_frame(row, &pal, t);
                    let p = match row.kind {
                        "tooltip-open" | "tooltip-close" => {
                            mascot_ui::motion::ease_css((t / TOOLTIP_ANIM_MS).clamp(0.0, 1.0) as f32)
                        }
                        _ => mascot_ui::motion::ease_standard(
                            (t / TRANSITION_MS).clamp(0.0, 1.0) as f32
                        ),
                    };
                    let mut fj = serde_json::json!({
                        "t_ms": t,
                        "eased_p": p,
                        "fill": hex_rgba(c.fill),
                        "fg": hex_rgba(c.fg),
                        "ring": c.ring,
                    });
                    if let MotionDraw::Tooltip(f) = draw {
                        fj["opacity"] = serde_json::json!(f.opacity);
                        fj["scale"] = serde_json::json!(f.scale);
                        fj["dy"] = serde_json::json!(f.dy);
                    }
                    fjson.push(fj);
                    frames.push(img);
                }
                // ---- self-checks (fail the capture) ----------------------
                let idle_cell = motion_static_cell(app, &pal, row, false)?;
                let target_cell = motion_static_cell(app, &pal, row, true)?;
                if !row.reduced && frames[0].data != idle_cell.data {
                    return Err(format!(
                        "motion strip '{}': t=0 frame not byte-identical to the static idle cell",
                        row.label
                    ));
                }
                if frames[6].data != target_cell.data {
                    return Err(format!(
                        "motion strip '{}': t=150 frame not byte-identical to the static target cell",
                        row.label
                    ));
                }
                if row.reduced {
                    for (i, f) in frames.iter().enumerate() {
                        if f.data != frames[6].data {
                            return Err(format!(
                                "reduced-motion strip '{}': frame {i} differs from the final frame",
                                row.label
                            ));
                        }
                    }
                }
                // tooltip strips must ACTUALLY draw: coverage and
                // alpha-weighted ink progress in the right direction (the
                // R1 layer bug drew nothing for every non-identity frame)
                if matches!(row.kind, "tooltip-open" | "tooltip-close") && !row.reduced {
                    // (no-ink cell, full-ink cell): for open the idle cell is
                    // empty; for close the TARGET cell is empty
                    let (empty, full) = if row.kind == "tooltip-open" {
                        (&idle_cell, &target_cell)
                    } else {
                        (&target_cell, &idle_cell)
                    };
                    let mut inks = Vec::new();
                    for f in &frames {
                        inks.push(strip_ink(f, empty, full));
                    }
                    let (dir, dir_name) = if row.kind == "tooltip-open" {
                        (1.0f64, "increasing")
                    } else {
                        (-1.0f64, "decreasing")
                    };
                    for (i, (cov, w)) in inks.iter().enumerate() {
                        let t = MOTION_TIMES_MS[i];
                        let need_ink = if row.kind == "tooltip-open" {
                            t >= 25.0
                        } else {
                            t <= 125.0
                        };
                        if need_ink && *cov == 0 {
                            return Err(format!(
                                "motion strip '{}': t={t} has zero ink (animation invisible)",
                                row.label
                            ));
                        }
                        if i > 0 {
                            let (pc, pw) = inks[i - 1];
                            // coverage must not move against the direction
                            if dir > 0.0 && *cov < pc || dir < 0.0 && *cov > pc {
                                return Err(format!(
                                    "motion strip '{}': coverage not {dir_name} at t={t}",
                                    row.label
                                ));
                            }
                            // alpha-weighted ink strictly progresses while
                            // opacity is between 0 and 1 (t=25..125)
                            if (25.0..=125.0).contains(&t) {
                                let dw = (*w - pw) * dir;
                                if dw <= 0.0 {
                                    return Err(format!(
                                        "motion strip '{}': ink weight not strictly {dir_name} at t={t} ({pw:.3}->{w:.3})",
                                        row.label
                                    ));
                                }
                            }
                        }
                    }
                }
                // monotonic: per channel the series must never reverse
                // direction (idle -> target for hover/focus rows)
                if !row.reduced && !matches!(row.kind, "tooltip-open" | "tooltip-close") {
                    let (_, to_c) = motion_frame(row, &pal, MOTION_TIMES_MS[6]);
                    let (_, fr_c) = motion_frame(row, &pal, MOTION_TIMES_MS[0]);
                    for t in &MOTION_TIMES_MS[1..6] {
                        let (_, c) = motion_frame(row, &pal, *t);
                        for k in 0..4 {
                            let lo = fr_c.fill[k].min(to_c.fill[k]);
                            let hi = fr_c.fill[k].max(to_c.fill[k]);
                            if c.fill[k] < lo - 1e-4 || c.fill[k] > hi + 1e-4 {
                                return Err(format!(
                                    "motion strip '{}': fill[{k}] at t={t} outside idle..target",
                                    row.label
                                ));
                            }
                        }
                    }
                }
                let label_img = text_img_alpha(
                    app,
                    row.label,
                    &app.painter.fonts.label,
                    pal.foreground,
                    1.0,
                )?;
                let caps: Vec<RgbaImage> = MOTION_TIMES_MS
                    .iter()
                    .map(|t| {
                        text_img_alpha(
                            app,
                            &format!("t={} ms", *t as i32),
                            &app.painter.fonts.caption,
                            pal.muted_fg,
                            1.0,
                        )
                    })
                    .collect::<Result<_, String>>()?;
                let (ref_label, ref_imgs) = match row.ref_key {
                    Some(key) => {
                        let mut imgs = Vec::new();
                        for t in MOTION_TIMES_MS {
                            let p = ref_dir
                                .join("motion")
                                .join(format!("{key}-{theme_name}-t{}.png", t as i32));
                            if p.exists() {
                                imgs.push(RgbaImage::load(&p)?);
                            }
                        }
                        if imgs.len() == MOTION_TIMES_MS.len() {
                            (
                                Some(text_img_alpha(
                                    app,
                                    "shadcn reference (200%)",
                                    &app.painter.fonts.caption,
                                    pal.muted_fg,
                                    1.0,
                                )?),
                                imgs,
                            )
                        } else {
                            (None, Vec::new())
                        }
                    }
                    None => (None, Vec::new()),
                };
                strips.push(MotionStrip {
                    label: row.label,
                    kind: row.kind,
                    reduced: row.reduced,
                    theme_name,
                    ref_key: row.ref_key,
                    label_img,
                    ref_label,
                    ref_imgs,
                    native_label: text_img_alpha(
                        app,
                        "native (200%)",
                        &app.painter.fonts.caption,
                        pal.muted_fg,
                        1.0,
                    )?,
                    frames,
                    caps,
                    fjson,
                    sheet_px: None,
                    frames_px: Vec::new(),
                    ref_frames_px: Vec::new(),
                });
            }
            bands.push((si, theme, strips));
        }
    }

    // ---- compose ---------------------------------------------------------
    // wrapped width of a line of `n` cells (4 per line)
    let line_w = |cell: u32, n: usize| {
        cell * (n.min(4) as u32) + GAP * ((n.min(4) as u32).saturating_sub(1))
    };
    let cap_h = |s: &MotionStrip| s.caps.first().map(|c| c.height).unwrap_or(0);
    let strip_h = |s: &MotionStrip| -> u32 {
        let mut h = s.label_img.height + 6;
        if let Some(rl) = &s.ref_label {
            let rlines = s.ref_imgs.len().div_ceil(4) as u32;
            h += rl.height
                + 4
                + rlines * (s.ref_imgs[0].height + CAP_GAP + cap_h(s))
                + (rlines - 1) * CAP_GAP
                + 8;
        }
        let lines = s.frames.len().div_ceil(4) as u32;
        h + s.native_label.height
            + 4
            + lines * (s.frames.first().map(|f| f.height).unwrap_or(0) + CAP_GAP + cap_h(s))
            + (lines - 1) * CAP_GAP
    };
    let strip_w = |s: &MotionStrip| -> u32 {
        let native = s
            .frames
            .first()
            .map(|f| line_w(f.width, s.frames.len()))
            .unwrap_or(0);
        let refs = s
            .ref_imgs
            .first()
            .map(|r| line_w(r.width, s.ref_imgs.len()))
            .unwrap_or(0);
        native.max(refs).max(s.label_img.width)
    };

    const COL_GAP: u32 = 48;
    let has_ink = |img: &RgbaImage| -> bool {
        // "blank" = uniform (every px equal to the first px)
        let first = &img.data[..4];
        img.data.chunks(4).skip(1).any(|p| p != first)
    };

    let mut sheets_out: Vec<(&'static str, RgbaImage)> = Vec::new();
    let mut rows_json = Vec::new();
    for (si, &sheet_file) in SHEET_FILES.iter().enumerate() {
        let max_strip_w = bands
            .iter()
            .filter(|(i, _, _)| *i == si)
            .flat_map(|(_, _, ss)| ss.iter().map(strip_w))
            .max()
            .unwrap_or(0);
        let band_w = 2 * PAD + max_strip_w * COLS as u32 + COL_GAP;
        // compose light band then dark band; sheet_y offsets band-local
        // coords into the stacked sheet
        let mut sheet_y = 0u32;
        let mut band_imgs: Vec<RgbaImage> = Vec::new();
        for (_, theme, strips) in bands.iter_mut().filter(|(i, _, _)| *i == si) {
            let pal = theme.palette();
            let bg = rgb8(pal.surface);
            let row_hs: Vec<u32> = strips
                .chunks(COLS)
                .map(|pair| pair.iter().map(strip_h).max().unwrap_or(0))
                .collect();
            let band_h: u32 = PAD + row_hs.iter().map(|h| h + ROW_GAP).sum::<u32>();
            let mut band = RgbaImage::new(band_w, band_h);
            band.fill_rect(0, 0, band_w, band_h, [bg[0], bg[1], bg[2], 255]);
            let mut y = PAD;
            for pair in strips.chunks_mut(COLS) {
                let mut pair_h = 0;
                for (i, s) in pair.iter_mut().enumerate() {
                    let x0 = PAD + i as u32 * (max_strip_w + COL_GAP);
                    let strip_top = y;
                    let mut sy = y;
                    band.blend_over(&s.label_img, x0, sy);
                    sy += s.label_img.height + 6;
                    if let Some(rl) = &s.ref_label {
                        band.blend_over(rl, x0, sy);
                        sy += rl.height + 4;
                        let rcell_w = s.ref_imgs[0].width;
                        let rline_h = s.ref_imgs[0].height + CAP_GAP + cap_h(s);
                        for (li, r) in s.ref_imgs.iter().enumerate() {
                            let x = x0 + (li % 4) as u32 * (rcell_w + GAP);
                            let ly = sy + (li / 4) as u32 * (rline_h + CAP_GAP);
                            band.blend_over(r, x, ly);
                            band.blend_over(&s.caps[li], x, ly + r.height + CAP_GAP);
                            s.ref_frames_px.push((x, sheet_y + ly, r.width, r.height));
                        }
                        let rlines = s.ref_imgs.len().div_ceil(4) as u32;
                        sy += rlines * rline_h + (rlines - 1) * CAP_GAP + 8;
                    }
                    band.blend_over(&s.native_label, x0, sy);
                    sy += s.native_label.height + 4;
                    let cell_w = s.frames[0].width;
                    let line_h = s.frames[0].height + CAP_GAP + cap_h(s);
                    for (li, (f, c)) in s.frames.iter().zip(&s.caps).enumerate() {
                        let x = x0 + (li % 4) as u32 * (cell_w + GAP);
                        let ly = sy + (li / 4) as u32 * (line_h + CAP_GAP);
                        band.blend_over(f, x, ly);
                        band.blend_over(c, x, ly + f.height + CAP_GAP);
                        s.frames_px.push((x, sheet_y + ly, f.width, f.height));
                    }
                    let lines = s.frames.len().div_ceil(4) as u32;
                    sy += lines * line_h + (lines - 1) * CAP_GAP;
                    let strip_h_here = sy - strip_top;
                    s.sheet_px = Some((x0, sheet_y + strip_top, strip_w(s), strip_h_here));
                    pair_h = pair_h.max(strip_h_here);

                    // not blank: at least one frame or ref frame carries ink
                    if !s.frames.iter().chain(s.ref_imgs.iter()).any(&has_ink) {
                        return Err(format!(
                            "motion strip '{}' ({}) placed on {} but is entirely blank",
                            s.label, s.theme_name, sheet_file
                        ));
                    }
                }
                y += pair_h + ROW_GAP;
            }
            band_imgs.push(band);
            sheet_y += band_h;
        }
        let w = band_w;
        let h: u32 = band_imgs.iter().map(|b| b.height).sum();
        if w > MAX_SHEET_W || h > MAX_SHEET_H {
            return Err(format!(
                "motion sheet {w}x{h} over {MAX_SHEET_W}x{MAX_SHEET_H}"
            ));
        }
        let mut sheet = RgbaImage::new(w, h);
        let mut yy = 0u32;
        for b in &band_imgs {
            sheet.blend_over(b, 0, yy);
            yy += b.height;
        }
        // placement self-check: every recorded rect inside the sheet bounds
        for (_, _, strips) in bands.iter().filter(|(i, _, _)| *i == si) {
            for s in strips {
                let (rx, ry, rw, rh) = s.sheet_px.unwrap_or((0, 0, 0, 0));
                if rx + rw > w || ry + rh > h {
                    return Err(format!(
                        "motion strip '{}' rect {:?} escapes sheet {} ({}x{})",
                        s.label, s.sheet_px, sheet_file, w, h
                    ));
                }
                for fr in s.frames_px.iter().chain(s.ref_frames_px.iter()) {
                    if fr.0 + fr.2 > w || fr.1 + fr.3 > h {
                        return Err(format!(
                            "motion strip '{}' frame rect {:?} escapes sheet {}",
                            s.label, fr, sheet_file
                        ));
                    }
                }
                rows_json.push(serde_json::json!({
                    "label": s.label,
                    "kind": s.kind,
                    "theme": s.theme_name,
                    "reduced": s.reduced,
                    "sheet": sheet_file,
                    "rect_px": s.sheet_px.map(|r| [r.0, r.1, r.2, r.3]),
                    "frames_px": s.frames_px.iter().map(|r| [r.0, r.1, r.2, r.3]).collect::<Vec<_>>(),
                    "ref_frames_px": s.ref_frames_px.iter().map(|r| [r.0, r.1, r.2, r.3]).collect::<Vec<_>>(),
                    "reference_strip": s.ref_key,
                    "frames": s.fjson,
                }));
            }
        }
        sheets_out.push((sheet_file, sheet));
    }

    // ---- motion.json -----------------------------------------------------
    let mut provenance = serde_json::json!({
        "shadcn_commit": "db2db460a26fa84fb65c8d903b213925fbdee9ed",
        "button": "https://github.com/shadcn-ui/ui/blob/db2db460a26fa84fb65c8d903b213925fbdee9ed/apps/v4/registry/new-york-v4/ui/button.tsx",
        "textarea": "https://github.com/shadcn-ui/ui/blob/db2db460a26fa84fb65c8d903b213925fbdee9ed/apps/v4/registry/new-york-v4/ui/textarea.tsx",
        "tooltip": "https://github.com/shadcn-ui/ui/blob/db2db460a26fa84fb65c8d903b213925fbdee9ed/apps/v4/registry/new-york-v4/ui/tooltip.tsx",
        "tw_animate_css": "1.4.0",
        "tailwind": "v4 (--default-transition-duration 150ms, --default-transition-timing-function cubic-bezier(0.4,0,0.2,1))",
    });
    let mprov = ref_dir.join("motion-provenance.json");
    if mprov.exists() {
        provenance["reference_computed_styles"] =
            serde_json::from_str(&std::fs::read_to_string(&mprov).map_err(|e| e.to_string())?)
                .unwrap_or(serde_json::Value::Null);
    }
    let doc = serde_json::json!({
        "durations_ms": { "transition": TRANSITION_MS, "tooltip": TOOLTIP_ANIM_MS },
        "easing": {
            "transition": { "name": "ease_standard", "bezier": [0.4, 0.0, 0.2, 1.0] },
            "tooltip": { "name": "ease_css (tw-animate-css)", "bezier": [0.25, 0.1, 0.25, 1.0] },
        },
        "tooltip": {
            "scale_from": mascot_ui::motion::TOOLTIP_SCALE_FROM,
            "slide_dip": mascot_ui::motion::TOOLTIP_SLIDE_DIP,
            "transform_origin": "bottom centre of the pill",
        },
        "rows": rows_json,
        "provenance": provenance,
    });
    Ok((sheets_out, doc))
}
