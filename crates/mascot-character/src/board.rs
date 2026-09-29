//! Combined reference board: pose guide | style image, 2048x1024.

use crate::png_io::Image;
use crate::profile::Profile;
use serde::Serialize;
use std::path::Path;

/// Exact area-weighted box downscale on premultiplied alpha.
pub fn downscale_box(src: &Image, dw: u32, dh: u32) -> Image {
    let mut out = Image::new(dw, dh, [0, 0, 0, 0]);
    let (sw, sh) = (src.w as f64, src.h as f64);
    let (sx, sy) = (sw / dw as f64, sh / dh as f64);
    for y in 0..dh {
        let y0 = y as f64 * sy;
        let y1 = (y as f64 + 1.0) * sy;
        for x in 0..dw {
            let x0 = x as f64 * sx;
            let x1 = (x as f64 + 1.0) * sx;
            let mut acc = [0.0f64; 4];
            let mut area = 0.0f64;
            let iy0 = y0.floor() as u32;
            let iy1 = y1.ceil() as u32;
            let ix0 = x0.floor() as u32;
            let ix1 = x1.ceil() as u32;
            for iy in iy0..iy1.min(src.h) {
                let wy = (y1.min(iy as f64 + 1.0) - y0.max(iy as f64)).max(0.0);
                for ix in ix0..ix1.min(src.w) {
                    let wx = (x1.min(ix as f64 + 1.0) - x0.max(ix as f64)).max(0.0);
                    let w_ = wx * wy;
                    let c = src.px(ix, iy);
                    let a = c[3] as f64 / 255.0;
                    for ch in 0..3 {
                        acc[ch] += c[ch] as f64 * a * w_;
                    }
                    acc[3] += c[3] as f64 * w_;
                    area += w_;
                }
            }
            if area > 0.0 {
                let oa = acc[3] / area;
                // acc[ch] accumulated premultiplied: unpremultiply by alpha
                let px_ = if oa > 0.0 {
                    [
                        (acc[0] / area / (oa / 255.0)).round().clamp(0.0, 255.0) as u8,
                        (acc[1] / area / (oa / 255.0)).round().clamp(0.0, 255.0) as u8,
                        (acc[2] / area / (oa / 255.0)).round().clamp(0.0, 255.0) as u8,
                        oa.round().clamp(0.0, 255.0) as u8,
                    ]
                } else {
                    [0, 0, 0, 0]
                };
                out.set(x, y, px_);
            }
        }
    }
    out
}

/// Bilinear upscale (straight alpha, nearest sample weights).
pub fn upscale_bilinear(src: &Image, dw: u32, dh: u32) -> Image {
    let mut out = Image::new(dw, dh, [0, 0, 0, 0]);
    let (sw, sh) = (src.w as f64, src.h as f64);
    for y in 0..dh {
        let gy = (y as f64 + 0.5) * sh / dh as f64 - 0.5;
        for x in 0..dw {
            let gx = (x as f64 + 0.5) * sw / dw as f64 - 0.5;
            let x0 = gx.floor();
            let y0 = gy.floor();
            let (fx, fy) = (gx - x0, gy - y0);
            let sample = |ix: i64, iy: i64| -> [f64; 4] {
                let (ix, iy) = (
                    ix.clamp(0, src.w as i64 - 1) as u32,
                    iy.clamp(0, src.h as i64 - 1) as u32,
                );
                let c = src.px(ix, iy);
                [c[0] as f64, c[1] as f64, c[2] as f64, c[3] as f64]
            };
            let mut acc = [0.0f64; 4];
            for (dx, wx) in [(0i64, 1.0 - fx), (1, fx)] {
                for (dy, wy) in [(0i64, 1.0 - fy), (1, fy)] {
                    let c = sample(x0 as i64 + dx, y0 as i64 + dy);
                    for ch in 0..4 {
                        acc[ch] += c[ch] * wx * wy;
                    }
                }
            }
            out.set(
                x,
                y,
                [
                    acc[0].round().clamp(0.0, 255.0) as u8,
                    acc[1].round().clamp(0.0, 255.0) as u8,
                    acc[2].round().clamp(0.0, 255.0) as u8,
                    acc[3].round().clamp(0.0, 255.0) as u8,
                ],
            );
        }
    }
    out
}

/// Fit `src` into `dw`x`dh` uniformly; returns (scaled image, scale used).
pub fn fit_uniform(src: &Image, dw: u32, dh: u32) -> (Image, f64) {
    let scale = (dw as f64 / src.w as f64).min(dh as f64 / src.h as f64);
    let (nw, nh) = (
        (src.w as f64 * scale).round().max(1.0) as u32,
        (src.h as f64 * scale).round().max(1.0) as u32,
    );
    let img = if scale >= 1.0 {
        upscale_bilinear(src, nw, nh)
    } else {
        downscale_box(src, nw, nh)
    };
    (img, scale)
}

#[derive(Serialize)]
pub struct BoardImageRef {
    pub path: String,
    pub sha256: String,
    pub size: [u32; 2],
}

