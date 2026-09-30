//! mascot-ui-lab: interactive playground + evidence tooling for the Mascot
//! native UI foundation (v0.1 Phase A).
//!
//!   mascot-ui-lab                  interactive window (Esc to hide, F1 theme,
//!                                  F2 placement, F3/F4 presets, F5 scale)
//!   mascot-ui-lab capture --out DIR [--allow-dirty]
//!   mascot-ui-lab components [--capture DIR] [--allow-dirty]
//!   mascot-ui-lab perf --out FILE.json [--runs N]
//!   mascot-ui-lab perf-ab --baseline-exe EXE --baseline-head SHA --out DIR
//!                          [--rounds 2] [--runs 5] [--allow-dirty]
//!   mascot-ui-lab selftest --out DIR
//!   mascot-ui-lab stamp-evidence DIR [DIR...]
//!   mascot-ui-lab diff-images DIR_A DIR_B [--out FILE]

mod capture;
mod components;
mod diff_images;
mod inventory;
mod perf;
mod perf_ab;
mod presets;
mod selftest;
mod stamp_evidence;

use mascot_animation::Rig;
use mascot_render_win32::renderer::DeviceKind;
use mascot_ui::Theme;
use mascot_ui::state::Placement;
use mascot_ui_win32::app;
use std::path::PathBuf;

fn find_rig() -> Result<Rig, String> {
    let mut args_rig: Option<PathBuf> = None;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        if a == "--rig" {
            args_rig = it.next().map(PathBuf::from);
        }
    }
    if let Some(d) = args_rig {
        return Rig::load(&d).map_err(|e| format!("rig load {}: {e:?}", d.display()));
    }
    // walk up from the exe/cwd to find assets/mascot/rig-v0.2
    for base in [
        std::env::current_dir().ok(),
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf())),
    ] {
        let Some(mut dir) = base else { continue };
        loop {
            let cand = dir.join("assets/mascot/rig-v0.2");
            if cand.join("rig.json").exists() {
                return Rig::load(&cand).map_err(|e| format!("rig load: {e:?}"));
            }
            if !dir.pop() {
                break;
            }
        }
    }
    Err("rig dir not found (use --rig DIR)".to_string())
}

fn get_arg(name: &str) -> Option<String> {
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        if a == name {
            return it.next();
        }
    }
    None
}

fn has_flag(name: &str) -> bool {
    std::env::args().any(|a| a == name)
}

/// Positional args (everything that isn't the subcommand, a `--flag`, or a
/// flag's value).
fn positional_args() -> Vec<String> {
    let mut out = Vec::new();
    let mut it = std::env::args().skip(1).peekable();
    let _ = it.next(); // subcommand
    while let Some(a) = it.next() {
        if a.starts_with("--") {
            // flags that take a value consume the next arg
            if !a.contains('=') && matches!(it.peek(), Some(n) if !n.starts_with("--")) {
                // known valued flags
                if matches!(
                    a.as_str(),
                    "--out"
                        | "--runs"
                        | "--rounds"
                        | "--rig"
                        | "--capture"
                        | "--references"
                        | "--scale"
                        | "--theme"
                        | "--placement"
                        | "--state"
                        | "--baseline-exe"
                        | "--baseline-head"
                ) {
                    it.next();
                    continue;
                }
            }
            continue;
        }
        out.push(a);
    }
    out
}

fn main() {
    if let Err(e) = real_main() {
        eprintln!("error: {e}");
        std::process::exit(2);
    }
}

fn real_main() -> Result<(), String> {
    let sub = std::env::args().nth(1).unwrap_or_default();
    match sub.as_str() {
        "capture" => capture::run(),
        "components" => components::run(),
        "perf" => perf::run(),
        "perf-child" => perf::run_child(),
        "perf-ab" => perf_ab::run(),
        "selftest" => selftest::run(),
        "stamp-evidence" => stamp_evidence::run(),
        "diff-images" => diff_images::run(),
        "" | "interactive" | "lab" => interactive(),
        other => Err(format!("unknown subcommand '{other}'")),
    }
}

fn interactive() -> Result<(), String> {
    use mascot_ui_win32::app::*;
    let rig = find_rig()?;
    let scale = get_arg("--scale")
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(1.0);
    let mut app = App::new(DeviceKind::Hardware, rig, Default::default(), scale)
        .map_err(|e| format!("app: {e}"))?;

    let theme = get_arg("--theme").map(|t| t == "dark").unwrap_or(false);
    if theme {
        app.state.theme = Theme::Dark;
        app.apply_theme();
    }
    if let Some(pl) = get_arg("--placement") {
        app.state.placement = if pl == "right" {
            Placement::Right
        } else {
            Placement::Left
        };
        let _ = app.rebuild_sprite();
    }
    if let Some(st) = get_arg("--state")
        && !presets::apply(&mut app, &st, false)
    {
        eprintln!("unknown state '{st}'");
    }

    // Lab hotkeys: F1 theme, F2 placement, F3/F4 preset cycle, F5 scale.
    let mut preset_idx = 0usize;
    const SCALES: [f32; 4] = [1.0, 1.25, 1.5, 2.0];
    let mut scale_idx = SCALES
        .iter()
        .position(|s| (*s - scale).abs() < 0.01)
        .unwrap_or(0);
    app.on_hotkey = Some(Box::new(move |app: &mut App, vk: u32| {
        use windows::Win32::UI::Input::KeyboardAndMouse::*;
        match vk {
            v if v == VK_F1.0 as u32 => {
                app.state.theme = app.state.theme.other();
                app.apply_theme();
                app.dirty = true;
            }
            v if v == VK_F2.0 as u32 => {
                app.state.placement = app.state.placement.other();
                let _ = app.rebuild_sprite();
                app.dirty = true;
            }
            v if v == VK_F3.0 as u32 || v == VK_F4.0 as u32 => {
                let n = presets::PRESETS.len();
                preset_idx = (preset_idx + if v == VK_F4.0 as u32 { 1 } else { n - 1 }) % n;
                presets::apply(app, presets::PRESETS[preset_idx].id, false);
            }
            v if v == VK_F5.0 as u32 => {
                scale_idx = (scale_idx + 1) % SCALES.len();
                app.scale = SCALES[scale_idx];
                let _ = app.rebuild_sprite();
                app.dirty = true;
            }
            _ => {}
        }
    }));

    app.open_composer();
    // anchor: the near screen edge mirrors with placement — Left anchors the
    // window's left edge near the work-area left, Right its right edge.
    let (ax, ay);
    unsafe {
        let wa = {
            use windows::Win32::UI::WindowsAndMessaging::*;
            let mut rc = windows::Win32::Foundation::RECT::default();
            let _ = SystemParametersInfoW(
                SPI_GETWORKAREA,
                0,
                Some(&mut rc as *mut _ as *mut std::ffi::c_void),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            );
            rc
        };
        ax = match app.state.placement {
            Placement::Right => wa.right - 60,
            Placement::Left => wa.left + 60,
        };
        ay = wa.bottom - 60;
    }
    app::run(app, ax, ay).map_err(|e| format!("{e}"))
}
