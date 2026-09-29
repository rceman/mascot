//! Mascot rest-pose sprite: rendered once per (size, mirror) through the
//! normal rig renderer, then composited into frames and offscreen captures.
//!
//! The sprite is rendered so its *content bbox* is at least 2x the displayed
//! pixel size and downscaled with `HIGH_QUALITY_CUBIC` (per the brief). The
//! rig's canvas is mostly transparent, so a first probe render measures the
//! content fraction once, then the final bitmap is sized so the content —
//! not the canvas — hits the supersample factor.
//!
//! DPI: `Renderer` assumes a 96-DPI device context. The shared `ctx` is set
//! to `96*scale` while our frames draw, so every `Renderer` call here runs
//! under a saved/reset/restored DPI.

use mascot_animation::{Affine, DrawItem, Rig, RootPlacement};
use mascot_render_win32::image::RgbaImage;
use mascot_render_win32::renderer::{RenderOptions, Renderer, View};
use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;
use windows::Win32::Graphics::Direct2D::*;
use windows::core::*;

/// A pre-rendered rest-pose bitmap plus the pixel-space bounds of its
/// non-transparent content.
pub struct Sprite {
    /// Full bitmap (may be larger than content; margin + silhouette).
    pub bitmap: ID2D1Bitmap1,
    /// Bitmap pixel size.
    pub px: [u32; 2],
    /// Content alpha bbox inside `bitmap` (pixel units).
    pub content: [f32; 4], // left, top, right, bottom
    /// Content size in DIP at the reference scale the sprite was built for.
    pub content_dip: [f32; 2],
}

/// Runs `f` with `ctx` pinned at 96 DPI (what `Renderer` expects), restoring
/// the caller's DPI afterwards. `ctx` is a COM clone so `r` stays mutable.
fn at_dpi96<T>(ctx: &ID2D1DeviceContext, f: impl FnOnce() -> Result<T>) -> Result<T> {
    unsafe {
        let mut dx = 0.0;
        let mut dy = 0.0;
        ctx.GetDpi(&mut dx, &mut dy);
        ctx.SetDpi(96.0, 96.0);
        let out = f();
        ctx.SetDpi(dx, dy);
        out
    }
}

/// Renders the pose into a bitmap and returns it with the content alpha bbox.
fn render_bitmap(
    r: &mut Renderer,
    rig: &Rig,
    items: &[DrawItem],
    world: &[Affine],
    view: View,
    size: [u32; 2],
) -> Result<(ID2D1Bitmap1, [f32; 4])> {
    let ctx = r.ctx.clone();
    let bmp = r.create_target_bitmap(size)?;
    at_dpi96(&ctx, || {
        r.render(
            &bmp,
            size,
            rig,
            items,
            world,
            view,
            &RenderOptions::default(),
        )
    })?;
    let img = at_dpi96(&ctx, || r.read_back(&bmp, size))?;
    Ok((bmp, content_bbox(&img)))
}

/// Alpha bbox of non-transparent pixels in `img`.
fn content_bbox(img: &RgbaImage) -> [f32; 4] {
    let mut bbox = [f32::MAX, f32::MAX, 0.0f32, 0.0f32];
    for y in 0..img.height {
        for x in 0..img.width {
            let a = img.data[((y * img.width + x) * 4 + 3) as usize];
            if a > 12 {
                bbox[0] = bbox[0].min(x as f32);
                bbox[1] = bbox[1].min(y as f32);
                bbox[2] = bbox[2].max(x as f32 + 1.0);
                bbox[3] = bbox[3].max(y as f32 + 1.0);
            }
        }
    }
    if bbox[2] <= bbox[0] {
        return [0.0, 0.0, img.width as f32, img.height as f32];
    }
    bbox
}

impl Sprite {
    /// Renders the rest pose. `content_dip` is the target content box the
    /// caller will draw the sprite into (`layout.mascot.w/h`); `scale` is the
    /// current px-per-DIP. The caller must have already `load_rig`ed `rig`.
    ///
    /// The render is sized so the content bbox lands at >= 2x display pixels;
    /// a cheap 1x probe render measures the canvas-space content fraction.
    pub fn render(
        r: &mut Renderer,
        rig: &Rig,
        mirror: bool,
        content_dip: [f32; 2],
        scale: f32,
    ) -> Result<Sprite> {
        let pose = rig.skeleton.rest_pose();
        let world = rig.skeleton.world(&pose, RootPlacement { mirror });
        let items = rig.draw_list(&world, &pose);
        let canvas = rig.canvas_size();

        // Probe at canvas px to measure the opaque fraction of the canvas.
        let probe_size = [canvas[0].max(1), canvas[1].max(1)];
        let probe_view = View::fit(canvas, probe_size, 0.0);
        let (_probe_bmp, pc) = render_bitmap(r, rig, &items, &world, probe_view, probe_size)?;
        let (cw_canvas, ch_canvas) = (pc[2] - pc[0], pc[3] - pc[1]);

        // Choose a bitmap size so the *content* is at least 2x the displayed
        // pixels. fit() is a uniform scale: content_px = cw_canvas * k.
        let target_px = (scale * 2.0).max(1.0);
        let margin = 4.0f32;
        let kx = (content_dip[0] * target_px) / cw_canvas.max(1.0);
        let ky = (content_dip[1] * target_px) / ch_canvas.max(1.0);
        let k = kx.max(ky);
        let size = [
            (canvas[0] as f32 * k + 2.0 * margin).ceil() as u32 + 1,
            (canvas[1] as f32 * k + 2.0 * margin).ceil() as u32 + 1,
        ];
        let view = View::fit(canvas, size, margin);
        let (bmp, bbox) = render_bitmap(r, rig, &items, &world, view, size)?;
        Ok(Sprite {
            bitmap: bmp,
            px: size,
            content: bbox,
            content_dip,
        })
    }

    /// Draws the sprite so its *content bbox* lands exactly in `rect` (DIP).
    pub fn draw(&self, ctx: &ID2D1DeviceContext, rect: mascot_ui::Rect) {
        unsafe {
            let (cw, ch) = (
                self.content[2] - self.content[0],
                self.content[3] - self.content[1],
            );
            if cw <= 0.0 || ch <= 0.0 {
                return;
            }
            let s = rect.w / cw;
            let dst = D2D_RECT_F {
                left: rect.x - self.content[0] * s,
                top: rect.y - self.content[1] * s,
                right: rect.x + (self.px[0] as f32 - self.content[0]) * s,
                bottom: rect.y + (self.px[1] as f32 - self.content[1]) * s,
            };
            ctx.DrawBitmap(
                &self.bitmap,
                Some(&dst),
                1.0,
                D2D1_INTERPOLATION_MODE_HIGH_QUALITY_CUBIC,
                None,
                None,
            );
        }
    }
}
