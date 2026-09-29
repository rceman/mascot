//! The UI state model: a pure value plus pure transition functions.
//!
//! The Win32 layer owns a `UiState`, mutates it only through these
//! transitions, and repaints when a transition reports a visual change.

use crate::Theme;

/// Which side of the screen the composition lives on. The mascot perches on
/// the near bubble edge and faces the bubble: on [`Placement::Left`] the
/// mascot sits at the bubble's left edge facing right; [`Placement::Right`]
/// mirrors only the mascot position/facing and the window anchor — bubble
/// internals stay LTR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Placement {
    #[default]
    Left,
    Right,
}

impl Placement {
    pub fn other(self) -> Placement {
        match self {
            Placement::Left => Placement::Right,
            Placement::Right => Placement::Left,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Placement::Left => "left",
            Placement::Right => "right",
        }
    }
}

/// Top-level surface: what is visible besides the mascot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Surface {
    /// Only the mascot is visible.
    #[default]
    Hidden,
    /// Mascot + composer bubble (empty or edited).
    Composer,
    /// Mascot + bubble with response section, separator and follow-up composer.
    Response,
}

/// Whether a (mock) submission is in flight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Activity {
    #[default]
    Idle,
    /// Submitted: Stop replaces Send; the submitted text is shown read-only.
    Submitting,
}

/// Interactive controls. `Send` and `Stop` share a rect and are exclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ControlId {
    Editor,
    Send,
    Stop,
    Copy,
}

impl ControlId {
    /// Keyboard focus cycle order: Editor -> Send/Stop -> Copy.
    pub const FOCUS_ORDER: &'static [ControlId] =
        &[ControlId::Editor, ControlId::Send, ControlId::Copy];
    /// The icon for this control in its default state (Copy shows `Check`
    /// while [`UiState::copied`]).
    pub fn icon(self) -> mascot_icons::Icon {
        match self {
            ControlId::Send => mascot_icons::Icon::ArrowUp,
            ControlId::Stop => mascot_icons::Icon::Square,
            ControlId::Copy => mascot_icons::Icon::Copy,
            ControlId::Editor => mascot_icons::Icon::ArrowUp, // editor has no icon
        }
    }
}

/// Pointer/keyboard interaction state. See crate docs for the exact visual
/// meaning of each field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Interaction {
    /// Pointer is over this control's hit rect.
    pub hover: Option<ControlId>,
    /// Pointer is held down over this control.
    pub pressed: Option<ControlId>,
    /// Keyboard focus owner (`None` = no control focused).
    pub focus: Option<ControlId>,
    /// Focus arrived via keyboard (Tab/arrows) — draw the focus ring.
    /// Pointer focus sets `focus` but clears this flag.
    pub focus_visible: bool,
}

/// The complete UI state. Pure data — all behaviour is in the transitions.
#[derive(Debug, Clone, PartialEq)]
pub struct UiState {
    pub theme: Theme,
    pub placement: Placement,
    pub surface: Surface,
    pub activity: Activity,
    pub interaction: Interaction,
    /// The editor holds no text (drives placeholder + send disabled).
    pub editor_empty: bool,
    /// An IME composition is active (Enter must not submit).
    pub composing: bool,
    /// Copy was just clicked; the Copy control shows `Icon::Check`.
    /// Reverted by a one-shot timer in the lab (~1.5 s).
    pub copied: bool,
    /// A tooltip is currently shown for this control.
    pub tooltip: Option<ControlId>,
}

impl Default for UiState {
    fn default() -> Self {
        UiState {
            theme: Theme::Light,
            placement: Placement::Left,
            surface: Surface::Hidden,
            activity: Activity::Idle,
            interaction: Interaction::default(),
            editor_empty: true,
            composing: false,
            copied: false,
            tooltip: None,
        }
    }
}

