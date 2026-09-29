//! Theme tokens: the shadcn neutral palette applied natively.
//!
//! Colours are sRGB `[r, g, b, a]` with components in `0.0..=1.0` (alpha is a
//! coverage multiplier — Direct2D brushes premultiply). Every token has a
//! single meaning documented on its field; any deviation from the raw shadcn
//! neutral palette is called out there.

/// Light or dark appearance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Theme {
    #[default]
    Light,
    Dark,
}

impl Theme {
    pub const ALL: &'static [Theme] = &[Theme::Light, Theme::Dark];
    pub fn name(self) -> &'static str {
        match self {
            Theme::Light => "light",
            Theme::Dark => "dark",
        }
    }
    pub fn other(self) -> Theme {
        match self {
            Theme::Light => Theme::Dark,
            Theme::Dark => Theme::Light,
        }
    }
    pub fn palette(self) -> Palette {
        match self {
            Theme::Light => Palette::LIGHT,
            Theme::Dark => Palette::DARK,
        }
    }
}

const fn rgb(v: u32) -> [f32; 4] {
    [
        ((v >> 16) & 0xff) as f32 / 255.0,
        ((v >> 8) & 0xff) as f32 / 255.0,
        (v & 0xff) as f32 / 255.0,
        1.0,
    ]
}

const fn rgba(r: f32, g: f32, b: f32, a: f32) -> [f32; 4] {
    [r, g, b, a]
}

/// The resolved colour set for one theme (shadcn neutral).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    /// Bubble/card surface fill.
    pub surface: [f32; 4],
    /// Primary text/icon colour on `surface`.
    pub foreground: [f32; 4],
    /// Primary action fill (Send/Stop button, tooltip).
    pub primary: [f32; 4],
    /// Text/icon colour on `primary`.
    pub primary_fg: [f32; 4],
    /// Secondary action fill (shadcn `secondary` — neutral button/badge fill).
    pub secondary: [f32; 4],
    /// Text/icon colour on `secondary` (shadcn `secondary-foreground`).
    pub secondary_fg: [f32; 4],
    /// Hover fill for ghost/subtle controls (shadcn `accent`).
    pub hover: [f32; 4],
    /// Pressed fill: one step stronger than `hover`.
    pub pressed: [f32; 4],
    /// Disabled control background (shadcn `muted`).
    pub muted: [f32; 4],
    /// Placeholder / muted text, disabled icon colour.
    pub muted_fg: [f32; 4],
    /// Hairline around the bubble and separators.
    pub border: [f32; 4],
    /// Focus ring colour (used at 50 % alpha for the ring, full for the border).
    pub ring: [f32; 4],
    /// Selection highlight in the editor (neutral, not OS-themed).
    pub selection_bg: [f32; 4],
    /// Text colour inside the selection.
    pub selection_fg: [f32; 4],
    /// Drop shadow colour (alpha scales the two-layer shadow).
    pub shadow: [f32; 4],
}

impl Palette {
    /// shadcn neutral light theme.
    pub const LIGHT: Palette = Palette {
        surface: rgb(0xffffff),
        foreground: rgb(0x0a0a0a),
        primary: rgb(0x171717),
        primary_fg: rgb(0xfafafa),
        secondary: rgb(0xf5f5f5),
        secondary_fg: rgb(0x171717),
        hover: rgb(0xf5f5f5),
        // deviation from shadcn: shadcn has no distinct pressed token; we reuse
        // the border tone so pressed reads one step stronger than hover #F5F5F5.
        pressed: rgb(0xe5e5e5),
        muted: rgb(0xf5f5f5),
        muted_fg: rgb(0x737373),
        border: rgb(0xe5e5e5),
        ring: rgb(0xa1a1a1),
        // neutral selection: ring tone at ~30% blended over surface (opaque —
        // the editor syscolor path cannot carry alpha), foreground text.
        selection_bg: rgb(0xbdbdbd),
        selection_fg: rgb(0x0a0a0a),
        shadow: rgba(0.0, 0.0, 0.0, 1.0),
    };

