//! Pure layout: [`UiState`] + measured sizes + scale -> [`Layout`].
//!
//! Everything is DIP (see crate docs). The Win32 layer measures the two text
//! runs that layout cannot know (editor content height via `TxGetNaturalSize`,
//! response fixture height via DirectWrite at [`tokens::RESPONSE_TEXT_W`],
//! tooltip text width) and passes them in [`Measured`].

use crate::geom::{Point, Rect};
use crate::state::{ControlId, Surface, UiState};
use crate::theme::tokens::*;

/// Visible content size of the mascot sprite in DIP (at scale 1.0), measured
/// once by the renderer from the rest-pose alpha bbox.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MascotMetrics {
    /// Width of the opaque content box.
    pub content_w: f32,
    /// Height of the opaque content box.
    pub content_h: f32,
}

/// Text sizes the host measured before layout (DIP).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Measured {
    /// RichEdit natural height of the editor text (`TxGetNaturalSize`).
    pub editor_content_h: f32,
    /// DirectWrite height of the response fixture laid out at
    /// [`RESPONSE_TEXT_W`].
    pub response_text_h: f32,
    /// DirectWrite width of the tooltip label for [`UiState::tooltip`]
    /// (ignored when no tooltip is up).
    pub tooltip_text_w: f32,
}

/// The resolved geometry of one frame, all in DIP window coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Layout {
    /// DPI scale used for pixel snapping/rounding (dpi/96).
    pub scale: f32,
    /// Window size in DIP.
    pub window: Rect,
    /// Mascot content rect (image is drawn so its opaque bbox lands here).
    pub mascot: Rect,
    /// Bubble rect (None on [`Surface::Hidden`]).
    pub bubble: Option<Rect>,
    /// Bubble corner radius.
    pub bubble_radius: f32,
    /// Response body text rect inside the bubble (Response surface only).
    pub response_text: Option<Rect>,
    /// Separator line rect between response and follow-up composer.
    pub separator: Option<Rect>,
    /// Composer row rect (the editor + send/stop band inside the bubble).
    pub composer: Option<Rect>,
    /// Editor text rect (windowless RichEdit client rect).
    pub editor: Option<Rect>,
    /// Shared send/stop button rect (which one is drawn depends on
    /// [`UiState::activity`]).
    pub send: Option<Rect>,
    /// Copy ghost button rect (Response surface only).
    pub copy: Option<Rect>,
    /// Tooltip bubble rect (when [`UiState::tooltip`] is set).
    pub tooltip: Option<Rect>,
}

impl Layout {
    /// Window size in device pixels.
    pub fn window_px(&self) -> [u32; 2] {
        [
            (self.window.w * self.scale).round().max(1.0) as u32,
            (self.window.h * self.scale).round().max(1.0) as u32,
        ]
    }
}

/// Composer height for measured editor content height (see crate docs for the
/// rule).
pub fn composer_height(content_h: f32) -> f32 {
    (content_h + 2.0 * EDITOR_PAD_Y).clamp(COMPOSER_MIN_H, COMPOSER_MAX_H)
}

