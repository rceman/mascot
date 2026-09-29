//! `capture --out DIR [--allow-dirty]`: deterministic offscreen evidence.
//!
//! WARP device, same painter path as the live window. Writes into
//! `DIR.partial` and only renames to `DIR` on full success so a failed run
//! never leaves a half-written evidence set.

use mascot_render_win32::image::RgbaImage;
use mascot_render_win32::renderer::{DeviceKind, Renderer};
use mascot_ui::theme::{Theme, tokens};
use mascot_ui_win32::app::App;
use std::path::{Path, PathBuf};

use crate::presets;

pub fn run() -> Result<(), String> {
    let out = crate::get_arg("--out")
        .map(PathBuf::from)
        .ok_or("capture needs --out DIR")?;
    let allow_dirty = crate::has_flag("--allow-dirty");
    let dirty = git_dirty();
    if dirty && !allow_dirty {
        return Err("working tree is dirty; pass --allow-dirty for dev iterations".into());
    }

    let tmp = out.with_file_name(format!(
        "{}.partial",
        out.file_name().unwrap_or_default().to_string_lossy()
    ));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(tmp.join("states")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(tmp.join("zoomed")).map_err(|e| e.to_string())?;

    let rig = crate::find_rig()?;
    let mut app = App::new(DeviceKind::Warp, rig.clone(), Default::default(), 1.0)
        .map_err(|e| format!("app: {e}"))?;
    app.response_delay_ms = 0;

    let mut files: Vec<String> = Vec::new();
    let mut sheets: Vec<(String, Vec<(String, RgbaImage)>)> = Vec::new();

    for theme in [Theme::Light, Theme::Dark] {
        let bg: [u8; 3] = match theme {
            Theme::Light => [0xED, 0xED, 0xED],
            Theme::Dark => [0x1F, 0x1F, 0x1F],
        };
        let mut cells: Vec<(String, RgbaImage)> = Vec::new();
        for p in presets::PRESETS {
            presets::apply(&mut app, p.id, false);
            app.state.theme = theme;
            app.apply_theme();
            let img = app
                .render_offscreen()
                .map_err(|e| format!("render {}: {e}", p.id))?;
            let flat = img.over(bg);
            let name = format!("{}-{}.png", theme.name(), p.id);
            flat.save(&tmp.join("states").join(&name))
                .map_err(|e| e.to_string())?;
            files.push(format!("states/{name}"));
            cells.push((p.id.to_string(), flat.clone()));
            // 3x zoomed crop of the composition bounds (bubble ∪ mascot)
            save_zoomed(
                &app,
                &img,
                &tmp.join("zoomed")
                    .join(format!("{}-{}-3x.png", theme.name(), p.id)),
                3,
            )
            .map_err(|e| e.to_string())?;
            files.push(format!("zoomed/{}-{}-3x.png", theme.name(), p.id));

            if presets::RIGHT_VARIANTS.contains(&p.id) {
                presets::apply(&mut app, p.id, true);
                let img = app
                    .render_offscreen()
                    .map_err(|e| format!("render {}-right: {e}", p.id))?;
                let flat = img.over(bg);
                let name = format!("{}-{}-right.png", theme.name(), p.id);
                flat.save(&tmp.join("states").join(&name))
                    .map_err(|e| e.to_string())?;
                files.push(format!("states/{name}"));
                cells.push((format!("{}-right", p.id), flat));
                save_zoomed(
                    &app,
                    &img,
                    &tmp.join("zoomed")
                        .join(format!("{}-{}-right-3x.png", theme.name(), p.id)),
                    3,
                )
                .map_err(|e| e.to_string())?;
                files.push(format!("zoomed/{}-{}-right-3x.png", theme.name(), p.id));
                presets::apply(&mut app, p.id, false); // reset
                app.state.theme = theme;
                app.apply_theme();
            }
        }
        sheets.push((theme.name().to_string(), cells));
    }

    // contact sheets: every cell padded to a common canvas so compose_sheet's
    // fit factor is exactly 1 — no per-state resampling. Light sheet
    // background both ways: state cells already carry their own backdrop and
    // compose_sheet's labels are dark text.
    let pad_w = sheets
        .iter()
        .flat_map(|(_, c)| c.iter().map(|(_, i)| i.width))
        .max()
        .unwrap_or(1);
    let pad_h = sheets
        .iter()
        .flat_map(|(_, c)| c.iter().map(|(_, i)| i.height))
        .max()
        .unwrap_or(1);
    for (tname, cells) in sheets {
        let cell_bg: [u8; 3] = match tname.as_str() {
            "light" => [0xED, 0xED, 0xED],
            _ => [0x1F, 0x1F, 0x1F],
        };
        let padded: Vec<(String, RgbaImage)> = cells
            .iter()
            .map(|(l, i)| (l.clone(), pad_bottom(i, pad_w, pad_h, cell_bg)))
            .collect();
        let refs: Vec<(String, &RgbaImage)> = padded.iter().map(|(l, i)| (l.clone(), i)).collect();
        let sheet = app
            .renderer
            .compose_sheet(refs.as_slice(), 4, [pad_w, pad_h], 22, [0.93, 0.93, 0.93])
            .map_err(|e| format!("sheet {tname}: {e}"))?;
        let name = format!("contact-sheet-{tname}.png");
        sheet.save(&tmp.join(&name)).map_err(|e| e.to_string())?;
        files.push(name);
    }

    // dpi sheet: {composer-multiline, response, send-focus-visible} x scales x themes
    let dpi = dpi_sheet(&mut app, &rig, &tmp)?;
    dpi.save(&tmp.join("dpi-contact-sheet.png"))
        .map_err(|e| e.to_string())?;
    files.push("dpi-contact-sheet.png".into());

    // icon sheet
    let icons = icon_sheet(&app.renderer)?;
    icons
        .save(&tmp.join("icon-sheet.png"))
        .map_err(|e| e.to_string())?;
    files.push("icon-sheet.png".into());

    // receipt
    write_receipt(&tmp, &files, &app)?;

    if out.exists() {
        std::fs::remove_dir_all(&out).map_err(|e| e.to_string())?;
    }
    std::fs::rename(&tmp, &out).map_err(|e| e.to_string())?;
    println!("capture: {} files -> {}", files.len() + 1, out.display());
    Ok(())
}

pub(crate) fn git_dirty() -> bool {
    let repo = repo_root();
    std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&repo)
        .output()
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(true)
}

