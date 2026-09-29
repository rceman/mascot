//! Deterministic lab presets: every state the UI can show, reachable from the
//! interactive hotkeys and from `capture`.

use mascot_ui::state::{Activity, ControlId, Interaction, Placement, Surface};
use mascot_ui_win32::app::App;

/// A named UI state preset.
pub struct Preset {
    pub id: &'static str,
    pub apply: fn(&mut App),
}

/// All presets in sheet order.
pub const PRESETS: &[Preset] = &[
    Preset {
        id: "mascot-only",
        apply: mascot_only,
    },
    Preset {
        id: "composer-empty",
        apply: composer_empty,
    },
    Preset {
        id: "composer-focused",
        apply: composer_focused,
    },
    Preset {
        id: "composer-text",
        apply: composer_text,
    },
    Preset {
        id: "composer-multiline",
        apply: composer_multiline,
    },
    Preset {
        id: "composer-selection",
        apply: composer_selection,
    },
    Preset {
        id: "submitting",
        apply: submitting,
    },
    Preset {
        id: "response",
        apply: response,
    },
    Preset {
        id: "response-followup",
        apply: response_followup,
    },
    Preset {
        id: "send-hover",
        apply: send_hover,
    },
    Preset {
        id: "send-pressed",
        apply: send_pressed,
    },
    Preset {
        id: "send-focus-visible",
        apply: send_focus_visible,
    },
    Preset {
        id: "stop-hover",
        apply: stop_hover,
    },
    Preset {
        id: "copy-hover-tooltip",
        apply: copy_hover_tooltip,
    },
    Preset {
        id: "copy-copied",
        apply: copy_copied,
    },
];

pub fn by_id(id: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.id == id)
}

/// Right-placement variants rendered for the contact sheets.
pub const RIGHT_VARIANTS: &[&str] = &["composer-text", "response", "send-hover"];

fn base(a: &mut App) {
    a.state.surface = Surface::Composer;
    a.state.interaction = Interaction::default();
    a.state.activity = Activity::Idle;
    a.state.composing = false;
    a.state.copied = false;
    a.state.tooltip = None;
    let _ = a.editor.set_text("");
    a.editor.set_read_only(false);
    a.editor.dim_text(None);
    a.state.editor_empty = true;
    a.dirty = true;
}

fn mascot_only(a: &mut App) {
    base(a);
    a.state.surface = Surface::Hidden;
}

fn composer_empty(a: &mut App) {
    base(a);
    // placeholder drawn by painter (unfocused, send disabled)
}

/// Focus the editor and put the caret at the end of any typed text.
fn focus_editor_caret_end(a: &mut App) {
    a.state.set_focus(Some(ControlId::Editor), false);
    a.editor
        .send(windows::Win32::UI::WindowsAndMessaging::WM_SETFOCUS, 0, 0);
    let len = a.editor.text().encode_utf16().count() as i32;
    a.editor.set_selection(len, len);
}

fn composer_focused(a: &mut App) {
    base(a);
    focus_editor_caret_end(a);
}

fn composer_text(a: &mut App) {
    base(a);
    let _ = a.editor.set_text("Refactor the rig loader to stream parts");
    a.state.editor_empty = false;
    focus_editor_caret_end(a);
}

fn composer_multiline(a: &mut App) {
    base(a);
    let _ = a.editor.set_text("Überprüfe die Verkabelung\nmit einem sauberen Delta\n日本語の行も入れる\nand a fourth line here");
    a.state.editor_empty = false;
    focus_editor_caret_end(a);
}

fn composer_selection(a: &mut App) {
    composer_text(a);
    a.editor.set_selection(0, 8); // "Refactor"
}

fn submitting(a: &mut App) {
    composer_text(a);
    // the real submit path: read-only + dimmed text, no caret, focus on Stop
    a.submit();
}

fn response(a: &mut App) {
    base(a);
    let _ = a.editor.set_text("");
    a.state.surface = Surface::Response;
}

fn response_followup(a: &mut App) {
    response(a);
    let _ = a.editor.set_text("follow-up: also check the DComp path");
    a.state.editor_empty = false;
    a.editor
        .send(windows::Win32::UI::WindowsAndMessaging::WM_SETFOCUS, 0, 0);
    a.state.set_focus(Some(ControlId::Editor), false);
}

fn send_hover(a: &mut App) {
    composer_text(a);
    a.state.set_hover(Some(ControlId::Send));
    a.state.interaction.focus = None;
}

fn send_pressed(a: &mut App) {
    send_hover(a);
    a.state.interaction.pressed = Some(ControlId::Send);
}

fn send_focus_visible(a: &mut App) {
    composer_text(a);
    a.state.set_focus(Some(ControlId::Send), true);
}

fn stop_hover(a: &mut App) {
    submitting(a);
    a.state.set_hover(Some(ControlId::Stop));
}

fn copy_hover_tooltip(a: &mut App) {
    response(a);
    a.state.set_hover(Some(ControlId::Copy));
    a.state.tooltip = Some(ControlId::Copy);
}

fn copy_copied(a: &mut App) {
    response(a);
    a.state.copy();
}

/// Applies a preset id (+ optional right placement).
pub fn apply(a: &mut App, id: &str, right: bool) -> bool {
    let Some(p) = by_id(id) else { return false };
    (p.apply)(a);
    let want = if right {
        Placement::Right
    } else {
        Placement::Left
    };
    if a.state.placement != want {
        a.state.placement = want;
        let _ = a.rebuild_sprite();
    }
    // presets mutate state directly — drain editor Change events (they
    // flag re-measurement) then refresh geometry before painting
    a.process_editor_events();
    a.relayout().ok();
    a.dirty = true;
    true
}