    /// shadcn neutral dark theme. `surface` is the card tone (#171717) rather
    /// than background #0A0A0A — the bubble floats over an arbitrary desktop,
    /// so it uses the elevated surface for luminance separation (recorded
    /// deviation approved by the spec text).
    pub const DARK: Palette = Palette {
        surface: rgb(0x171717),
        foreground: rgb(0xfafafa),
        primary: rgb(0xe5e5e5),
        primary_fg: rgb(0x171717),
        secondary: rgb(0x262626),
        secondary_fg: rgb(0xfafafa),
        hover: rgb(0x262626),
        // deviation: pressed = hover brightened one step (shadcn has none).
        pressed: rgba(1.0, 1.0, 1.0, 0.22),
        muted: rgb(0x262626),
        muted_fg: rgb(0xa1a1a1),
        border: rgba(1.0, 1.0, 1.0, 0.10),
        ring: rgb(0x737373),
        // white@25% over #171717 blended opaque (editor syscolor has no alpha)
        selection_bg: rgb(0x4a4a4a),
        selection_fg: rgb(0xfafafa),
        shadow: rgba(0.0, 0.0, 0.0, 1.0),
    };
}

/// Size/spacing/layout tokens in DIP. Names are semantic; comments note the
/// shadcn counterpart a value was taken from.
pub mod tokens {
    // spacing steps
    pub const SPACE_XS: f32 = 4.0;
    pub const SPACE_SM: f32 = 8.0;
    pub const SPACE_MD: f32 = 12.0;
    pub const SPACE_LG: f32 = 16.0;
    pub const SPACE_XL: f32 = 20.0;

    // radii
    pub const RADIUS_SM: f32 = 6.0;
    /// `rounded-md` — ghost icon buttons, tooltip.
    pub const RADIUS_MD: f32 = 8.0;
    pub const RADIUS_LG: f32 = 10.0;
    /// `rounded-xl` — the bubble.
    pub const RADIUS_XL: f32 = 14.0;

    /// Bubble hairline width. Always painted on a snapped device-pixel edge.
    pub const BORDER_W: f32 = 1.0;

    /// Icon glyph box (Lucide 24x24 art scaled to this).
    pub const ICON_SIZE: f32 = 16.0;

    /// Send/Stop primary icon button edge (shadcn `size-8` = 32).
    pub const PRIMARY_BUTTON: f32 = 32.0;
    /// Copy ghost icon button edge (between shadcn `size-7`/`size-8`).
    pub const GHOST_BUTTON: f32 = 28.0;
    /// Minimum hit target for any interactive rect.
    pub const MIN_HIT: f32 = 28.0;

    // text
    pub const BODY_SIZE: f32 = 14.0;
    pub const BODY_LINE: f32 = 20.0;
    pub const SMALL_SIZE: f32 = 12.0;
    pub const SMALL_LINE: f32 = 16.0;

    // bubble / composer
    /// Bubble width bounds (design-system range 320–440 DIP).
    pub const BUBBLE_W_MIN: f32 = 320.0;
    /// Bubble width default (composer and response share it).
    pub const BUBBLE_W: f32 = 380.0;
    /// Widest practical bubble width.
    pub const BUBBLE_W_MAX: f32 = 440.0;
    pub const BUBBLE_RADIUS: f32 = RADIUS_XL;
    /// Horizontal inset of bubble content (response text, composer row).
    pub const BUBBLE_PAD_X: f32 = SPACE_LG;
    /// Composer height when the editor holds one line.
    pub const COMPOSER_MIN_H: f32 = 52.0;
    /// Vertical inset of the editor inside the composer.
    pub const EDITOR_PAD_Y: f32 = (COMPOSER_MIN_H - BODY_LINE) / 2.0;
    /// Composer growth cap: six body text lines.
    pub const COMPOSER_MAX_LINES: f32 = 6.0;
    /// Max composer height: 6 lines + vertical insets.
    pub const COMPOSER_MAX_H: f32 = COMPOSER_MAX_LINES * BODY_LINE + 2.0 * EDITOR_PAD_Y;
    /// Gap between editor text and the send button.
    pub const COMPOSER_GAP: f32 = SPACE_SM;
    /// Outer edge gap for the send button.
    pub const COMPOSER_EDGE: f32 = 10.0;

    /// Padding above the response text / below the copy row.
    pub const RESPONSE_PAD_Y: f32 = SPACE_LG;
    /// Gap between response text bottom and the copy button row.
    pub const RESPONSE_COPY_GAP: f32 = SPACE_SM;
    /// Separator line thickness between response and follow-up composer.
    pub const SEPARATOR_H: f32 = 1.0;

