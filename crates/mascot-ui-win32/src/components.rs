//! Component painters: the reusable production drawing entry points.
//!
//! Every visual primitive the product owns lives here as a [`Painter`]
//! method; `paint_frame` (paint.rs) composes them into a frame, and the
//! lab component gallery calls the same functions for its cells — one code
//! path, no lookalike painters. All coordinates are DIP.

use mascot_ui::component::{BadgeVariant, ButtonVariant, ControlVisual, IconButtonKind, TextStyle};
use mascot_ui::geom::Rect;
use mascot_ui::theme::Palette;
use mascot_ui::theme::tokens::*;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::core::*;
use windows_numerics::Vector2;

use crate::icons::draw_icon;
use crate::paint::{Painter, ShadowCache, ShadowKey, brush, cf, dr, mix, rr};

impl Painter {
    /// Measures `text` in `style`; returns the natural width in DIP.
    pub fn text_width(&self, text: &str, style: TextStyle) -> f32 {
        self.fonts
            .measure(text, f32::MAX, self.fonts.format(style))
            .map(|(w, _)| w)
            .unwrap_or(0.0)
    }

    /// Preps resources that need their own draw pass (the shadow bitmap).
    /// Call before the frame's `BeginDraw`, whenever layout/palette changed.
    /// `None` bubble clears the cache.
    pub fn prepare(
        &mut self,
        ctx: &ID2D1DeviceContext,
        pal: &Palette,
        bubble: Option<Rect>,
        radius: f32,
        scale: f32,
        window_px: [u32; 2],
    ) -> Result<()> {
        let Some(b) = bubble else {
            self.shadow = None;
            return Ok(());
        };
        let key = ShadowKey {
            window_px,
            scale: scale.to_bits(),
            bubble: [b.x, b.y, b.w, b.h].map(|v| v.to_bits()),
            radius: radius.to_bits(),
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
            ctx.FillRoundedRectangle(&rr(b, radius), &white);
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
                    dpiX: 96.0 * scale,
                    dpiY: 96.0 * scale,
                    bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET,
                    colorContext: std::mem::ManuallyDrop::new(None),
                },
            )?;
            let bmp_img: ID2D1Image = bmp.cast()?;
            ctx.SetTarget(&bmp_img);
            ctx.SetDpi(96.0 * scale, 96.0 * scale);
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

    /// Draws the cached shadow bitmap over `window` (nothing when no shadow
    /// was prepared — Hidden surface).
    pub fn shadow(&self, ctx: &ID2D1DeviceContext, window: Rect) -> Result<()> {
        unsafe {
            if let Some(s) = &self.shadow {
                ctx.DrawBitmap(
                    &s.bmp.cast::<ID2D1Bitmap>()?,
                    Some(&dr(window)),
                    1.0,
                    D2D1_INTERPOLATION_MODE_LINEAR,
                    None,
                    None,
                );
            }
            Ok(())
        }
    }

    /// Surface fill + snapped 1-device-px hairline. `focused` paints the
    /// border in the ring colour (shadcn input focus treatment).
    pub fn surface(
        &self,
        ctx: &ID2D1DeviceContext,
        pal: &Palette,
        rect: Rect,
        radius: f32,
        scale: f32,
        focused: bool,
    ) -> Result<()> {
        unsafe {
            ctx.FillRoundedRectangle(&rr(rect, radius), &brush(ctx, pal.surface)?);
            let border_c = if focused { pal.ring } else { pal.border };
            let inset = 0.5 / scale;
            let inner = Rect::new(
                rect.x + inset,
                rect.y + inset,
                rect.w - 2.0 * inset,
                rect.h - 2.0 * inset,
            );
            let w = 1.0 / scale;
            ctx.DrawRoundedRectangle(
                &rr(inner, (radius - inset).max(1.0)),
                &brush(ctx, border_c)?,
                w,
                None,
            );
            Ok(())
        }
    }

    /// The two-stroke keyboard focus ring (ring colour at 50 % alpha + a 1px
    /// ring-coloured border).
    pub fn focus_ring(
        &self,
        ctx: &ID2D1DeviceContext,
        pal: &Palette,
        rect: Rect,
        radius: f32,
    ) -> Result<()> {
        unsafe {
            let ring_a = [pal.ring[0], pal.ring[1], pal.ring[2], 0.5];
            ctx.DrawRoundedRectangle(
                &rr(
                    rect.grow(FOCUS_RING_W / 2.0 + 1.0),
                    radius + FOCUS_RING_W / 2.0 + 1.0,
                ),
                &brush(ctx, ring_a)?,
                FOCUS_RING_W,
                None,
            );
            ctx.DrawRoundedRectangle(
                &rr(rect.grow(0.5), radius + 0.5),
                &brush(ctx, pal.ring)?,
                1.0,
                None,
            );
            Ok(())
        }
    }