fn layout_left(state: &UiState, mascot: &MascotMetrics, m: &Measured) -> Layout {
    let mut l = Layout {
        scale: 1.0,
        window: Rect::default(),
        mascot: Rect::default(),
        bubble: None,
        bubble_radius: BUBBLE_RADIUS,
        response_text: None,
        separator: None,
        composer: None,
        editor: None,
        send: None,
        copy: None,
        tooltip: None,
    };
    if state.surface == Surface::Hidden {
        // mascot-only window: tight box around the content
        l.window = Rect::new(0.0, 0.0, mascot.content_w + 16.0, mascot.content_h + 16.0);
        l.mascot = Rect::new(8.0, 8.0, mascot.content_w, mascot.content_h);
        return l;
    }

    let window_w = BUBBLE_W + 2.0 * SHADOW_MARGIN;
    let bubble_x = SHADOW_MARGIN;
    let bubble_y = MASCOT_TOP_PAD + mascot.content_h - MASCOT_OVERLAP;

    let composer_h = composer_height(m.editor_content_h);
    let response_h = if state.surface == Surface::Response {
        RESPONSE_PAD_Y + m.response_text_h + RESPONSE_COPY_GAP + GHOST_BUTTON + SPACE_MD
    } else {
        0.0
    };
    let sep_h = if state.surface == Surface::Response {
        SEPARATOR_H
    } else {
        0.0
    };
    let bubble_h = response_h + sep_h + composer_h;

    let bubble = Rect::new(bubble_x, bubble_y, BUBBLE_W, bubble_h);
    l.bubble = Some(bubble);
    l.window = Rect::new(0.0, 0.0, window_w, bubble.bottom() + SHADOW_MARGIN);

    // mascot perch: content bottom overlaps the bubble top edge
    l.mascot = Rect::new(
        bubble.x + MASCOT_EDGE,
        bubble.y + MASCOT_OVERLAP - mascot.content_h,
        mascot.content_w,
        mascot.content_h,
    );

    let mut y = bubble.y;
    if state.surface == Surface::Response {
        let text = Rect::new(
            bubble.x + BUBBLE_PAD_X,
            y + RESPONSE_PAD_Y,
            RESPONSE_TEXT_W,
            m.response_text_h,
        );
        let copy = Rect::new(
            bubble.right() - SPACE_MD - GHOST_BUTTON,
            text.bottom() + RESPONSE_COPY_GAP,
            GHOST_BUTTON,
            GHOST_BUTTON,
        );
        l.response_text = Some(text);
        l.copy = Some(copy);
        y += response_h;
        l.separator = Some(Rect::new(bubble.x, y, bubble.w, SEPARATOR_H));
        y += SEPARATOR_H;
    }

    let composer = Rect::new(bubble.x, y, bubble.w, composer_h);
    l.composer = Some(composer);
    // send/stop bottom-anchored (== vertically centred for the single-line row)
    let send = Rect::new(
        composer.right() - COMPOSER_EDGE - PRIMARY_BUTTON,
        composer.bottom() - COMPOSER_EDGE - PRIMARY_BUTTON,
        PRIMARY_BUTTON,
        PRIMARY_BUTTON,
    );
    l.send = Some(send);
    let editor = Rect::new(
        composer.x + BUBBLE_PAD_X,
        composer.y + EDITOR_PAD_Y,
        send.x - COMPOSER_GAP - (composer.x + BUBBLE_PAD_X),
        composer.h - 2.0 * EDITOR_PAD_Y,
    );
    l.editor = Some(editor);

    // tooltip floats just above its control, horizontally centred on it
    if let Some(id) = state.tooltip {
        let anchor = match id {
            ControlId::Send | ControlId::Stop => Some(send),
            ControlId::Copy => l.copy,
            ControlId::Editor => l.editor,
        };
        if let Some(a) = anchor {
            let w = m.tooltip_text_w + 2.0 * TOOLTIP_PAD_X;
            let h = SMALL_LINE + 2.0 * TOOLTIP_PAD_Y;
            let x = (a.center().x - w / 2.0).clamp(2.0, window_w - w - 2.0);
            let ty = a.y - TOOLTIP_GAP - h;
            l.tooltip = Some(Rect::new(x, ty.max(2.0), w, h));
        }
    }
    l
}

/// Computes the full layout. [`Placement::Right`] mirrors only the mascot's
/// position (the sprite itself is mirrored by the renderer so it faces the
/// bubble) and the window anchor — the bubble internals always stay LTR:
/// text on the left, Send/Stop on the right, Copy at the right.
pub fn layout(state: &UiState, mascot: &MascotMetrics, m: &Measured, scale: f32) -> Layout {
    use crate::state::Placement;
    let mut l = layout_left(state, mascot, m);
    l.scale = scale;
    if state.placement == Placement::Right {
        l.mascot = l.mascot.mirror(l.window.w);
    }
    l
}

/// What a point in DIP window coordinates hits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    /// Mascot image (drag moves the window; click toggles the bubble).
    Mascot,
    /// An interactive control.
    Control(ControlId),
    /// The editor text area (clicks focus and position the caret).
    Editor,
    /// Bubble interior that is not a control (dead zone, still HTCLIENT).
    Bubble,
    /// Outside every shape — also outside the bubble's rounded corners.
    Outside,
}

