//! Icon geometry: `mascot_icons` path segments -> cached `ID2D1PathGeometry`.
//!
//! Lucide icons are stroked (not filled): 24x24 art space, stroke width 2,
//! round caps and joins. The geometry is created once per icon and drawn with
//! `DrawGeometry` — always crisp at any DPI because the art is vector.

use mascot_icons::{Icon, Seg};
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows_numerics::Matrix3x2;

fn dpt(x: f32, y: f32) -> windows_numerics::Vector2 {
    windows_numerics::Vector2 { X: x, Y: y }
}

/// Converts an icon's absolute path commands into a `ID2D1PathGeometry`
/// in the 24x24 Lucide space.
pub fn icon_geometry(
    factory: &ID2D1Factory,
    icon: Icon,
) -> windows::core::Result<ID2D1PathGeometry> {
    unsafe {
        let geom = factory.CreatePathGeometry()?;
        {
            let sink = geom.Open()?;
            let mut open = false;
            for seg in icon.path() {
                match *seg {
                    Seg::M(x, y) => {
                        if open {
                            sink.EndFigure(D2D1_FIGURE_END_OPEN);
                        }
                        sink.BeginFigure(dpt(x, y), D2D1_FIGURE_BEGIN_HOLLOW);
                        open = true;
                    }
                    Seg::L(x, y) => {
                        sink.AddLines(&[dpt(x, y)]);
                    }
                    Seg::C(x1, y1, x2, y2, x, y) => {
                        sink.AddBezier(&D2D1_BEZIER_SEGMENT {
                            point1: dpt(x1, y1),
                            point2: dpt(x2, y2),
                            point3: dpt(x, y),
                        });
                    }
                    Seg::Z => {
                        if open {
                            sink.EndFigure(D2D1_FIGURE_END_OPEN);
                            open = false;
                        }
                    }
                }
            }
            if open {
                sink.EndFigure(D2D1_FIGURE_END_OPEN);
            }
            sink.Close()?;
        }
        Ok(geom)
    }
}

/// Shared Lucide stroke style (round caps/joins, solid).
pub fn icon_stroke_style(factory: &ID2D1Factory) -> windows::core::Result<ID2D1StrokeStyle> {
    unsafe {
        factory.CreateStrokeStyle(
            &D2D1_STROKE_STYLE_PROPERTIES {
                startCap: D2D1_CAP_STYLE_ROUND,
                endCap: D2D1_CAP_STYLE_ROUND,
                dashCap: D2D1_CAP_STYLE_ROUND,
                lineJoin: D2D1_LINE_JOIN_ROUND,
                miterLimit: 4.0,
                dashStyle: D2D1_DASH_STYLE_SOLID,
                dashOffset: 0.0,
            },
            None,
        )
    }
}

/// Draws an icon centred in `rect` (DIP) scaled to `tokens::ICON_SIZE` with
/// Lucide stroke settings: 2/24 of the glyph box, round caps/joins.
pub fn draw_icon(
    ctx: &ID2D1DeviceContext,
    geom: &ID2D1PathGeometry,
    rect: mascot_ui::Rect,
    brush: &ID2D1Brush,
    style: &ID2D1StrokeStyle,
) {
    let size = mascot_ui::theme::tokens::ICON_SIZE;
    let s = size / 24.0;
    let ox = rect.x + (rect.w - size) / 2.0;
    let oy = rect.y + (rect.h - size) / 2.0;
    unsafe {
        let mut old = Matrix3x2::default();
        ctx.GetTransform(&mut old);
        ctx.SetTransform(&(Matrix3x2::scale(s, s) * Matrix3x2::translation(ox, oy)));
        // stroke width lives in geometry units; the scaled transform applies it in DIP
        ctx.DrawGeometry(geom, brush, Icon::STROKE_WIDTH, Some(style));
        ctx.SetTransform(&old);
    }
}