impl UiState {
    /// True while the Send control is enabled (non-empty editor, not composing,
    /// not submitting). Disabled send renders muted with no hover/pressed.
    pub fn send_enabled(&self) -> bool {
        !self.editor_empty && !self.composing && self.activity == Activity::Idle
    }

    /// The control currently occupying the shared send/stop rect.
    pub fn action_control(&self) -> ControlId {
        match self.activity {
            Activity::Idle => ControlId::Send,
            Activity::Submitting => ControlId::Stop,
        }
    }

    /// True when the given control is disabled (never hover/press/focus-ring).
    pub fn disabled(&self, c: ControlId) -> bool {
        matches!(c, ControlId::Send) && !self.send_enabled()
    }

    /// Whether the composer is interactive (visible and editable). While
    /// `Submitting` the text stays but is read-only/dimmed.
    pub fn editor_editable(&self) -> bool {
        self.surface != Surface::Hidden && self.activity == Activity::Idle
    }

    // -- transitions ---------------------------------------------------------

    /// Enter pressed. Returns true when a submission was triggered.
    /// Never submits while composing, empty, submitting or hidden.
    pub fn submit(&mut self) -> bool {
        if self.surface == Surface::Hidden || !self.send_enabled() {
            return false;
        }
        self.activity = Activity::Submitting;
        self.interaction.hover = None;
        self.interaction.pressed = None;
        self.tooltip = None;
        true
    }

    /// Stop clicked (or submission cancelled): back to an editable composer
    /// with the submitted text preserved.
    pub fn stop(&mut self) {
        if self.activity == Activity::Submitting {
            self.activity = Activity::Idle;
        }
    }

    /// The (mock) response arrived: response surface with an empty follow-up
    /// composer; the submitted text is consumed.
    pub fn response_arrived(&mut self) {
        self.surface = Surface::Response;
        self.activity = Activity::Idle;
        self.editor_empty = true;
        self.copied = false;
        self.interaction = Interaction {
            focus: Some(ControlId::Editor),
            ..self.interaction
        };
    }

    /// Escape: hide the bubble entirely (mascot only).
    pub fn escape(&mut self) {
        self.surface = Surface::Hidden;
        self.interaction = Interaction::default();
        self.composing = false;
        self.copied = false;
        self.tooltip = None;
    }

    /// Mascot clicked while hidden: open the composer and focus the editor.
    /// (While visible, mascot click is a toggle handled by the caller through
    /// `escape`; this function is idempotent when already visible.)
    pub fn open(&mut self) {
        if self.surface == Surface::Hidden {
            self.surface = Surface::Composer;
            self.interaction.focus = Some(ControlId::Editor);
        }
    }

    /// Copy clicked: show the `Check` state; the lab writes CF_UNICODETEXT and
    /// starts the revert timer.
    pub fn copy(&mut self) {
        if self.surface == Surface::Response {
            self.copied = true;
            self.tooltip = Some(ControlId::Copy);
        }
    }

    /// Revert the copied indicator (one-shot timer elapsed).
    pub fn copy_revert(&mut self) {
        self.copied = false;
    }

    /// Pointer moved to hovering `c` (or nothing). Hover is suppressed on
    /// disabled controls.
    pub fn set_hover(&mut self, c: Option<ControlId>) {
        let c = c.filter(|c| !self.disabled(*c));
        if self.interaction.hover == c {
            return;
        }
        self.interaction.hover = c;
        self.tooltip = None; // tooltip re-arms via its delay in the host
    }

    /// Move keyboard focus to `c` (or clear). `focus_visible` selects ring.
    pub fn set_focus(&mut self, c: Option<ControlId>, keyboard: bool) {
        let c = c.filter(|c| !self.disabled(*c) || *c == ControlId::Editor);
        // a disabled send is skipped by tab order, but Editor remains focusable
        self.interaction.focus = c;
        self.interaction.focus_visible = keyboard && c.is_some() && c != Some(ControlId::Editor);
    }

