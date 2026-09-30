//! The frame painter: one [`UiState`]+[`Layout`] -> one Direct2D frame.
//!
//! Shared verbatim between the DComp window and the offscreen capture path so
//! captured PNGs always show the real product frame (same code, same glyphs).
//!
//! Callers wrap a frame in `SetTarget`/`BeginDraw`/`EndDraw` and `SetDpi`;
//! [`Painter`] owns resources that must be built *outside* a draw block
//! (the cached bubble shadow bitmap, icon geometries, text formats).

use mascot_ui::geom::Rect;
use mascot_ui::layout::Layout;
use mascot_ui::state::{ControlId, Surface, UiState};
use mascot_ui::theme::tokens;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::core::*;

use crate::edit::Editor;
use crate::icons::{icon_geometry, icon_stroke_style};
use crate::sprite::Sprite;
use crate::text::Fonts;

pub(crate) fn cf(c: [f32; 4]) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: c[0],
        g: c[1],
        b: c[2],
        a: c[3],
    }
}

pub(crate) fn rr(r: Rect, rad: f32) -> D2D1_ROUNDED_RECT {
    D2D1_ROUNDED_RECT {
        rect: D2D_RECT_F {
            left: r.x,
            top: r.y,
            right: r.right(),
            bottom: r.bottom(),
        },
        radiusX: rad,
        radiusY: rad,
    }
}

pub(crate) fn dr(r: Rect) -> D2D_RECT_F {
    D2D_RECT_F {
        left: r.x,
        top: r.y,
        right: r.right(),
        bottom: r.bottom(),
    }
}

pub(crate) fn brush(ctx: &ID2D1DeviceContext, c: [f32; 4]) -> Result<ID2D1Brush> {
    unsafe { ctx.CreateSolidColorBrush(&cf(c), None)?.cast() }
}

/// One frame's resolved motion state: the animated control colours and the
/// tooltip's current transform. `settled` is the static resolution (what
/// offscreen captures always paint).
pub struct MotionFrame {
    pub action: mascot_ui::motion::ControlColors,
    pub copy: mascot_ui::motion::ControlColors,
    /// (rect, label, frame) — present also while a tooltip is closing.
    pub tooltip: Option<(Rect, String, mascot_ui::motion::TooltipFrame)>,
}

impl MotionFrame {
    /// Static resolution of the frame — no motion applied.
    pub fn settled(state: &UiState, layout: &Layout) -> Self {
        use mascot_ui::component::{ControlVisual, IconButtonKind, icon_button_paint};
        let pal = state.theme.palette();
        let tooltip = match (layout.tooltip, state.tooltip) {
            (Some(tip), Some(id)) => {
                let label = match id {
                    ControlId::Copy if state.copied => "Copied".to_string(),
                    c => c.icon().label().to_string(),
                };
                Some((tip, label, mascot_ui::motion::TOOLTIP_IDENTITY))
            }
            _ => None,
        };
        MotionFrame {
            action: action_paint(state, &pal),
            copy: icon_button_paint(
                &pal,
                IconButtonKind::Ghost,
                ControlVisual::of(state, ControlId::Copy),
            ),
            tooltip,
        }
    }
}

fn action_paint(
    state: &UiState,
    pal: &mascot_ui::theme::Palette,
) -> mascot_ui::motion::ControlColors {
    use mascot_ui::component::{ControlVisual, IconButtonKind, icon_button_paint};
    icon_button_paint(
        pal,
        IconButtonKind::Primary,
        ControlVisual::of(state, state.action_control()),
    )
}

/// Reusable painter resources: fonts, icon geometries, stroke style, sprite.
pub struct Painter {
    pub fonts: Fonts,
    pub icons: Vec<Option<ID2D1PathGeometry>>,
    pub icon_style: ID2D1StrokeStyle,
    pub sprite: Option<Sprite>,
    /// Bubble drop-shadow bitmap, rebuilt when (window px, bubble rect,
    /// radius, theme) change. Drawn first under the bubble.
    pub(crate) shadow: Option<ShadowCache>,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct ShadowKey {
    pub window_px: [u32; 2],
    pub scale: u32,
    pub bubble: [u32; 4],
    pub radius: u32,
    pub dark: bool,
}

pub(crate) struct ShadowCache {
    pub key: ShadowKey,
    pub bmp: ID2D1Bitmap1,
}

impl Painter {
    pub fn new(ctx: &ID2D1DeviceContext, fonts: Fonts) -> Result<Painter> {
        let factory = unsafe { ctx.GetFactory()? };
        let icons = mascot_icons::Icon::ALL
            .iter()
            .map(|i| icon_geometry(&factory, *i).ok())
            .collect();
        Ok(Painter {
            fonts,
            icons,
            icon_style: icon_stroke_style(&factory)?,
            sprite: None,
            shadow: None,
        })
    }

