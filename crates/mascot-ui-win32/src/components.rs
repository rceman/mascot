//! Component painters: the reusable production drawing entry points.
//!
//! Every visual primitive the product owns lives here as a [`Painter`]
//! method; `paint_frame` (paint.rs) composes them into a frame, and the
//! lab component gallery calls the same functions for its cells — one code
//! path, no lookalike painters. All coordinates are DIP.

use mascot_ui::component::{BadgeVariant, TextStyle};
use mascot_ui::geom::Rect;
use mascot_ui::theme::Palette;
use mascot_ui::theme::tokens::*;
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::core::*;
use windows_numerics::Vector2;

use crate::icons::draw_icon;
use crate::paint::{Painter, ShadowCache, ShadowKey, brush, cf, dr, rr};
use mascot_ui::component::{badge_colors, tooltip_colors};

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
    /// ring-coloured border). `ring` is the animation progress 0..=1: the
    /// halo's inner edge stays fixed at 1 DIP out while the stroke width and
    /// alphas ramp in (like a growing box-shadow spread); at `ring == 1` the
    /// output is byte-identical to the settled ring.
    pub fn focus_ring(
        &self,
        ctx: &ID2D1DeviceContext,
        pal: &Palette,
        rect: Rect,
        radius: f32,
        ring: f32,
    ) -> Result<()> {
        if ring <= 0.0 {
            return Ok(());
        }
        unsafe {
            let w = FOCUS_RING_W * ring;
            let ring_a = [pal.ring[0], pal.ring[1], pal.ring[2], 0.5 * ring];
            ctx.DrawRoundedRectangle(
                &rr(rect.grow(w / 2.0 + 1.0), radius + w / 2.0 + 1.0),
                &brush(ctx, ring_a)?,
                w,
                None,
            );
            let border_a = [pal.ring[0], pal.ring[1], pal.ring[2], pal.ring[3] * ring];
            ctx.DrawRoundedRectangle(
                &rr(rect.grow(0.5), radius + 0.5),
                &brush(ctx, border_a)?,
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

    /// Fixed-size icon button (`PRIMARY_BUTTON`/`GHOST_BUTTON` edge) painted
    /// with resolved colours `c` (see `component::icon_button_paint` — the
    /// single colour source; motion tweens interpolate it).
    pub fn icon_button(
        &self,
        ctx: &ID2D1DeviceContext,
        pal: &Palette,
        rect: Rect,
        icon: mascot_icons::Icon,
        c: mascot_ui::motion::ControlColors,
    ) -> Result<()> {
        unsafe {
            if c.fill[3] > 0.0 {
                ctx.FillRoundedRectangle(&rr(rect, RADIUS_MD), &brush(ctx, c.fill)?);
            }
            if c.ring > 0.0 {
                self.focus_ring(ctx, pal, rect, RADIUS_MD, c.ring)?;
            }
            self.icon(ctx, icon, rect, c.fg)?;
            Ok(())
        }
    }

    /// Content-sized text button (shadcn Button, compact sizes only) painted
    /// with resolved colours `c` (see `component::button_paint`). No
    /// `shadow-xs`.
    pub fn button(
        &self,
        ctx: &ID2D1DeviceContext,
        pal: &Palette,
        rect: Rect,
        label: &str,
        c: mascot_ui::motion::ControlColors,
    ) -> Result<()> {
        unsafe {
            if c.fill[3] > 0.0 {
                ctx.FillRoundedRectangle(&rr(rect, RADIUS_MD), &brush(ctx, c.fill)?);
            }
            if c.ring > 0.0 {
                self.focus_ring(ctx, pal, rect, RADIUS_MD, c.ring)?;
            }
            self.text_centred(ctx, rect, label, TextStyle::Label, c.fg)?;
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
            let (bg, fg_c, border) = badge_colors(pal, variant);
            // pill: radius = h/2 (shadcn badge is a full pill)
            if bg[3] > 0.0 {
                ctx.FillRoundedRectangle(&rr(rect, rect.h / 2.0), &brush(ctx, bg)?);
            }
            if let Some(border_c) = border {
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
                    &brush(ctx, border_c)?,
                    BORDER_W / scale,
                    None,
                );
            }
            self.text_centred(ctx, rect, label, TextStyle::Caption, fg_c)?;
            Ok(())
        }
    }

    /// Single-line tooltip bubble: `foreground` fill, `background` text
    /// (shadcn `bg-foreground text-background`), centred; long labels
    /// ellipsis-trim to the rect (word-wrap off).
    pub fn tooltip(
        &self,
        ctx: &ID2D1DeviceContext,
        pal: &Palette,
        rect: Rect,
        label: &str,
    ) -> Result<()> {
        let (fill, fg) = tooltip_colors(pal);
        unsafe {
            ctx.FillRoundedRectangle(&rr(rect, RADIUS_MD), &brush(ctx, fill)?);
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
            fg,
        )
    }

    /// `tooltip` with an animated frame (shadcn tooltip.tsx animate-in/out:
    /// opacity + scale about the pill's bottom centre + enter slide). At
    /// `f == TOOLTIP_IDENTITY` this calls `tooltip()` unchanged
    /// (byte-identical); opacity <= 0 draws nothing.
    pub fn tooltip_with(
        &self,
        ctx: &ID2D1DeviceContext,
        pal: &Palette,
        rect: Rect,
        label: &str,
        f: mascot_ui::motion::TooltipFrame,
    ) -> Result<()> {
        if f == mascot_ui::motion::TOOLTIP_IDENTITY {
            return self.tooltip(ctx, pal, rect, label);
        }
        if f.opacity <= 0.0 {
            return Ok(());
        }
        unsafe {
            use windows_numerics::Matrix3x2;
            let mut old = Matrix3x2::default();
            ctx.GetTransform(&mut old);
            // scale about the pill's bottom centre, then slide down dy DIP
            let (ox, oy) = (rect.x + rect.w / 2.0, rect.bottom());
            let m = Matrix3x2::translation(-ox, -oy)
                * Matrix3x2::scale(f.scale, f.scale)
                * Matrix3x2::translation(ox, oy)
                * Matrix3x2::translation(0.0, f.dy);
            ctx.SetTransform(&(m * old));
            // NB: contentBounds must be a VALID finite rect — Default's
            // {0,0,0,0} is an empty clip and ±3.4e38 overflows D2D's extent
            // math to NaN; both produce an invisible layer.
            let params = D2D1_LAYER_PARAMETERS1 {
                contentBounds: D2D_RECT_F {
                    left: -1.0e6,
                    top: -1.0e6,
                    right: 1.0e6,
                    bottom: 1.0e6,
                },
                maskTransform: Matrix3x2::identity(),
                opacity: f.opacity,
                ..Default::default()
            };
            ctx.PushLayer(&params, None::<&ID2D1Layer>);
            self.tooltip(ctx, pal, rect, label)?;
            ctx.PopLayer();
            ctx.SetTransform(&old);
            Ok(())
        }
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
