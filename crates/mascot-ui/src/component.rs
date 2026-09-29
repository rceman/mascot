//! Pure component semantics: the visual/state vocabulary shared by the
//! Win32 painters and the component gallery. No Win32 types — everything is
//! DIP geometry plus the small enums that name variants, sizes and states.
//! This is a fixed product vocabulary, not a widget framework.

use crate::geom::Size;
use crate::state::{ControlId, UiState};
use crate::theme::Palette;
use crate::theme::tokens::*;

/// Icon-button families — the two fixed edges used by the composer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconButtonKind {
    /// Filled primary action (Send/Stop).
    Primary,
    /// Transparent subtle action (Copy).
    Ghost,
}

impl IconButtonKind {
    /// The fixed square edge in DIP.
    pub fn edge(self) -> f32 {
        match self {
            IconButtonKind::Primary => PRIMARY_BUTTON,
            IconButtonKind::Ghost => GHOST_BUTTON,
        }
    }
}

/// Text-button fill variants (shadcn `variant`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonVariant {
    /// Primary fill (shadcn `default`).
    Default,
    /// Neutral `secondary` fill.
    Secondary,
    /// Transparent with hover accent.
    Ghost,
}

/// Compact text-button sizes (shadcn `size`: `sm`/`default` only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonSize {
    /// `h-8 px-3`.
    Sm,
    /// `h-9 px-4`.
    Default,
}

impl ButtonSize {
    /// Button height in DIP.
    pub fn height(self) -> f32 {
        match self {
            ButtonSize::Sm => BUTTON_H_SM,
            ButtonSize::Default => BUTTON_H,
        }
    }
    /// Horizontal padding in DIP.
    pub fn pad_x(self) -> f32 {
        match self {
            ButtonSize::Sm => BUTTON_PAD_X_SM,
            ButtonSize::Default => BUTTON_PAD_X,
        }
    }
}

/// Badge variants (shadcn `variant`, minus `destructive` — neutral-first).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BadgeVariant {
    /// Primary fill.
    Default,
    /// `secondary` fill.
    Secondary,
    /// Border only, transparent fill.
    Outline,
}

/// The text hierarchy (size/weight in DIP; colour is the painter's job:
/// `Muted` reads `muted_fg`, the rest read `foreground`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextStyle {
    /// 14/400 — composer and response text.
    Body,
    /// 12/400 muted — metadata/status.
    Muted,
    /// 14/500 — control labels (buttons).
    Label,
    /// 12/500 — badges, compact captions.
    Caption,
}

/// Resolved visual state for one control — the painter's only input, so the
/// gallery can express states without fabricating a [`UiState`].
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ControlVisual {
    /// Pointer is over the control (never set when disabled).
    pub hover: bool,
    /// Mouse button held on the control (never set when disabled).
    pub pressed: bool,
    /// Focused AND focus came from the keyboard (ring shown).
    pub focus_visible: bool,
    /// Control is disabled (send with empty editor).
    pub disabled: bool,
}

impl ControlVisual {
    /// Resolves a control's visual state from [`UiState`] — the same rules
    /// the painters applied inline.
    pub fn of(state: &UiState, id: ControlId) -> Self {
        let disabled = state.disabled(id);
        ControlVisual {
            disabled,
            hover: state.interaction.hover == Some(id) && !disabled,
            pressed: state.interaction.pressed == Some(id) && !disabled,
            focus_visible: state.interaction.focus == Some(id) && state.interaction.focus_visible,
        }
    }
}

/// Content-sized button rect for a measured label width.
pub fn button_size(text_w: f32, size: ButtonSize) -> Size {
    Size::new(text_w + 2.0 * size.pad_x(), size.height())
}

/// Content-sized badge rect: `px-2 py-0.5` + 1px border each side -> 22 tall
/// ([`BADGE_H`]); a full pill (`radius = h/2`) drawn by the badge painter.
pub fn badge_size(text_w: f32) -> Size {
    Size::new(text_w + 2.0 * BADGE_PAD_X + 2.0 * BORDER_W, BADGE_H)
}

/// Tooltip size: single line, bounded at [`TOOLTIP_MAX_W`].
pub fn tooltip_size(text_w: f32) -> Size {
    Size::new(
        (text_w + 2.0 * TOOLTIP_PAD_X).min(TOOLTIP_MAX_W),
        SMALL_LINE + 2.0 * TOOLTIP_PAD_Y,
    )
}

// -------------------------------------------------------- colour resolution

/// Per-channel colour mix (linear, straight alpha): `a + (b - a) * t`.
/// The single source of the painter's hover/pressed blends.
pub fn mix(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}

/// (fill, fg) for a text button in the given visual state.
/// Disabled: filled variants get muted/muted_fg, ghost gets no fill +
/// muted_fg — the documented deviation from shadcn `disabled:opacity-50`.
pub fn button_colors(
    pal: &Palette,
    variant: ButtonVariant,
    v: ControlVisual,
) -> ([f32; 4], [f32; 4]) {
    let hover = v.hover && !v.disabled;
    let pressed = v.pressed && !v.disabled;
    match variant {
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
                [0.0; 4]
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
    }
}