    // mascot perch
    /// Mascot content height above the bubble.
    pub const MASCOT_H: f32 = 120.0;
    /// Mascot content bottom edge overlaps the bubble top edge by this much.
    pub const MASCOT_OVERLAP: f32 = 12.0;
    /// Mascot content edge offset from the near (perch side) bubble edge.
    pub const MASCOT_EDGE: f32 = 20.0;
    /// Free space kept above the mascot inside the window.
    pub const MASCOT_TOP_PAD: f32 = 8.0;

    /// Room kept around the bubble inside the window for the drop shadow.
    /// Shadow = 0 1 2 α.05 + 0 8 24 α.10 → 28 covers blur + offset.
    pub const SHADOW_MARGIN: f32 = 28.0;
    /// Focus ring width (ring colour at 50 % alpha + ring-coloured border).
    pub const FOCUS_RING_W: f32 = 3.0;

    // tooltip
    pub const TOOLTIP_PAD_X: f32 = 12.0;
    pub const TOOLTIP_PAD_Y: f32 = 6.0;
    /// Hover delay before a control tooltip appears — the shadcn/Radix
    /// `delayDuration` convention.
    pub const TOOLTIP_DELAY_MS: u32 = 500;
    /// Vertical gap between a control and its tooltip.
    pub const TOOLTIP_GAP: f32 = 6.0;
    /// Tooltip content width cap (single-line, ellipsis beyond).
    pub const TOOLTIP_MAX_W: f32 = 240.0;

    // text buttons / badges
    /// Small text-button height (shadcn `h-8`).
    pub const BUTTON_H_SM: f32 = 32.0;
    /// Small text-button horizontal padding (shadcn `px-3`).
    pub const BUTTON_PAD_X_SM: f32 = 12.0;
    /// Default text-button height (shadcn `h-9`).
    pub const BUTTON_H: f32 = 36.0;
    /// Default text-button horizontal padding (shadcn `px-4`).
    pub const BUTTON_PAD_X: f32 = 16.0;
    /// Badge height — shadcn pill: 1 border + 2 pad + 16 line box, both sides.
    pub const BADGE_H: f32 = 22.0;
    /// Badge horizontal padding (shadcn `px-2`).
    pub const BADGE_PAD_X: f32 = 8.0;
    /// Badge vertical padding (shadcn `py-0.5`; with 1px border -> 22 tall).
    pub const BADGE_PAD_Y: f32 = 2.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rel_luminance(c: [f32; 4]) -> f32 {
        let lin = |v: f32| {
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])
    }

    fn contrast(a: [f32; 4], b: [f32; 4]) -> f32 {
        let (la, lb) = (rel_luminance(a), rel_luminance(b));
        let (hi, lo) = (la.max(lb), la.min(lb));
        (hi + 0.05) / (lo + 0.05)
    }

    #[test]
    fn palette_is_complete_and_contrasting() {
        for t in Theme::ALL {
            let p = t.palette();
            // every colour finite, alpha sane
            for c in [
                p.surface,
                p.foreground,
                p.primary,
                p.primary_fg,
                p.secondary,
                p.secondary_fg,
                p.hover,
                p.pressed,
                p.muted,
                p.muted_fg,
                p.border,
                p.ring,
                p.selection_bg,
                p.selection_fg,
                p.shadow,
            ] {
                assert!(
                    c.iter().all(|v| v.is_finite() && *v >= 0.0 && *v <= 1.0),
                    "{t:?}: bad component {c:?}"
                );
            }
            // WCAG AA: body text and muted text vs surface >= 4.5
            assert!(
                contrast(p.foreground, p.surface) >= 4.5,
                "{t:?} fg/surface {:.2}",
                contrast(p.foreground, p.surface)
            );
            assert!(
                contrast(p.muted_fg, p.surface) >= 4.5,
                "{t:?} muted/surface {:.2}",
                contrast(p.muted_fg, p.surface)
            );
            // primary button label vs fill must also read
            assert!(
                contrast(p.primary_fg, p.primary) >= 4.5,
                "{t:?} primary fg/bg {:.2}",
                contrast(p.primary_fg, p.primary)
            );
            assert!(
                contrast(p.secondary_fg, p.secondary) >= 4.5,
                "{t:?} secondary fg/bg {:.2}",
                contrast(p.secondary_fg, p.secondary)
            );
        }
    }
}