    /// Tab/Shift+Tab focus cycling: Editor -> Send/Stop -> Copy (when the copy
    /// button exists). Disabled controls are skipped.
    pub fn cycle_focus(&mut self, backwards: bool) {
        let mut order: Vec<ControlId> = Vec::with_capacity(3);
        order.push(ControlId::Editor);
        order.push(self.action_control());
        if self.surface == Surface::Response {
            order.push(ControlId::Copy);
        }
        order.retain(|c| !self.disabled(*c) || *c == ControlId::Editor);
        let cur = self.interaction.focus;
        let next = match cur.and_then(|f| order.iter().position(|c| *c == f)) {
            None => order[0],
            Some(i) => {
                let n = order.len();
                order[(i + if backwards { n - 1 } else { 1 }) % n]
            }
        };
        self.interaction.focus = Some(next);
        self.interaction.focus_visible = next != ControlId::Editor;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn composer_state() -> UiState {
        let mut s = UiState::default();
        s.open();
        s
    }

    #[test]
    fn submit_requires_text_and_no_ime() {
        let mut s = composer_state();
        assert!(!s.submit(), "empty editor must not submit");
        s.editor_empty = false;
        s.composing = true;
        assert!(!s.submit(), "IME composition must not submit");
        s.composing = false;
        assert!(s.submit());
        assert_eq!(s.activity, Activity::Submitting);
        assert!(!s.submit(), "already submitting");
    }

    #[test]
    fn stop_restores_editable_composer() {
        let mut s = composer_state();
        s.editor_empty = false;
        s.submit();
        s.stop();
        assert_eq!(s.activity, Activity::Idle);
        assert!(s.editor_editable());
        assert!(!s.editor_empty, "submitted text is preserved");
    }

    #[test]
    fn response_arrival_swaps_surface() {
        let mut s = composer_state();
        s.editor_empty = false;
        s.submit();
        s.response_arrived();
        assert_eq!(s.surface, Surface::Response);
        assert_eq!(s.activity, Activity::Idle);
        assert!(s.editor_empty, "follow-up composer starts empty");
    }

    #[test]
    fn escape_hides_and_open_shows() {
        let mut s = composer_state();
        s.escape();
        assert_eq!(s.surface, Surface::Hidden);
        s.open();
        assert_eq!(s.surface, Surface::Composer);
        assert_eq!(s.interaction.focus, Some(ControlId::Editor));
    }

    #[test]
    fn disabled_send_has_no_hover_or_focus() {
        let mut s = composer_state();
        s.set_hover(Some(ControlId::Send));
        assert_eq!(s.interaction.hover, None, "disabled send is not hoverable");
        s.cycle_focus(false);
        assert_eq!(s.interaction.focus, Some(ControlId::Editor));
        s.cycle_focus(false);
        // Send is disabled -> skipped; no copy on Composer surface -> back to Editor
        assert_eq!(s.interaction.focus, Some(ControlId::Editor));
        s.editor_empty = false;
        s.cycle_focus(false);
        assert_eq!(s.interaction.focus, Some(ControlId::Send));
        assert!(s.interaction.focus_visible);
    }

    #[test]
    fn focus_visible_only_for_keyboard_non_editor() {
        let mut s = composer_state();
        s.editor_empty = false;
        s.set_focus(Some(ControlId::Send), true);
        assert!(s.interaction.focus_visible);
        s.set_focus(Some(ControlId::Editor), true);
        assert!(
            !s.interaction.focus_visible,
            "editor never shows a focus ring"
        );
        s.set_focus(Some(ControlId::Send), false);
        assert!(!s.interaction.focus_visible, "pointer focus shows no ring");
    }

    #[test]
    fn copy_only_on_response() {
        let mut s = composer_state();
        s.copy();
        assert!(!s.copied);
        s.editor_empty = false;
        s.submit();
        s.response_arrived();
        s.copy();
        assert!(s.copied);
        s.copy_revert();
        assert!(!s.copied);
    }
}