/// Hit test against a layout. Priority: interactive controls, then mascot,
/// then editor, then bubble. The bubble's rounded corners are excluded
/// (transparent pixels must be click-through).
pub fn hit_test(state: &UiState, l: &Layout, p: Point) -> Hit {
    // controls first so a control under the mascot's perch zone still wins
    if let Some(r) = l.send
        && r.contains(p)
    {
        return Hit::Control(state.action_control());
    }
    if let Some(r) = l.copy
        && r.contains(p)
    {
        return Hit::Control(ControlId::Copy);
    }
    if l.mascot.contains(p) {
        return Hit::Mascot;
    }
    if let Some(r) = l.editor
        && r.contains(p)
    {
        return Hit::Editor;
    }
    if let Some(b) = l.bubble
        && b.contains(p)
        && !b.in_cut_corner(p, l.bubble_radius)
    {
        return Hit::Bubble;
    }
    Hit::Outside
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::snap;
    use crate::state::{Activity, Interaction, Placement};

    const MASCOT: MascotMetrics = MascotMetrics {
        content_w: 96.0,
        content_h: 120.0,
    };
    const SCALES: [f32; 4] = [1.0, 1.25, 1.5, 2.0];

    fn open(state: Surface) -> UiState {
        let mut s = UiState {
            surface: state,
            ..UiState::default()
        };
        s.interaction = Interaction::default();
        s
    }

    #[test]
    fn snap_lands_on_device_pixels() {
        for &s in &SCALES {
            for dip in [0.0, 0.5, 1.0, 14.4, 27.9, 100.0, 380.0] {
                let px = snap(dip, s) * s;
                assert!(
                    (px - px.round()).abs() < 1e-4,
                    "snap({dip},{s}) -> {px} px not integral"
                );
            }
        }
        // hairline stays 1 device px at every scale
        for &s in &SCALES {
            assert!((snap(BORDER_W, s) * s - 1.0).abs() < 1e-4 || BORDER_W * s >= 1.0);
        }
    }

    #[test]
    fn composer_height_clamps() {
        assert_eq!(composer_height(0.0), COMPOSER_MIN_H);
        assert_eq!(composer_height(BODY_LINE), COMPOSER_MIN_H);
        assert_eq!(
            composer_height(4.0 * BODY_LINE),
            4.0 * BODY_LINE + 2.0 * EDITOR_PAD_Y
        );
        assert_eq!(composer_height(12.0 * BODY_LINE), COMPOSER_MAX_H);
        assert_eq!(composer_height(1e6), COMPOSER_MAX_H);
    }

    #[test]
    fn perch_overlaps_bubble_top() {
        let s = open(Surface::Composer);
        let l = layout(&s, &MASCOT, &Measured::default(), 1.0);
        let (m, b) = (l.mascot, l.bubble.unwrap());
        assert!(
            (m.bottom() - b.y - MASCOT_OVERLAP).abs() < 1e-4,
            "mascot bottom must overlap bubble top by {MASCOT_OVERLAP}"
        );
        assert!((m.x - (b.x + MASCOT_EDGE)).abs() < 1e-4);
        assert!(m.y >= 0.0);
    }

    #[test]
    fn right_placement_mirrors_only_the_mascot() {
        for surface in [Surface::Hidden, Surface::Composer, Surface::Response] {
            let mut l_state = open(surface);
            let mut r_state = l_state.clone();
            l_state.placement = Placement::Left;
            r_state.placement = Placement::Right;
            let m = Measured {
                response_text_h: 80.0,
                ..Measured::default()
            };
            let a = layout(&l_state, &MASCOT, &m, 1.0);
            let b = layout(&r_state, &MASCOT, &m, 1.0);
            assert_eq!(a.window, b.window);
            let w = a.window.w;
            // only the mascot mirrors; bubble internals stay LTR
            assert_eq!(a.mascot.mirror(w), b.mascot);
            assert_eq!(a.bubble, b.bubble);
            assert_eq!(a.response_text, b.response_text);
            assert_eq!(a.separator, b.separator);
            assert_eq!(a.composer, b.composer);
            assert_eq!(a.editor, b.editor);
            assert_eq!(a.send, b.send);
            assert_eq!(a.copy, b.copy);
            assert_eq!(a.tooltip, b.tooltip);
        }
    }

    #[test]
    fn send_centred_in_single_line_composer() {
        let s = open(Surface::Composer);
        let l = layout(
            &s,
            &MASCOT,
            &Measured {
                editor_content_h: BODY_LINE,
                ..Measured::default()
            },
            1.0,
        );
        let (send, comp) = (l.send.unwrap(), l.composer.unwrap());
        assert!((send.center().y - comp.center().y).abs() < 1e-4);
        // hit targets >= MIN_HIT
        assert!(send.w >= MIN_HIT && send.h >= MIN_HIT);
        if let Some(c) = l.copy {
            assert!(c.w >= MIN_HIT && c.h >= MIN_HIT);
        }
    }

    #[test]
    fn editor_inside_bubble_with_insets() {
        for surface in [Surface::Composer, Surface::Response] {
            let s = open(surface);
            let l = layout(
                &s,
                &MASCOT,
                &Measured {
                    editor_content_h: BODY_LINE,
                    response_text_h: 80.0,
                    ..Measured::default()
                },
                1.0,
            );
            let (e, b) = (l.editor.unwrap(), l.bubble.unwrap());
            assert!(e.x >= b.x + 8.0 && e.right() <= b.right() - 8.0);
            assert!(e.y >= b.y && e.bottom() <= b.bottom());
        }
    }

    #[test]
    fn response_layout_is_bounded_and_window_has_shadow_margin() {
        let s = open(Surface::Response);
        let l = layout(
            &s,
            &MASCOT,
            &Measured {
                editor_content_h: BODY_LINE,
                response_text_h: 80.0,
                ..Measured::default()
            },
            1.0,
        );
        let t = l.response_text.unwrap();
        assert!(t.w <= RESPONSE_TEXT_W && t.w > 0.0);
        let b = l.bubble.unwrap();
        // shadow margin: bubble inset from window edges
        assert!(
            b.x >= SHADOW_MARGIN
                && l.window.w - b.right() >= SHADOW_MARGIN
                && l.window.h - b.bottom() >= SHADOW_MARGIN
        );
        let sep = l.separator.unwrap();
        assert!(sep.y > t.bottom() && sep.bottom() < l.composer.unwrap().y + 1.0);
        // copy button sits at the response block bottom-right
        let c = l.copy.unwrap();
        assert!(c.x > t.x + t.w / 2.0 && c.y >= t.bottom());
    }

    #[test]
    fn hit_test_orders_controls_mascot_bubble() {
        let s = open(Surface::Composer);
        let l = layout(&s, &MASCOT, &Measured::default(), 1.0);
        assert_eq!(
            hit_test(&s, &l, l.send.unwrap().center()),
            Hit::Control(ControlId::Send)
        );
        assert_eq!(hit_test(&s, &l, l.editor.unwrap().center()), Hit::Editor);
        assert_eq!(hit_test(&s, &l, l.mascot.center()), Hit::Mascot);
        // rounded corner: just inside the bbox corner is outside the shape
        let b = l.bubble.unwrap();
        assert_eq!(
            hit_test(&s, &l, Point::new(b.x + 1.0, b.y + 1.0)),
            Hit::Outside
        );
        assert_eq!(
            hit_test(&s, &l, Point::new(b.x + BUBBLE_RADIUS, b.y + 0.5)),
            Hit::Bubble
        );
        assert_eq!(hit_test(&s, &l, Point::new(-5.0, -5.0)), Hit::Outside);
        // submitting: same rect reports Stop
        let mut s2 = s.clone();
        s2.activity = Activity::Submitting;
        assert_eq!(
            hit_test(&s2, &l, l.send.unwrap().center()),
            Hit::Control(ControlId::Stop)
        );
    }

    #[test]
    fn hidden_surface_only_hits_mascot() {
        let s = open(Surface::Hidden);
        let l = layout(&s, &MASCOT, &Measured::default(), 1.0);
        assert!(l.bubble.is_none());
        assert_eq!(hit_test(&s, &l, l.mascot.center()), Hit::Mascot);
        assert_eq!(hit_test(&s, &l, Point::new(0.0, 0.0)), Hit::Outside);
    }
}
