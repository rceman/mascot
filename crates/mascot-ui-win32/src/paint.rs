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
use mascot_ui::theme::{Palette, tokens};
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::core::*;
use windows_numerics::Vector2;

use crate::edit::Editor;
use crate::icons::{draw_icon, icon_geometry, icon_stroke_style};
use crate::sprite::Sprite;
use crate::text::Fonts;

fn cf(c: [f32; 4]) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: c[0],
        g: c[1],
        b: c[2],
        a: c[3],
    }
}

fn rr(r: Rect, rad: f32) -> D2D1_ROUNDED_RECT {
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

fn dr(r: Rect) -> D2D_RECT_F {
    D2D_RECT_F {
        left: r.x,
        top: r.y,
        right: r.right(),
        bottom: r.bottom(),
    }
}

fn mix(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}

fn brush(ctx: &ID2D1DeviceContext, c: [f32; 4]) -> Result<ID2D1Brush> {
    unsafe { ctx.CreateSolidColorBrush(&cf(c), None)?.cast() }
}

/// Reusable painter resources: fonts, icon geometries, stroke style, sprite.
pub struct Painter {
    pub fonts: Fonts,
    pub icons: Vec<Option<ID2D1PathGeometry>>,
    pub icon_style: ID2D1StrokeStyle,
    pub sprite: Option<Sprite>,
    /// Bubble drop-shadow bitmap, rebuilt when (window px, bubble rect,
    /// radius, theme) change. Drawn first under the bubble.
    shadow: Option<ShadowCache>,
}

#[derive(Clone, Copy, PartialEq)]
struct ShadowKey {
    window_px: [u32; 2],
    scale: u32,
    bubble: [u32; 4],
    radius: u32,
    dark: bool,
}

struct ShadowCache {
    key: ShadowKey,
    bmp: ID2D1Bitmap1,
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

    fn icon(&self, i: mascot_icons::Icon) -> Option<&ID2D1PathGeometry> {
        let idx = mascot_icons::Icon::ALL.iter().position(|x| *x == i)?;
        self.icons[idx].as_ref()
    }

    /// Preps resources that need their own draw pass (the shadow bitmap).
    /// Call before the frame's `BeginDraw`, whenever layout/palette changed.
    pub fn prepare(
        &mut self,
        ctx: &ID2D1DeviceContext,
        pal: &Palette,
        layout: &Layout,
        window_px: [u32; 2],
    ) -> Result<()> {
        let Some(b) = layout.bubble else {
            self.shadow = None;
            return Ok(());
        };
        let key = ShadowKey {
            window_px,
            scale: layout.scale.to_bits(),
            bubble: [b.x, b.y, b.w, b.h].map(|v| v.to_bits()),
            radius: layout.bubble_radius.to_bits(),
            dark: pal.surface[0] < 0.2,
        };
        if self.shadow.as_ref().is_some_and(|s| s.key == key) {
            return Ok(());
        }
        unsafe {
            // record the bubble silhouette (DIP space) in a command list
            let cl = ctx.CreateCommandList()?;
            let cli: ID2D1Image = cl.cast()?;
            ctx.SetTarget(&cli);
            ctx.BeginDraw();
            ctx.Clear(Some(&cf([0.0, 0.0, 0.0, 0.0])));
            let white = brush(ctx, [1.0, 1.0, 1.0, 1.0])?;
            ctx.FillRoundedRectangle(&rr(b, layout.bubble_radius), &white);
            ctx.EndDraw(None, None)?;
            cl.Close()?;
            ctx.SetTarget(None);

            let shadow = ctx.CreateEffect(&CLSID_D2D1Shadow)?;
            let src: ID2D1Image = cl.cast()?;
            shadow.SetInput(0, &src, true);
            // two shadcn layers (0 1 2 a.05, 0 8 24 a.10) approximated by one
            // shadow at sigma 4 / alpha 0.14 (0.30 dark) offset +4 DIP; tuned
            // by visual iteration.
            let alpha = if key.dark { 0.30 } else { 0.14 };
            let col: [u8; 16] = {
                let mut v = [0u8; 16];
                for (i, c) in [pal.shadow[0], pal.shadow[1], pal.shadow[2], alpha]
                    .iter()
                    .enumerate()
                {
                    v[i * 4..i * 4 + 4].copy_from_slice(&c.to_ne_bytes());
                }
                v
            };
            shadow.SetValue(
                D2D1_SHADOW_PROP_COLOR.0 as u32,
                D2D1_PROPERTY_TYPE_VECTOR4,
                &col,
            )?;
            shadow.SetValue(
                D2D1_SHADOW_PROP_BLUR_STANDARD_DEVIATION.0 as u32,
                D2D1_PROPERTY_TYPE_FLOAT,
                &4.0f32.to_ne_bytes(),
            )?;

            let bmp = ctx.CreateBitmap(
                D2D_SIZE_U {
                    width: window_px[0].max(1),
                    height: window_px[1].max(1),
                },
                None,
                0,
                &D2D1_BITMAP_PROPERTIES1 {
                    pixelFormat: D2D1_PIXEL_FORMAT {
                        format: windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
                        alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                    },
                    dpiX: 96.0 * layout.scale,
                    dpiY: 96.0 * layout.scale,
                    bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET,
                    colorContext: std::mem::ManuallyDrop::new(None),
                },
            )?;
            let bmp_img: ID2D1Image = bmp.cast()?;
            ctx.SetTarget(&bmp_img);
            ctx.SetDpi(96.0 * layout.scale, 96.0 * layout.scale);
            ctx.BeginDraw();
            ctx.Clear(Some(&cf([0.0, 0.0, 0.0, 0.0])));
            let off = Vector2 { X: 0.0, Y: 4.0 };
            ctx.DrawImage(
                &shadow.cast::<ID2D1Image>()?,
                Some(&off),
                None,
                D2D1_INTERPOLATION_MODE_LINEAR,
                D2D1_COMPOSITE_MODE_SOURCE_OVER,
            );
            ctx.EndDraw(None, None)?;
            ctx.SetTarget(None);
            // leave the context at 96 DPI outside our frames — the shared
            // Renderer APIs assume it
            ctx.SetDpi(96.0, 96.0);
            self.shadow = Some(ShadowCache { key, bmp });
        }
        Ok(())
    }

    /// Paints one frame into `ctx` (caller owns SetTarget/BeginDraw/EndDraw
    /// and SetDpi). All coordinates DIP.
    pub fn paint_frame(
        &self,
        ctx: &ID2D1DeviceContext,
        state: &UiState,
        layout: &Layout,
        editor: Option<&Editor>,
    ) -> Result<()> {
        let pal = state.theme.palette();
        unsafe {
            ctx.Clear(Some(&cf([0.0, 0.0, 0.0, 0.0])));

            if let Some(s) = &self.shadow {
                let dst = dr(layout.window);
                ctx.DrawBitmap(
                    &s.bmp.cast::<ID2D1Bitmap>()?,
                    Some(&dst),
                    1.0,
                    D2D1_INTERPOLATION_MODE_LINEAR,
                    None,
                    None,
                );
            }

            if let Some(b) = layout.bubble {
                ctx.FillRoundedRectangle(&rr(b, layout.bubble_radius), &brush(ctx, pal.surface)?);
                // hairline: snapped outer edge minus half a device pixel.
                // While the editor holds keyboard focus the border takes the
                // ring colour (shadcn input focus treatment).
                let border_c = if state.interaction.focus == Some(ControlId::Editor)
                    && state.activity == mascot_ui::state::Activity::Idle
                {
                    pal.ring
                } else {
                    pal.border
                };
                let inset = 0.5 / layout.scale;
                let inner = Rect::new(
                    b.x + inset,
                    b.y + inset,
                    b.w - 2.0 * inset,
                    b.h - 2.0 * inset,
                );
                let w = 1.0 / layout.scale;
                ctx.DrawRoundedRectangle(
                    &rr(inner, (layout.bubble_radius - inset).max(1.0)),
                    &brush(ctx, border_c)?,
                    w,
                    None,
                );
            }

            if let (Some(t), Surface::Response) = (layout.response_text, state.surface) {
                let text: Vec<u16> = mascot_ui::RESPONSE_FIXTURE.encode_utf16().collect();
                ctx.DrawText(
                    &text,
                    &self.fonts.body,
                    &dr(t),
                    &brush(ctx, pal.foreground)?,
                    D2D1_DRAW_TEXT_OPTIONS_NONE,
                    DWRITE_MEASURING_MODE_NATURAL,
                );
            }
            if let Some(s) = layout.separator {
                ctx.FillRectangle(&dr(s), &brush(ctx, pal.border)?);
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
                self.paint_button(ctx, state, send, state.action_control(), &pal)?;
            }
            if let Some(c) = layout.copy {
                self.paint_button(ctx, state, c, ControlId::Copy, &pal)?;
            }

            if let Some(sp) = &self.sprite {
                sp.draw(ctx, layout.mascot);
            }

            if let (Some(tip), Some(id)) = (layout.tooltip, state.tooltip) {
                let label = match id {
                    ControlId::Copy if state.copied => "Copied",
                    c => c.icon().label(),
                };
                ctx.FillRoundedRectangle(&rr(tip, tokens::RADIUS_MD), &brush(ctx, pal.primary)?);
                let fmt = &self.fonts.small;
                // measure and centre manually (tooltip rect is fixed size)
                let wtext: Vec<u16> = label.encode_utf16().collect();
                if let Ok(tl) = self
                    .fonts
                    .dwrite
                    .CreateTextLayout(&wtext, fmt, tip.w, tip.h)
                {
                    tl.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
                    tl.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
                    ctx.DrawTextLayout(
                        Vector2 { X: tip.x, Y: tip.y },
                        &tl,
                        &brush(ctx, pal.primary_fg)?,
                        D2D1_DRAW_TEXT_OPTIONS_NONE,
                    );
                }
            }
            Ok(())
        }
    }

    fn paint_button(
        &self,
        ctx: &ID2D1DeviceContext,
        state: &UiState,
        rect: Rect,
        id: ControlId,
        pal: &Palette,
    ) -> Result<()> {
        let disabled = state.disabled(id);
        let hover = state.interaction.hover == Some(id) && !disabled;
        let pressed = state.interaction.pressed == Some(id) && !disabled;
        let primary = matches!(id, ControlId::Send | ControlId::Stop);
        let copied = state.copied && id == ControlId::Copy;

        unsafe {
            let (bg, fg_c): ([f32; 4], [f32; 4]) = if primary {
                let mut bg = if disabled { pal.muted } else { pal.primary };
                if hover {
                    bg = mix(bg, pal.primary_fg, 0.10);
                }
                if pressed {
                    bg = mix(bg, pal.primary_fg, 0.20);
                }
                let fg = if disabled {
                    pal.muted_fg
                } else {
                    pal.primary_fg
                };
                (bg, fg)
            } else {
                let bg = if pressed {
                    pal.pressed
                } else if hover {
                    pal.hover
                } else {
                    [0.0, 0.0, 0.0, 0.0]
                };
                (bg, pal.foreground)
            };
            if bg[3] > 0.0 {
                ctx.FillRoundedRectangle(&rr(rect, tokens::RADIUS_MD), &brush(ctx, bg)?);
            }
            if state.interaction.focus == Some(id) && state.interaction.focus_visible {
                let ring_a = [pal.ring[0], pal.ring[1], pal.ring[2], 0.5];
                ctx.DrawRoundedRectangle(
                    &rr(
                        rect.grow(tokens::FOCUS_RING_W / 2.0 + 1.0),
                        tokens::RADIUS_MD + tokens::FOCUS_RING_W / 2.0 + 1.0,
                    ),
                    &brush(ctx, ring_a)?,
                    tokens::FOCUS_RING_W,
                    None,
                );
                ctx.DrawRoundedRectangle(
                    &rr(rect.grow(0.5), tokens::RADIUS_MD + 0.5),
                    &brush(ctx, pal.ring)?,
                    1.0,
                    None,
                );
            }
            let icon = if copied {
                mascot_icons::Icon::Check
            } else {
                id.icon()
            };
            if let Some(g) = self.icon(icon) {
                draw_icon(ctx, g, rect, &brush(ctx, fg_c)?, &self.icon_style);
            }
            Ok(())
        }
    }
}