    pub(crate) fn icon_geom(&self, i: mascot_icons::Icon) -> Option<&ID2D1PathGeometry> {
        let idx = mascot_icons::Icon::ALL.iter().position(|x| *x == i)?;
        self.icons[idx].as_ref()
    }

    /// Paints one frame into `ctx` (caller owns SetTarget/BeginDraw/EndDraw
    /// and SetDpi). All coordinates DIP.
    pub fn paint_frame(
        &self,
        ctx: &ID2D1DeviceContext,
        state: &UiState,
        layout: &Layout,
        editor: Option<&Editor>,
        motion: &MotionFrame,
    ) -> Result<()> {
        use mascot_ui::component::TextStyle;
        use mascot_ui::state::Activity;
        let pal = state.theme.palette();
        unsafe {
            ctx.Clear(Some(&cf([0.0, 0.0, 0.0, 0.0])));

            self.shadow(ctx, layout.window)?;

            if let Some(b) = layout.bubble {
                // While the editor holds keyboard focus the border takes the
                // ring colour (shadcn input focus treatment).
                let focused = state.interaction.focus == Some(ControlId::Editor)
                    && state.activity == Activity::Idle;
                self.surface(ctx, &pal, b, layout.bubble_radius, layout.scale, focused)?;
            }

            if let (Some(t), Surface::Response) = (layout.response_text, state.surface) {
                self.label(ctx, &pal, t, mascot_ui::RESPONSE_FIXTURE, TextStyle::Body)?;
            }
            if let Some(s) = layout.separator {
                self.separator(ctx, &pal, s)?;
            }

            if let Some(e_rect) = layout.editor
                && let Some(ed) = editor
            {
                ed.draw_d2d(
                    &ctx.cast::<ID2D1RenderTarget>()?,
                    (e_rect.x, e_rect.y, e_rect.right(), e_rect.bottom()),
                )?;
                if state.editor_empty && !state.composing {
                    let ph: Vec<u16> = match state.surface {
                        Surface::Response => {
                            mascot_ui::PLACEHOLDER_FOLLOWUP.encode_utf16().collect()
                        }
                        _ => mascot_ui::PLACEHOLDER_ASK.encode_utf16().collect(),
                    };
                    let pr = Rect::new(e_rect.x, e_rect.y, e_rect.w, tokens::BODY_LINE);
                    ctx.DrawText(
                        &ph,
                        &self.fonts.body,
                        &dr(pr),
                        &brush(ctx, pal.muted_fg)?,
                        D2D1_DRAW_TEXT_OPTIONS_NONE,
                        DWRITE_MEASURING_MODE_NATURAL,
                    );
                }
                if let Some((pos, size)) = ed.caret()
                    && state.interaction.focus == Some(ControlId::Editor)
                {
                    // caret pos/size are in the editor's DIP client space
                    let caret = Rect::new(
                        e_rect.x + pos.x as f32,
                        e_rect.y + pos.y as f32,
                        (size.cx as f32).max(1.0 / layout.scale),
                        size.cy as f32,
                    );
                    ctx.FillRectangle(&dr(caret), &brush(ctx, pal.foreground)?);
                }
            }

            if let Some(send) = layout.send {
                self.icon_button(
                    ctx,
                    &pal,
                    send,
                    state.action_control().icon(),
                    motion.action,
                )?;
            }
            if let Some(c) = layout.copy {
                let icon = if state.copied {
                    mascot_icons::Icon::Check
                } else {
                    ControlId::Copy.icon()
                };
                self.icon_button(ctx, &pal, c, icon, motion.copy)?;
            }

            if let Some(sp) = &self.sprite {
                sp.draw(ctx, layout.mascot);
            }

            if let Some((tip, label, f)) = &motion.tooltip {
                self.tooltip_with(ctx, &pal, *tip, label, *f)?;
            }
            Ok(())
        }
    }
}
