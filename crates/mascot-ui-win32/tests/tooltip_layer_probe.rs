#![cfg(windows)]
//! Regression for the tooltip-layer clip bug: `tooltip_with` composited via
//! PushLayer/PopLayer and produced an empty frame because layer
//! `contentBounds` ±3.4e38 (and Default's `{0,0,0,0}`) collapse to an empty
//! clip in D2D. Finite bounds are required.
use mascot_render_win32::renderer::{DeviceKind, Renderer};
use mascot_ui::geom::Rect;
use mascot_ui::motion::tooltip_open_frame;
use mascot_ui::theme::Theme;
use mascot_ui_win32::text::Fonts;
use windows::Win32::Graphics::Direct2D::*;
use windows::core::Interface;

fn ink(img: &mascot_render_win32::image::RgbaImage) -> usize {
    img.data.chunks(4).filter(|p| p[3] > 8).count()
}

#[test]
fn tooltip_mid_frame_has_ink() {
    let r = Renderer::new(DeviceKind::Warp).unwrap();
    let fonts = Fonts::new(r.dwrite()).unwrap();
    let p = mascot_ui_win32::paint::Painter::new(&r.ctx, fonts).unwrap();
    let pal = Theme::Light.palette();
    let rect = Rect::new(4.0, 4.0, 52.0, 28.0);
    let (w, h) = (64u32, 48u32);
    for t in [0.0, 25.0, 75.0, 125.0, 150.0] {
        let f = tooltip_open_frame(t);
        let bmp = r.create_target_bitmap([w, h]).unwrap();
        unsafe {
            let ctx = &r.ctx;
            ctx.SetTarget(&bmp.cast::<ID2D1Image>().unwrap());
            ctx.SetDpi(96.0, 96.0);
            ctx.BeginDraw();
            ctx.Clear(None);
            p.tooltip_with(ctx, &pal, rect, "Send", f).unwrap();
            ctx.EndDraw(None, None).unwrap();
        }
        let n = ink(&r.read_back(&bmp, [w, h]).unwrap());
        if t == 0.0 {
            assert_eq!(n, 0, "t=0 (opacity 0) must draw nothing");
        } else {
            assert!(n > 200, "t={t} must draw the pill (ink={n})");
        }
    }
}