/// (fill, fg) for an icon button in the given visual state.
pub fn icon_button_colors(
    pal: &Palette,
    kind: IconButtonKind,
    v: ControlVisual,
) -> ([f32; 4], [f32; 4]) {
    let hover = v.hover && !v.disabled;
    let pressed = v.pressed && !v.disabled;
    match kind {
        IconButtonKind::Primary => {
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
        IconButtonKind::Ghost => {
            let bg = if pressed {
                pal.pressed
            } else if hover {
                pal.hover
            } else {
                [0.0; 4]
            };
            (bg, pal.foreground)
        }
    }
}

/// (fill, fg, border) for a badge. `border` is `Some(pal.border)` only for
/// the outline variant; filled variants draw no border.
pub fn badge_colors(
    pal: &Palette,
    variant: BadgeVariant,
) -> ([f32; 4], [f32; 4], Option<[f32; 4]>) {
    match variant {
        BadgeVariant::Default => (pal.primary, pal.primary_fg, None),
        BadgeVariant::Secondary => (pal.secondary, pal.secondary_fg, None),
        BadgeVariant::Outline => ([0.0; 4], pal.foreground, Some(pal.border)),
    }
}

/// (fill, fg) for a tooltip.
pub fn tooltip_colors(pal: &Palette) -> ([f32; 4], [f32; 4]) {
    (pal.primary, pal.primary_fg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Interaction, Surface};

    #[test]
    fn button_sizes_match_shadcn_compact() {
        // sm h-8 px-3
        let s = button_size(40.0, ButtonSize::Sm);
        assert_eq!(s.h, BUTTON_H_SM);
        assert_eq!(s.w, 40.0 + 2.0 * BUTTON_PAD_X_SM);
        // default h-9 px-4
        let d = button_size(40.0, ButtonSize::Default);
        assert_eq!(d.h, BUTTON_H);
        assert_eq!(d.w, 40.0 + 2.0 * BUTTON_PAD_X);
    }

    #[test]
    fn badge_is_22_tall() {
        let b = badge_size(30.0);
        assert_eq!(b.h, BADGE_H, "shadcn pill badge: 1+2+16+2+1 = 22");
        assert_eq!(b.w, 30.0 + 2.0 * BADGE_PAD_X + 2.0 * BORDER_W);
    }

    #[test]
    fn tooltip_clamps_at_max_width() {
        let s = tooltip_size(20.0);
        assert_eq!(s.w, 20.0 + 2.0 * TOOLTIP_PAD_X);
        let l = tooltip_size(1e4);
        assert_eq!(l.w, TOOLTIP_MAX_W);
        assert_eq!(l.h, SMALL_LINE + 2.0 * TOOLTIP_PAD_Y);
    }

    #[test]
    fn control_visual_suppresses_disabled_states() {
        let mut s = UiState {
            surface: Surface::Composer,
            ..UiState::default()
        };
        s.editor_empty = false;
        s.interaction = Interaction {
            hover: Some(ControlId::Send),
            pressed: None,
            focus: Some(ControlId::Send),
            focus_visible: true,
        };
        let v = ControlVisual::of(&s, ControlId::Send);
        assert!(v.hover && v.focus_visible && !v.pressed && !v.disabled);

        // disabled Send: no hover/pressed leak
        s.editor_empty = true;
        let v = ControlVisual::of(&s, ControlId::Send);
        assert!(v.disabled && !v.hover && !v.pressed && v.focus_visible);
    }

    fn rgb(c: [f32; 4]) -> String {
        format!(
            "#{:02X}{:02X}{:02X}",
            (c[0] * 255.0).round() as u8,
            (c[1] * 255.0).round() as u8,
            (c[2] * 255.0).round() as u8
        )
    }

    #[test]
    fn button_colors_known_values() {
        let l = crate::theme::Theme::Light.palette();
        let d = crate::theme::Theme::Dark.palette();
        let idle = ControlVisual::default();
        // light default = primary #171717 on #FAFAFA text
        let (f, g) = button_colors(&l, ButtonVariant::Default, idle);
        assert_eq!(rgb(f), "#171717");
        assert_eq!(rgb(g), "#FAFAFA");
        // dark secondary = #262626 fill / #FAFAFA text
        let (f, _) = button_colors(&l, ButtonVariant::Secondary, idle);
        assert_eq!(rgb(f), "#F5F5F5");
        let (f, g) = button_colors(&d, ButtonVariant::Secondary, idle);
        assert_eq!(rgb(f), "#262626");
        assert_eq!(rgb(g), "#FAFAFA");
        // ghost idle is transparent; disabled ghost is muted_fg text
        let (f, _) = button_colors(&l, ButtonVariant::Ghost, idle);
        assert_eq!(f[3], 0.0);
        let (_, g) = button_colors(
            &l,
            ButtonVariant::Ghost,
            ControlVisual {
                disabled: true,
                ..ControlVisual::default()
            },
        );
        assert_eq!(rgb(g), "#737373");
    }

    #[test]
    fn icon_button_and_badge_colors_known_values() {
        let l = crate::theme::Theme::Light.palette();
        let idle = ControlVisual::default();
        let (f, g) = icon_button_colors(&l, IconButtonKind::Primary, idle);
        assert_eq!(rgb(f), "#171717");
        assert_eq!(rgb(g), "#FAFAFA");
        let (f, g) = icon_button_colors(&l, IconButtonKind::Ghost, idle);
        assert_eq!(f[3], 0.0);
        assert_eq!(rgb(g), "#0A0A0A");
        // badge: outline draws only the border
        let (f, g, b) = badge_colors(&l, BadgeVariant::Default);
        assert_eq!(
            (rgb(f), rgb(g), b.is_some()),
            ("#171717".into(), "#FAFAFA".into(), false)
        );
        let (f, _, b) = badge_colors(&l, BadgeVariant::Outline);
        assert_eq!(f[3], 0.0);
        assert_eq!(b.map(rgb).as_deref(), Some("#E5E5E5"));
        // tooltip = primary on primary_fg
        let (f, g) = tooltip_colors(&l);
        assert_eq!((rgb(f), rgb(g)), ("#171717".into(), "#FAFAFA".into()));
    }
}