pub(crate) fn repo_root() -> PathBuf {
    std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| PathBuf::from(s.trim()))
        .unwrap_or_else(|| PathBuf::from("."))
}

pub(crate) fn git(args: &[&str]) -> String {
    std::process::Command::new("git")
        .args(args)
        .current_dir(repo_root())
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// Pads `img` onto a `w`x`h` canvas, bottom-centre anchored (windows anchor
/// their bottom edge), filled with `bg` so flattened cells stay seamless.
fn pad_bottom(img: &RgbaImage, w: u32, h: u32, bg: [u8; 3]) -> RgbaImage {
    if img.width == w && img.height == h {
        return img.clone();
    }
    let mut out = RgbaImage::new(w.max(img.width), h.max(img.height));
    out.fill_rect(0, 0, out.width, out.height, [bg[0], bg[1], bg[2], 255]);
    let x = (out.width - img.width) / 2;
    let y = out.height - img.height; // bottom-anchored
    out.blit(img, x, y);
    out
}

/// Renders a small text label into an `RgbaImage` (DWrite via the renderer's
/// context, at 96 DPI = pixel units).
fn text_bmp(app: &App, text: &str, w: u32, h: u32, color: [f32; 4]) -> Result<RgbaImage, String> {
    use windows::Win32::Graphics::Direct2D::Common::*;
    use windows::Win32::Graphics::Direct2D::*;
    use windows::core::Interface;
    let r = &app.renderer;
    let ctx = &r.ctx;
    let bmp = r.create_target_bitmap([w, h]).map_err(|e| e.to_string())?;
    unsafe {
        let img: ID2D1Image = bmp.cast().map_err(|e| e.to_string())?;
        ctx.SetTarget(&img);
        ctx.SetDpi(96.0, 96.0);
        ctx.BeginDraw();
        ctx.Clear(Some(&D2D1_COLOR_F {
            r: 0.93,
            g: 0.93,
            b: 0.93,
            a: 1.0,
        }));
        let brush = ctx
            .CreateSolidColorBrush(
                &D2D1_COLOR_F {
                    r: color[0],
                    g: color[1],
                    b: color[2],
                    a: color[3],
                },
                None,
            )
            .map_err(|e| e.to_string())?;
        let text16: Vec<u16> = text.encode_utf16().collect();
        ctx.DrawText(
            &text16,
            &app.painter.fonts.small,
            &D2D_RECT_F {
                left: 4.0,
                top: 2.0,
                right: w as f32,
                bottom: h as f32,
            },
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
            windows::Win32::Graphics::DirectWrite::DWRITE_MEASURING_MODE_NATURAL,
        );
        ctx.EndDraw(None, None).map_err(|e| e.to_string())?;
        ctx.SetTarget(None);
        ctx.SetDpi(96.0, 96.0);
    }
    r.read_back(&bmp, [w, h]).map_err(|e| e.to_string())
}

/// Crop rect covering the whole composition — bubble ∪ mascot ∪ tooltip —
/// plus a small margin, converted to device px and clamped to the image.
/// Replaces the old fixed crop that cut off the bubble's left edge and Send.
fn composition_crop(app: &App, margin_dip: f32) -> (u32, u32, u32, u32) {
    let lay = &app.layout;
    let mut r = lay.mascot;
    if let Some(b) = lay.bubble {
        r = r.union(b);
    }
    if let Some(t) = lay.tooltip {
        r = r.union(t);
    }
    let r = r.grow(margin_dip);
    // clamp to the window rect so the crop never exceeds the bitmap
    let w = r.x.max(lay.window.x).max(0.0);
    let t = r.y.max(lay.window.y).max(0.0);
    let rr = r.right().min(lay.window.right());
    let bb = r.bottom().min(lay.window.bottom());
    let s = app.scale;
    (
        (w * s).round() as u32,
        (t * s).round() as u32,
        ((rr - w) * s).round().max(1.0) as u32,
        ((bb - t) * s).round().max(1.0) as u32,
    )
}

/// Writes `img` cropped to the composition bounds, nearest-zoomed by `zoom`.
fn save_zoomed(app: &App, img: &RgbaImage, path: &Path, zoom: u32) -> Result<(), String> {
    let (x, y, w, h) = composition_crop(app, 6.0);
    let x = x.min(img.width - 1);
    let y = y.min(img.height - 1);
    let w = w.min(img.width - x).max(1);
    let h = h.min(img.height - y).max(1);
    let crop = img.crop(x, y, w, h);
    let mut out = RgbaImage::new(crop.width * zoom, crop.height * zoom);
    for dy in 0..crop.height * zoom {
        for dx in 0..crop.width * zoom {
            let so = (((dy / zoom) * crop.width + dx / zoom) * 4) as usize;
            let oo = ((dy * out.width + dx) * 4) as usize;
            out.data[oo..oo + 4].copy_from_slice(&crop.data[so..so + 4]);
        }
    }
    out.save(path)
}

/// Vertical extent (first..last row) of pixels in `img[rect]` that deviate
/// from `bg` — i.e. the drawn text's ink span in device px. Counting
/// contiguous bands is unreliable (diacritics above the cap line split a
/// text line into two bands); the span directly measures "is the text the
/// right size and fully inside the rect".
fn ink_span(
    img: &RgbaImage,
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
    bg: [u8; 3],
) -> Option<(u32, u32)> {
    let mut first = None;
    let mut last = 0u32;
    for y in y0..y1.min(img.height) {
        for x in x0..x1.min(img.width) {
            let o = ((y * img.width + x) * 4) as usize;
            let p = &img.data[o..o + 4];
            // RgbaImage is straight alpha; text vs bubble bg differ by a lot
            let diff = (p[0] as i32 - bg[0] as i32).abs()
                + (p[1] as i32 - bg[1] as i32).abs()
                + (p[2] as i32 - bg[2] as i32).abs();
            if diff > 60 {
                if first.is_none() {
                    first = Some(y);
                }
                last = y;
                break;
            }
        }
    }
    first.map(|f| (f, last))
}

/// Per-DPI-cell self validation: the editor's natural height is
/// scale-independent (DIP space) and the drawn text rows fit the editor rect.
fn validate_dpi_cell(
    app: &App,
    img: &RgbaImage,
    st: &str,
    scale: f32,
    theme: Theme,
    base_line_h: f32,
    want_lines: usize,
) -> Result<(), String> {
    let e = app
        .layout
        .editor
        .ok_or_else(|| format!("dpi {scale} {st}: no editor rect"))?;
    // 1) measured content height: want_lines * ~19 DIP at every scale
    //    (an empty editor legitimately measures 0 — only text states assert)
    let eh = app.measured.editor_content_h;
    let want_h = base_line_h * want_lines as f32;
    if want_lines > 0 && (eh - want_h).abs() > 4.0 {
        return Err(format!(
            "dpi {scale} {} {st}: editor content h {eh} DIP, want ~{want_h}",
            theme.name()
        ));
    }
    // 2) drawn text ink span must cover ~all lines inside the editor rect:
    //    too large => clipped (span ~= rect height, lines missing),
    //    too small => span far under expected
    if want_lines > 0 {
        let pal = theme.palette();
        let bg = [
            (pal.surface[0] * 255.0) as u8,
            (pal.surface[1] * 255.0) as u8,
            (pal.surface[2] * 255.0) as u8,
        ];
        let span = ink_span(
            img,
            (e.x * scale) as u32,
            (e.y * scale) as u32,
            (e.right() * scale) as u32,
            (e.bottom() * scale) as u32,
            bg,
        );
        let Some((first, last)) = span else {
            return Err(format!(
                "dpi {scale} {} {st}: no text ink in editor rect",
                theme.name()
            ));
        };
        // text height ~ cap-to-baseline per line; want span covers at least
        // want_lines worth of line pitch minus the last line's leading
        let want_span = (want_lines as f32 * base_line_h * 0.8) * scale;
        let got = (last - first) as f32;
        if got < want_span * 0.7 || got > e.h * scale + 2.0 {
            return Err(format!(
                "dpi {scale} {} {st}: text ink span {got}px vs want ~{want_span}px",
                theme.name()
            ));
        }
    }
    // 3) caret stays inside the editor rect when shown
    let (created, pos, _size) = app.editor.caret_info();
    if created && (pos.x < 0 || pos.y < 0 || pos.x as f32 > e.w || pos.y as f32 > e.h) {
        return Err(format!(
            "dpi {scale} {} {st}: caret {pos:?} outside editor {e:?}",
            theme.name()
        ));
    }
    Ok(())
}

fn dpi_sheet(app: &mut App, _rig: &mascot_animation::Rig, tmp: &Path) -> Result<RgbaImage, String> {
    let states = ["composer-multiline", "response", "send-focus-visible"];
    let want_lines = [4usize, 0, 1]; // multiline=4 lines, response follow-up empty
    let themes = [Theme::Light, Theme::Dark];
    let scales = [1.0f32, 1.25, 1.5, 2.0];
    // single-line DIP height baseline (100% light) — ~19
    let mut base_line_h = tokens::BODY_LINE;
    // grid[scale][theme*state] at NATIVE pixels
    let mut grid: Vec<Vec<RgbaImage>> = Vec::new();
    for &sc in &scales {
        app.scale = sc;
        app.editor.set_scale(sc).ok();
        let _ = app.rebuild_sprite();
        let mut row: Vec<RgbaImage> = Vec::new();
        for theme in themes {
            for (si, st) in states.iter().enumerate() {
                presets::apply(app, st, false);
                app.state.theme = theme;
                app.apply_theme();
                let img = app
                    .render_offscreen()
                    .map_err(|e| format!("dpi {sc}/{st}: {e}"))?;
                if sc == 1.0 && *st == "send-focus-visible" && theme == Theme::Light {
                    base_line_h = app.measured.editor_content_h;
                }
                if let Err(e) =
                    validate_dpi_cell(app, &img, st, sc, theme, base_line_h, want_lines[si])
                {
                    let _ = img.save(
                        &std::env::temp_dir()
                            .join(format!("bad-cell-{sc}-{}-{st}.png", theme.name())),
                    );
                    return Err(format!("cell {} {st}: {e}", theme.name()));
                }
                // layout-derived crop, zoomed: the whole cell stays visible
                save_zoomed(
                    app,
                    &img,
                    &tmp.join("zoomed").join(format!(
                        "dpi{}-{}-{}.png",
                        (sc * 100.0).round() as u32,
                        theme.name(),
                        st
                    )),
                    2,
                )
                .map_err(|e| format!("zoom dpi {sc} {st}: {e}"))?;
                row.push(img);
            }
        }
        grid.push(row);
    }
    app.scale = 1.0;
    app.editor.set_scale(1.0).ok();
    let _ = app.rebuild_sprite();

    // column widths = max cell width per column; row heights = max per scale
    let ncols = themes.len() * states.len();
    let mut col_w = vec![0u32; ncols];
    let mut row_h = vec![0u32; scales.len()];
    for (si, row) in grid.iter().enumerate() {
        for (j, img) in row.iter().enumerate() {
            col_w[j] = col_w[j].max(img.width);
            row_h[si] = row_h[si].max(img.height);
        }
    }
    let label_h = 22u32;
    let scale_w = 44u32;
    let dark = [0.05f32, 0.05, 0.06, 1.0];
    // column headers: "{theme} {state}"
    let mut headers: Vec<RgbaImage> = Vec::new();
    for theme in themes {
        for st in states {
            let label = format!("{} {}", theme.name(), st);
            headers.push(text_bmp(app, &label, 260, label_h, dark)?);
        }
    }
    let total_w = scale_w + col_w.iter().sum::<u32>();
    let total_h = label_h + row_h.iter().sum::<u32>();
    let mut sheet = RgbaImage::new(total_w, total_h);
    sheet.fill_rect(0, 0, total_w, total_h, [0xED, 0xED, 0xED, 255]);
    // headers
    let mut x = scale_w;
    for (j, hb) in headers.iter().enumerate() {
        sheet.blit(hb, x, 0);
        x += col_w[j];
    }
    // rows: scale label + cells bottom-anchored in the row
    let mut y = label_h;
    for (si, row) in grid.iter().enumerate() {
        let label = format!("{}%", (scales[si] * 100.0) as u32);
        let lc = text_bmp(app, &label, scale_w, label_h, dark)?;
        sheet.blit(&lc, 0, y + (row_h[si] - label_h) / 2);
        let mut x = scale_w;
        for (j, img) in row.iter().enumerate() {
            let cx = x + (col_w[j] - img.width) / 2;
            sheet.blit(img, cx, y + (row_h[si] - img.height));
            x += col_w[j];
        }
        y += row_h[si];
    }
    Ok(sheet)
}

/// Every icon at 16 DIP x {1.0,1.25,1.5,2.0} x {light,dark}, nearest-zoomed 4x.
fn icon_sheet(r: &Renderer) -> Result<RgbaImage, String> {
    use windows::Win32::Graphics::Direct2D::Common::*;
    use windows::Win32::Graphics::Direct2D::*;
    use windows::core::Interface;
    let scales = [1.0f32, 1.25, 1.5, 2.0];
    let cell_native = (tokens::ICON_SIZE * 2.0).ceil() as u32 + 4; // enough for 2x
    let zoom = 4u32;
    let cell = cell_native * zoom;
    let cols = scales.len();
    let rows = mascot_icons::Icon::ALL.len() * 2;
    let mut sheet = RgbaImage::new(cell * cols as u32, cell * rows as u32);
    let ctx = &r.ctx;
    for (ti, theme) in [Theme::Light, Theme::Dark].iter().enumerate() {
        let pal = theme.palette();
        for (ii, icon) in mascot_icons::Icon::ALL.iter().enumerate() {
            for (si, sc) in scales.iter().enumerate() {
                let px = (tokens::ICON_SIZE * sc).ceil() as u32;
                let bmp = r
                    .create_target_bitmap([px, px])
                    .map_err(|e| e.to_string())?;
                unsafe {
                    ctx.SetTarget(&bmp.cast::<ID2D1Image>().unwrap());
                    ctx.SetDpi(96.0 * sc, 96.0 * sc);
                    ctx.BeginDraw();
                    ctx.Clear(Some(&D2D1_COLOR_F {
                        r: pal.surface[0],
                        g: pal.surface[1],
                        b: pal.surface[2],
                        a: 1.0,
                    }));
                    let g =
                        mascot_ui_win32::icons::icon_geometry(&ctx.GetFactory().unwrap(), *icon)
                            .unwrap();
                    let st = mascot_ui_win32::icons::icon_stroke_style(&ctx.GetFactory().unwrap())
                        .unwrap();
                    let b: ID2D1Brush = ctx
                        .CreateSolidColorBrush(
                            &D2D1_COLOR_F {
                                r: pal.foreground[0],
                                g: pal.foreground[1],
                                b: pal.foreground[2],
                                a: 1.0,
                            },
                            None,
                        )
                        .unwrap()
                        .cast()
                        .unwrap();
                    mascot_ui_win32::icons::draw_icon(
                        ctx,
                        &g,
                        mascot_ui::Rect::new(0.0, 0.0, tokens::ICON_SIZE, tokens::ICON_SIZE),
                        &b,
                        &st,
                    );
                    ctx.EndDraw(None, None).unwrap();
                    ctx.SetTarget(None);
                    ctx.SetDpi(96.0, 96.0); // shared ctx invariant
                }
                let img = r.read_back(&bmp, [px, px]).map_err(|e| e.to_string())?;
                // nearest 4x zoom into the sheet cell
                let cx = si as u32 * cell;
                let cy = (ti * mascot_icons::Icon::ALL.len() + ii) as u32 * cell;
                for y in 0..px {
                    for x in 0..px {
                        let p = &img.data[((y * px + x) * 4) as usize..][..4];
                        for dy in 0..zoom {
                            for dx in 0..zoom {
                                let sx = cx + (cell - px * zoom) / 2 + x * zoom + dx;
                                let sy = cy + (cell - px * zoom) / 2 + y * zoom + dy;
                                sheet.fill_rect(sx, sy, 1, 1, [p[0], p[1], p[2], p[3]]);
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(sheet)
}

/// Real OS build string, e.g. "10.0.26200.6584" (major.minor.build.UBR) —
/// via RtlGetVersion + the CurrentVersion UBR registry value.
pub fn os_build() -> String {
    use windows::Win32::System::LibraryLoader::*;
    unsafe {
        let ntdll = GetModuleHandleW(windows::core::w!("ntdll.dll")).unwrap_or_default();
        let p = GetProcAddress(ntdll, windows::core::s!("RtlGetVersion"));
        let mut base = String::from("windows");
        if let Some(p) = p {
            #[repr(C)]
            struct OsVer {
                size: u32,
                major: u32,
                minor: u32,
                build: u32,
                platform: u32,
                csd: [u16; 128],
            }
            let f: unsafe extern "system" fn(*mut OsVer) -> i32 = std::mem::transmute(p as usize);
            let mut v = OsVer {
                size: std::mem::size_of::<OsVer>() as u32,
                major: 0,
                minor: 0,
                build: 0,
                platform: 0,
                csd: [0; 128],
            };
            if f(&mut v) == 0 {
                base = format!("{}.{}.{}", v.major, v.minor, v.build);
            }
        }
        // UBR (update build revision) from the registry
        let out = std::process::Command::new("reg")
            .args([
                "query",
                "HKLM\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion",
                "/v",
                "UBR",
            ])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|s| {
                s.split_whitespace()
                    .last()
                    .and_then(|h| u32::from_str_radix(h.trim_start_matches("0x"), 16).ok())
            });
        match out {
            Some(ubr) => format!("{base}.{ubr}"),
            None => base,
        }
    }
}

fn write_receipt(out: &Path, files: &[String], app: &App) -> Result<(), String> {
    let mut file_objs = Vec::new();
    for f in files {
        let p = out.join(f);
        let len = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
        file_objs.push(serde_json::json!({"path": f, "bytes": len}));
    }
    let adapter = unsafe {
        windows::core::Interface::cast::<windows::Win32::Graphics::Dxgi::IDXGIDevice>(
            &app.renderer.d3d,
        )
        .and_then(|d| d.GetAdapter())
        .and_then(|a| {
            let desc = a.GetDesc()?;
            Ok::<_, windows::core::Error>(
                desc.Description
                    .iter()
                    .take_while(|c| **c != 0)
                    .map(|c| *c as u8 as char)
                    .collect::<String>(),
            )
        })
        .unwrap_or_default()
    };
    let receipt = serde_json::json!({
        "head": git(&["rev-parse", "HEAD"]),
        "dirty": git_dirty(),
        "rig_rev": git(&["rev-parse", "HEAD:assets/mascot/rig-v0.2"]),
        "tool_version": env!("CARGO_PKG_VERSION"),
        "device": "warp",
        "adapter": adapter,
        "os_build": os_build(),
        "font_family": app.painter.fonts.family,
        "scales": [1.0, 1.25, 1.5, 2.0],
        "backdrop": {"light": "#EDEDED", "dark": "#1F1F1F"},
        "files": file_objs,
    });
    std::fs::write(
        out.join("receipt.json"),
        serde_json::to_string_pretty(&receipt).unwrap(),
    )
    .map_err(|e| e.to_string())
}