    /// A Lucide icon centred in `rect` at [`ICON_SIZE`] — the only supported
    /// icon size.
    pub fn icon(
        &self,
        ctx: &ID2D1DeviceContext,
        icon: mascot_icons::Icon,
        rect: Rect,
        color: [f32; 4],
    ) -> Result<()> {
        if let Some(g) = self.icon_geom(icon) {
            draw_icon(ctx, g, rect, &brush(ctx, color)?, &self.icon_style);
        }
        Ok(())
    }

    /// Fixed-size icon button (`PRIMARY_BUTTON`/`GHOST_BUTTON` edge).
    /// Primary: muted/muted_fg when disabled, hover mix 0.10, pressed mix
    /// 0.20; ghost: transparent / hover accent / pressed fill.
    pub fn icon_button(
        &self,
        ctx: &ID2D1DeviceContext,
        pal: &Palette,
        rect: Rect,
        icon: mascot_icons::Icon,
        kind: IconButtonKind,
        v: ControlVisual,
    ) -> Result<()> {
        let hover = v.hover && !v.disabled;
        let pressed = v.pressed && !v.disabled;
        unsafe {
            let (bg, fg_c): ([f32; 4], [f32; 4]) = match kind {
                IconButtonKind::Primary => {
                    let mut bg = if v.disabled { pal.muted } else { pal.primary };
                    if hover {
                        bg = mix(bg, pal.primary_fg, 0.10);
                    }
                    if pressed {
                        bg = mix(bg, pal.primary_fg, 0.20);
                    }
                    let fg = if v.disabled {
                        pal.muted_fg
                    } else {
                        pal.primary_fg
                    };
                    (bg, fg)
                }
                IconButtonKind::Ghost => {
                    let bg = if pressed {
                        pal.pressed
                    } else if hover {
                        pal.hover
                    } else {
                        [0.0, 0.0, 0.0, 0.0]
                    };
                    (bg, pal.foreground)
                }
            };
            if bg[3] > 0.0 {
                ctx.FillRoundedRectangle(&rr(rect, RADIUS_MD), &brush(ctx, bg)?);
            }
            if v.focus_visible {
                self.focus_ring(ctx, pal, rect, RADIUS_MD)?;
            }
            self.icon(ctx, icon, rect, fg_c)?;
            Ok(())
        }
    }

    /// Content-sized text button (shadcn Button, compact sizes only).
    /// Disabled uses the same one-rule treatment as the icon button: filled
    /// variants muted/muted_fg, ghost no fill + muted_fg (documented
    /// deviation from shadcn `disabled:opacity-50`). No `shadow-xs`.
    pub fn button(
        &self,
        ctx: &ID2D1DeviceContext,
        pal: &Palette,
        rect: Rect,
        label: &str,
        variant: ButtonVariant,
        v: ControlVisual,
    ) -> Result<()> {
        let hover = v.hover && !v.disabled;
        let pressed = v.pressed && !v.disabled;
        unsafe {
            let (bg, fg_c): ([f32; 4], [f32; 4]) = match variant {
                ButtonVariant::Default => {
                    let mut bg = if v.disabled { pal.muted } else { pal.primary };
                    if hover {
                        bg = mix(bg, pal.primary_fg, 0.10);
                    }
                    if pressed {
                        bg = mix(bg, pal.primary_fg, 0.20);
                    }
                    (
                        bg,
                        if v.disabled {
                            pal.muted_fg
                        } else {
                            pal.primary_fg
                        },
                    )
                }
                ButtonVariant::Secondary => {
                    let mut bg = if v.disabled { pal.muted } else { pal.secondary };
                    if hover {
                        // shadcn `secondary/80`
                        bg = mix(bg, pal.surface, 0.20);
                    }
                    if pressed {
                        bg = pal.pressed;
                    }
                    (
                        bg,
                        if v.disabled {
                            pal.muted_fg
                        } else {
                            pal.secondary_fg
                        },
                    )
                }
                ButtonVariant::Ghost => {
                    let bg = if pressed {
                        pal.pressed
                    } else if hover {
                        pal.hover
                    } else {
                        [0.0, 0.0, 0.0, 0.0]
                    };
                    (
                        bg,
                        if v.disabled {
                            pal.muted_fg
                        } else {
                            pal.foreground
                        },
                    )
                }
            };
            if bg[3] > 0.0 {
                ctx.FillRoundedRectangle(&rr(rect, RADIUS_MD), &brush(ctx, bg)?);
            }
            if v.focus_visible {
                self.focus_ring(ctx, pal, rect, RADIUS_MD)?;
            }
            self.text_centred(ctx, rect, label, TextStyle::Label, fg_c)?;
            Ok(())
        }
    }