#[derive(Serialize)]
pub struct BoardGuideRef {
    /// sha256 of the in-memory generation guide (== committed
    /// `dummy-generation.png` for the same profile).
    pub sha256: String,
    pub size: [u32; 2],
}

#[derive(Serialize)]
pub struct BoardPromptRef {
    pub path: String,
    pub sha256: String,
    pub bytes: u32,
}

#[derive(Serialize)]
pub struct BoardReceiptLayout {
    pub size: [u32; 2],
    pub left: [u32; 4],
    pub right: [u32; 4],
    pub style_rect: [u32; 4],
    pub scale: f64,
}

#[derive(Serialize)]
pub struct BoardReceipt {
    pub format: &'static str,
    pub profile: crate::report::ProfileRef,
    pub guide: BoardGuideRef,
    pub style: BoardImageRef,
    pub layout: BoardReceiptLayout,
    pub board: BoardImageRef,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<BoardPromptRef>,
    pub tool: crate::report::Tool,
}

/// Compose the board and write board.png + board.json (+ prompt.txt).
/// Returns (board image, receipt).
pub fn make_board(
    p: &Profile,
    profile_path: &Path,
    profile_bytes: &[u8],
    style_path: &Path,
    out_dir: &Path,
    brief: Option<&str>,
) -> Result<(Image, BoardReceipt), String> {
    std::fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;
    // left panel: generation guide at 1024
    let guide = crate::guide::render_generation(p, 1024)?;
    let mut guide_png = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut guide_png, guide.w, guide.h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(png::Compression::Balanced);
        enc.set_filter(png::Filter::Adaptive);
        let mut w = enc.write_header().map_err(|e| e.to_string())?;
        w.write_image_data(&guide.data).map_err(|e| e.to_string())?;
    }
    // right panel: style image
    let style = crate::png_io::load_png(style_path)?;
    let (fitted, scale) = fit_uniform(&style, 1024, 1024);
    // compose 2048x1024 on white
    let mut board = Image::new(2048, 1024, [255, 255, 255, 255]);
    for y in 0..1024 {
        for x in 0..1024 {
            board.over(x, y, guide.px(x, y), 1.0);
        }
    }
    let sx0 = 1024 + (1024 - fitted.w) / 2;
    let sy0 = (1024 - fitted.h) / 2;
    for y in 0..fitted.h {
        for x in 0..fitted.w {
            board.over(sx0 + x, sy0 + y, fitted.px(x, y), 1.0);
        }
    }
    // divider
    for x in [1023u32, 1024] {
        for y in 0..1024 {
            board.set(x, y, [0xE0, 0xE0, 0xE0, 255]);
        }
    }
    let board_path = out_dir.join("board.png");
    crate::png_io::save_png(&board_path, &board)?;
    let board_sha = crate::hash::sha256_file(&board_path).map_err(|e| e.to_string())?;
    // prompt.txt
    let prompt_ref = if let Some(b) = brief {
        let tpl_path = profile_path
            .parent()
            .unwrap_or(Path::new("."))
            .join("prompt-template.txt");
        let tpl = std::fs::read_to_string(&tpl_path)
            .map_err(|e| format!("read {}: {e}", tpl_path.display()))?;
        let text = tpl.replace("{brief}", b);
        std::fs::write(out_dir.join("prompt.txt"), &text).map_err(|e| e.to_string())?;
        Some(BoardPromptRef {
            path: "prompt.txt".into(),
            sha256: crate::hash::sha256_hex(text.as_bytes()),
            bytes: text.len() as u32,
        })
    } else {
        None
    };
    let style_bytes = std::fs::read(style_path).map_err(|e| e.to_string())?;
    let receipt = BoardReceipt {
        format: "mascot-character-board/1",
        profile: crate::report::ProfileRef {
            id: p.id.clone(),
            revision: p.revision,
            sha256: crate::hash::sha256_hex(profile_bytes),
        },
        guide: BoardGuideRef {
            sha256: crate::hash::sha256_hex(&guide_png),
            size: [1024, 1024],
        },
        style: BoardImageRef {
            path: style_path.to_string_lossy().replace('\\', "/"),
            sha256: crate::hash::sha256_hex(&style_bytes),
            size: [style.w, style.h],
        },
        layout: BoardReceiptLayout {
            size: [2048, 1024],
            left: [0, 0, 1024, 1024],
            right: [1024, 0, 1024, 1024],
            style_rect: [sx0, sy0, fitted.w, fitted.h],
            scale,
        },
        board: BoardImageRef {
            path: "board.png".into(),
            sha256: board_sha,
            size: [2048, 1024],
        },
        prompt: prompt_ref,
        tool: crate::report::tool(),
    };
    let j = serde_json::to_string_pretty(&receipt).map_err(|e| e.to_string())?;
    std::fs::write(out_dir.join("board.json"), j + "\n").map_err(|e| e.to_string())?;
    Ok((board, receipt))
}