    /// Compact status pill (shadcn Badge, neutral variants only — no
    /// destructive; state is never colour-only). Outline gets a snapped
    /// 1-device-px `border` stroke.
    pub fn badge(
        &self,
        ctx: &ID2D1DeviceContext,
        pal: &Palette,
        rect: Rect,
        label: &str,
        variant: BadgeVariant,
    ) -> Result<()> {
        unsafe {
            let (bg, fg_c, stroke) = match variant {
                BadgeVariant::Default => (pal.primary, pal.primary_fg, false),
                BadgeVariant::Secondary => (pal.secondary, pal.secondary_fg, false),
                BadgeVariant::Outline => ([0.0; 4], pal.foreground, true),
            };
            // pill: radius = h/2 (shadcn badge is a full pill)
            if bg[3] > 0.0 {
                ctx.FillRoundedRectangle(&rr(rect, rect.h / 2.0), &brush(ctx, bg)?);
            }
            if stroke {
                // 1 DIP border in `border` colour, snapped to device pixels
                let mut dx = 0.0f32;
                let mut dy = 0.0f32;
                ctx.GetDpi(&mut dx, &mut dy);
                let scale = (dx / 96.0).max(0.5);
                let inset = (BORDER_W / 2.0) / scale;
                let inner = Rect::new(
                    rect.x + inset,
                    rect.y + inset,
                    rect.w - 2.0 * inset,
                    rect.h - 2.0 * inset,
                );
                ctx.DrawRoundedRectangle(
                    &rr(inner, (inner.h / 2.0 - inset).max(1.0)),
                    &brush(ctx, pal.border)?,
                    BORDER_W / scale,
                    None,
                );
            }
            self.text_centred(ctx, rect, label, TextStyle::Caption, fg_c)?;
            Ok(())
        }
    }

    /// Single-line tooltip bubble: `primary` fill, `primary_fg` text,
    /// centred; long labels ellipsis-trim to the rect (word-wrap off).
    pub fn tooltip(
        &self,
        ctx: &ID2D1DeviceContext,
        pal: &Palette,
        rect: Rect,
        label: &str,
    ) -> Result<()> {
        unsafe {
            ctx.FillRoundedRectangle(&rr(rect, RADIUS_MD), &brush(ctx, pal.primary)?);
        }
        // the ellipsis layout gets the padded inner rect: max width is
        // `rect.w - 2*TOOLTIP_PAD_X` so a clamped label keeps its padding
        // instead of touching the pill edge
        let inner = Rect::new(
            rect.x + TOOLTIP_PAD_X,
            rect.y,
            (rect.w - 2.0 * TOOLTIP_PAD_X).max(1.0),
            rect.h,
        );
        self.text_centred(
            ctx,
            inner,
            label,
            TextStyle::Muted, /* 12/400 = small */
            pal.primary_fg,
        )
    }

    /// Hairline separator (horizontal or vertical by rect shape).
    pub fn separator(&self, ctx: &ID2D1DeviceContext, pal: &Palette, rect: Rect) -> Result<()> {
        unsafe {
            ctx.FillRectangle(&dr(rect), &brush(ctx, pal.border)?);
            Ok(())
        }
    }

    /// Typography primitive: draws `text` in `style` (Body/Muted/Label/
    /// Caption); `Muted` reads `muted_fg`, the rest `foreground`.
    pub fn label(
        &self,
        ctx: &ID2D1DeviceContext,
        pal: &Palette,
        rect: Rect,
        text: &str,
        style: TextStyle,
    ) -> Result<()> {
        let color = match style {
            TextStyle::Muted => pal.muted_fg,
            _ => pal.foreground,
        };
        let wtext: Vec<u16> = text.encode_utf16().collect();
        unsafe {
            ctx.DrawText(
                &wtext,
                self.fonts.format(style),
                &dr(rect),
                &brush(ctx, color)?,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
                DWRITE_MEASURING_MODE_NATURAL,
            );
        }
        Ok(())
    }

    /// Centred single-line text in `rect` (shared by button/badge/tooltip
    /// label drawing).
    fn text_centred(
        &self,
        ctx: &ID2D1DeviceContext,
        rect: Rect,
        text: &str,
        style: TextStyle,
        color: [f32; 4],
    ) -> Result<()> {
        unsafe {
            let wtext: Vec<u16> = text.encode_utf16().collect();
            if let Ok(tl) =
                self.fonts
                    .dwrite
                    .CreateTextLayout(&wtext, self.fonts.format(style), rect.w, rect.h)
            {
                tl.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
                let sign = self.fonts.dwrite.CreateEllipsisTrimmingSign(&tl)?;
                tl.SetTrimming(
                    &DWRITE_TRIMMING {
                        granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER,
                        delimiter: 0,
                        delimiterCount: 0,
                    },
                    &sign,
                )?;
                tl.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
                tl.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
                ctx.DrawTextLayout(
                    Vector2 {
                        X: rect.x,
                        Y: rect.y,
                    },
                    &tl,
                    &brush(ctx, color)?,
                    D2D1_DRAW_TEXT_OPTIONS_NONE,
                );
            }
            Ok(())
        }
    }
}
