#![cfg(windows)]
use mascot_ui_win32::edit::{Editor, EditorConfig};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::Controls::RichEdit::*;
use windows::Win32::UI::WindowsAndMessaging::*;

fn cfg() -> EditorConfig {
    EditorConfig {
        face: "Segoe UI".into(),
        size_twips: 210,
        fg: [0.0, 0.0, 0.0, 1.0],
        sel_bg: [0.0, 0.0, 0.0, 0.2],
        sel_fg: [0.0, 0.0, 0.0, 1.0],
        read_only: false,
    }
}

#[test]
fn emptiness_flags() {
    let ed = Editor::new(
        HWND::default(),
        RECT {
            left: 0,
            top: 0,
            right: 336,
            bottom: 40,
        },
        1.0,
        &cfg(),
    )
    .unwrap();
    let gtl = GETTEXTLENGTHEX {
        flags: GTL_PRECISE | GTL_NUMCHARS,
        codepage: 1200,
    };
    let n0 = ed.send(EM_GETTEXTLENGTHEX, &gtl as *const _ as usize, 0);
    println!(
        "empty: gtl={n0} is_empty={} text={:?}",
        ed.is_empty(),
        ed.text()
    );
    ed.set_text("hello").unwrap();
    let n1 = ed.send(EM_GETTEXTLENGTHEX, &gtl as *const _ as usize, 0);
    println!(
        "5ch: gtl={n1} is_empty={} text={:?}",
        ed.is_empty(),
        ed.text()
    );
    // multiline
    let lines = "l1\nl2\nl3\nl4\nl5\nl6\nl7\nl8\nl9\nl10\nl11\nl12";
    ed.set_text(lines).unwrap();
    let n2 = ed.send(EM_GETTEXTLENGTHEX, &gtl as *const _ as usize, 0);
    println!(
        "12ln: gtl={n2} is_empty={} len={}",
        ed.is_empty(),
        ed.text().len()
    );
}

#[test]
fn natural_size_and_set_text() {
    let ed = Editor::new(
        HWND::default(),
        RECT {
            left: 0,
            top: 0,
            right: 336,
            bottom: 40,
        },
        1.0,
        &cfg(),
    )
    .unwrap();
    ed.set_text("line1\nline2\nline3").unwrap();
    let t = ed.text();
    let (w, h) = ed.natural_size(336.0).unwrap();
    println!("text={t:?} natural={w}x{h}");
    assert!(t.contains("line2"), "set_text failed: {t:?}");
    assert!(h > 40.0, "natural height didn't grow: {h}");
}

/// EN_REQUESTRESIZE replies in the editor's client units — DIPs, so the
/// natural size is scale-independent.
#[test]
fn request_resize_units() {
    for scale in [1.0f32, 2.0] {
        let ed = Editor::new(
            HWND::default(),
            RECT {
                left: 0,
                top: 0,
                right: 336,
                bottom: 40,
            },
            scale,
            &cfg(),
        )
        .unwrap();
        ed.set_text("line1\nline2\nline3\nline4").unwrap();
        let raw0 = ed.natural_size_raw();
        let (w, h) = ed.natural_size(336.0).unwrap();
        println!(
            "scale={scale} client=336x40 raw={}x{} dip={w:.0}x{h:.0}",
            raw0.cx, raw0.cy
        );
        // client rect is DIP: 4 lines at ~19 DIP at any scale
        assert!((h - 76.0).abs() < 8.0, "scale {scale}: h={h}");
        assert_eq!(raw0.cy, h as i32, "natural_size is DIP at scale {scale}");
    }
}

#[test]
fn copy_path() {
    let ed = Editor::new(
        HWND::default(),
        RECT {
            left: 0,
            top: 0,
            right: 336,
            bottom: 40,
        },
        1.0,
        &cfg(),
    )
    .unwrap();
    ed.set_text("copyme").unwrap();
    ed.send(0x00B1 /*EM_SETSEL*/, 0, -1);
    let r = ed.send(WM_COPY, 0, 0);
    println!("WM_COPY ret={r}");
    unsafe {
        use windows::Win32::System::DataExchange::*;
        use windows::Win32::System::Memory::*;
        let mut txt = String::new();
        for _ in 0..10 {
            if OpenClipboard(None).is_ok() {
                if let Ok(h) = GetClipboardData(13u32 /*CF_UNICODETEXT*/) {
                    let p = GlobalLock(windows::Win32::Foundation::HGLOBAL(h.0));
                    if !p.is_null() {
                        txt = windows::core::PCWSTR(p as _)
                            .to_string()
                            .unwrap_or_default();
                        let _ = GlobalUnlock(windows::Win32::Foundation::HGLOBAL(h.0));
                    }
                }
                let _ = CloseClipboard();
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        println!("clipboard={txt:?}");
    }
}

/// The editor's coordinate space is DIP: scale changes must not affect the
/// reported natural size.
#[test]
fn dpi_notify_probe() {
    let mut ed = Editor::new(
        HWND::default(),
        RECT {
            left: 0,
            top: 0,
            right: 314,
            bottom: 76,
        },
        1.0,
        &cfg(),
    )
    .unwrap();
    ed.set_text("line1\nline2\nline3\nline4").unwrap();
    let r0 = ed.natural_size_raw();
    println!("created@1: raw={}x{}", r0.cx, r0.cy);

    ed.set_scale(2.0).unwrap();
    let r1 = ed.natural_size_raw();
    println!("after set_scale(2): raw={}x{}", r1.cx, r1.cy);
    assert_eq!(r0.cy, r1.cy, "DIP space is scale-independent");
}

/// Replicates capture.rs dpi_sheet: create at 1.0 then switch scale — the
/// client-rect / measure ordering must still produce a full-size composer.
#[test]
fn app_scale_switch() {
    let mut dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let rig = loop {
        let cand = dir.join("assets/mascot/rig-v0.2");
        if cand.join("rig.json").exists() {
            break mascot_animation::Rig::load(&cand).unwrap();
        }
        if !dir.pop() {
            panic!("rig not found");
        }
    };
    let mut app = mascot_ui_win32::app::App::new(
        mascot_render_win32::renderer::DeviceKind::Warp,
        rig.clone(),
        HWND::default(),
        1.0,
    )
    .unwrap();
    app.open_composer();
    let _ = app
        .editor
        .set_text("Überprüfe die Verkabelung\nmit einem sauberen Delta\n日本語の行も入れる\nand a fourth line here");
    app.state.editor_empty = false;
    app.process_editor_events();
    app.relayout().unwrap();
    app.render_offscreen().unwrap();
    // now the scale switch, like dpi_sheet does
    app.scale = 2.0;
    app.editor.set_scale(2.0).unwrap();
    let _ = app.rebuild_sprite();
    app.process_editor_events();
    app.relayout().unwrap();
    // replicate presets::apply's full call set on the editor
    // app.editor.dim_text(None); // base()
    let _ = app.editor.set_text("");
    let _ = app
        .editor
        .set_text("Überprüfe die Verkabelung\nmit einem sauberen Delta\n日本語の行も入れる\nand a fourth line here");
    app.editor.send(WM_SETFOCUS, 0, 0);
    app.editor.set_selection(i32::MAX / 2, i32::MAX / 2);
    app.process_editor_events();
    app.relayout().unwrap();
    // capture.rs then applies a theme (OnTxPropertyBitsChange) before draw
    app.state.theme = mascot_ui::Theme::Dark;
    app.apply_theme();
    let img = app.render_offscreen().unwrap();
    img.save(&std::env::temp_dir().join("probe-switch-2.png"))
        .unwrap();
    println!(
        "switched: editor={:?} eh={} client={:?}",
        app.layout.editor,
        0,
        app.editor.client_rect()
    );
}

/// Fresh app borrow for a single statement — `*app` is also written by the
/// wndproc through `APP_PTR` inside `pump_once`, so a `&mut` bound across a
/// pump lets LLVM cache field loads (the `enter-submits` stale-read bug).
fn am<'x>(app: *mut mascot_ui_win32::app::App) -> &'x mut mascot_ui_win32::app::App {
    unsafe { &mut *app }
}

/// Caret blink: after focus, the host timer should toggle the caret (presents
/// during blink), then freeze after SPI_GETCARETTIMEOUT.
#[test]
fn caret_blink_presents() {
    let mut dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let rig = loop {
        let cand = dir.join("assets/mascot/rig-v0.2");
        if cand.join("rig.json").exists() {
            break mascot_animation::Rig::load(&cand).unwrap();
        }
        if !dir.pop() {
            panic!("rig not found");
        }
    };
    let mut app = mascot_ui_win32::app::App::new(
        mascot_render_win32::renderer::DeviceKind::Warp,
        rig,
        HWND::default(),
        1.0,
    )
    .unwrap();
    app.open_composer();
    let app_ptr: *mut mascot_ui_win32::app::App = Box::leak(Box::new(app));
    let hwnd = mascot_ui_win32::app::create(unsafe { &mut *app_ptr }, 60, 60).unwrap();
    let app = app_ptr;
    am(app).focus_editor();
    for _ in 0..20 {
        mascot_ui_win32::app::pump_once();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let p0 = am(app).present_count;
    let t0 = std::time::Instant::now();
    while t0.elapsed().as_millis() < 1800 {
        mascot_ui_win32::app::pump_once();
        std::thread::sleep(std::time::Duration::from_millis(4));
    }
    let blink_frames = am(app).present_count - p0;
    let (created, pos, size) = am(app).editor.caret_info();
    println!("blink_presents={blink_frames} caret=({created} {pos:?} {size:?}) hwnd={hwnd:?}");
    assert!(created, "caret never created");
    assert!(
        blink_frames >= 2,
        "expected blink toggles, got {blink_frames}"
    );
}

/// Re-entrancy: clicking the editor while the window does NOT have OS focus
/// makes richedit's `TxSetFocus` call `SetFocus(hwnd)`, which delivers a
/// synchronous `WM_SETFOCUS` while `wm_lbuttondown` is still on the stack —
/// a nested `with_app`. The in_app guard must skip it (counted in
/// `reentry_skips`) and the outer handler must leave focus/caret consistent
/// itself.
#[test]
fn click_editor_reentry() {
    let mut dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let rig = loop {
        let cand = dir.join("assets/mascot/rig-v0.2");
        if cand.join("rig.json").exists() {
            break mascot_animation::Rig::load(&cand).unwrap();
        }
        if !dir.pop() {
            panic!("rig not found");
        }
    };
    let mut app = mascot_ui_win32::app::App::new(
        mascot_render_win32::renderer::DeviceKind::Warp,
        rig,
        HWND::default(),
        1.0,
    )
    .unwrap();
    app.open_composer();
    let app_ptr: *mut mascot_ui_win32::app::App = Box::leak(Box::new(app));
    // no_activate: the window never takes OS focus, so richedit's
    // SetFocus(hwnd) inside TxSetFocus is a real focus change
    let hwnd = mascot_ui_win32::app::create_opts(unsafe { &mut *app_ptr }, 60, 60, true).unwrap();
    let app = app_ptr;
    assert_eq!(
        unsafe { windows::Win32::UI::Input::KeyboardAndMouse::GetFocus() },
        HWND::default(),
        "precondition: our window must not already hold focus"
    );
    let skips0 = mascot_ui_win32::app::reentry_skips();
    // click inside the editor rect (scale 1: DIP == px)
    let e = am(app).layout.editor.unwrap();
    let (cx, cy) = ((e.x + e.w / 2.0) as i32, (e.y + e.h / 2.0) as i32);
    let lp = ((cy as u32) << 16 | (cx as u32 & 0xFFFF)) as isize;
    unsafe {
        use windows::Win32::Foundation::{LPARAM, WPARAM};
        use windows::Win32::System::SystemServices::MK_LBUTTON;
        let _ = PostMessageW(
            Some(hwnd),
            WM_LBUTTONDOWN,
            WPARAM(MK_LBUTTON.0 as usize),
            LPARAM(lp),
        );
        let _ = PostMessageW(Some(hwnd), WM_LBUTTONUP, WPARAM(0), LPARAM(lp));
    }
    for _ in 0..10 {
        mascot_ui_win32::app::pump_once();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let skips1 = mascot_ui_win32::app::reentry_skips();
    println!(
        "reentry_skips {skips0} -> {skips1} (delta {})",
        skips1 - skips0
    );
    assert_eq!(
        am(app).state.interaction.focus,
        Some(mascot_ui::ControlId::Editor),
        "click must focus the editor even when the nested WM_SETFOCUS is skipped"
    );
    assert!(am(app).editor.caret().is_some(), "caret created+shown");
    unsafe {
        let _ = DestroyWindow(hwnd);
        while mascot_ui_win32::app::pump_once() {}
    }
    mascot_ui_win32::app::unbind();
}

/// Hover tooltips via REAL input: SendInput moves the cursor onto a control,
/// the 500 ms hover delay fires TIMER_TOOLTIP, `state.tooltip` rises, and
/// moving off the window delivers a genuine WM_MOUSELEAVE that clears it.
/// (Posted WM_MOUSEMOVE can't fake "cursor is over the window" —
/// TrackMouseEvent reports a leave immediately.)
#[test]
fn hover_tooltip_and_leave() {
    let mut dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let rig = loop {
        let cand = dir.join("assets/mascot/rig-v0.2");
        if cand.join("rig.json").exists() {
            break mascot_animation::Rig::load(&cand).unwrap();
        }
        if !dir.pop() {
            panic!("rig not found");
        }
    };
    let mut app = mascot_ui_win32::app::App::new(
        mascot_render_win32::renderer::DeviceKind::Warp,
        rig,
        HWND::default(),
        1.0,
    )
    .unwrap();
    app.open_composer();
    let app_ptr: *mut mascot_ui_win32::app::App = Box::leak(Box::new(app));
    // visible (real input needs a hit-testable window), anchored on-screen
    let hwnd =
        mascot_ui_win32::app::create_opts(unsafe { &mut *app_ptr }, 100, 460, false).unwrap();
    let app = app_ptr;
    let pump_ms = |ms: u64| {
        let t0 = std::time::Instant::now();
        while t0.elapsed().as_millis() < ms as u128 {
            mascot_ui_win32::app::pump_once();
            std::thread::sleep(std::time::Duration::from_millis(4));
        }
    };
    // absolute cursor move via SendInput -> real WM_MOUSEMOVE/WM_MOUSELEAVE
    let move_cursor = |cx: i32, cy: i32| unsafe {
        use windows::Win32::UI::Input::KeyboardAndMouse::*;
        use windows::Win32::UI::WindowsAndMessaging::{
            GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
            SM_YVIRTUALSCREEN,
        };
        let (vx, vy) = (
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
        );
        let (vw, vh) = (
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        );
        let mi = MOUSEINPUT {
            dx: ((cx - vx) * 65535 + (vw - 2)) / (vw - 1),
            dy: ((cy - vy) * 65535 + (vh - 2)) / (vh - 1),
            mouseData: 0,
            dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
            time: 0,
            dwExtraInfo: 0,
        };
        let inp = windows::Win32::UI::Input::KeyboardAndMouse::INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 { mi },
        };
        SendInput(&[inp], std::mem::size_of_val(&inp) as i32);
    };
    // remember where the user's cursor was so we can put it back
    let mut saved = windows::Win32::Foundation::POINT::default();
    unsafe {
        let _ = GetCursorPos(&mut saved);
    }

    // reach the Response surface (Copy button exists)
    let _ = am(app).editor.set_text("probe");
    am(app).state.editor_empty = false;
    am(app).submit();
    am(app).response_arrived();
    am(app).relayout().unwrap();
    pump_ms(100);

    // window is borderless: client coords == window-rect offsets; the rect
    // must be re-read after surface changes (the window resizes)
    let mut rc = RECT::default();
    unsafe {
        let _ = GetWindowRect(hwnd, &mut rc);
    }

    // hover Copy: inside the delay window the tooltip must NOT be up
    let e = am(app).layout.copy.unwrap();
    let (sx, sy) = (
        rc.left + (e.x + e.w / 2.0) as i32,
        rc.top + (e.y + e.h / 2.0) as i32,
    );
    // step-pump and anchor "hover began" at the app-observed hover (the
    // moment the app arms TIMER_TOOLTIP — a foreign cursor move by a
    // parallel test can't skew our own injection anchor otherwise).
    let motion0 = am(app)
        .present_reasons
        .iter()
        .filter(|r| r.as_str() == "motion")
        .count();
    let mut hover_observed = false;
    for _ in 0..30 {
        move_cursor(sx, sy);
        pump_ms(10);
        if am(app).state.interaction.hover == Some(mascot_ui::ControlId::Copy) {
            hover_observed = true;
            break;
        }
    }
    // keep hovering ~250 ms real time, watching whether the tooltip shows
    let mut elapsed_ms;
    let t_hover = std::time::Instant::now();
    loop {
        pump_ms(25);
        move_cursor(sx, sy);
        elapsed_ms = t_hover.elapsed().as_millis();
        if am(app).state.tooltip.is_some() || elapsed_ms >= 250 {
            break;
        }
    }
    let motion_n = am(app)
        .present_reasons
        .iter()
        .filter(|r| r.as_str() == "motion")
        .count()
        - motion0;
    assert!(hover_observed, "cursor over Copy must set hover");
    // invariant on the app's own clock: a visible tooltip must have been
    // armed >= TOOLTIP_DELAY_MS earlier, within one USER timer tick
    // (SetTimer is quantised to the ~15.6 ms system tick and may fire
    // up to one tick early relative to a QPC-based Instant)
    const TIMER_TICK_MS: u128 = 16;
    if am(app).state.tooltip.is_some() {
        let (armed, shown) = (am(app).tooltip_armed_at, am(app).tooltip_shown_at);
        let arm_to_show = match (armed, shown) {
            (Some(a), Some(sh)) => (sh - a).as_millis(),
            _ => 0,
        };
        assert!(
            arm_to_show + TIMER_TICK_MS >= mascot_ui::theme::tokens::TOOLTIP_DELAY_MS as u128,
            "tooltip shown {arm_to_show} ms after arming < TOOLTIP_DELAY_MS \
             (elapsed_since_inject={elapsed_ms} ms, motion_frames={motion_n})"
        );
    }
    // past the delay: tooltip up, and its rect is laid out
    pump_ms(500);
    assert_eq!(
        am(app).state.tooltip,
        Some(mascot_ui::ControlId::Copy),
        "tooltip must appear after TOOLTIP_DELAY_MS"
    );
    assert!(am(app).layout.tooltip.is_some());

    // move off the window: real WM_MOUSELEAVE clears hover + tooltip + rect
    move_cursor(rc.right + 60, rc.bottom + 60);
    pump_ms(150);
    assert_eq!(am(app).state.interaction.hover, None);
    assert_eq!(am(app).state.tooltip, None);
    assert!(am(app).layout.tooltip.is_none());

    // Send on the composer: hover -> tooltip, and its label measures > 0
    am(app).state.escape();
    am(app).open_composer();
    let _ = am(app).editor.set_text("probe");
    am(app).state.editor_empty = false;
    am(app).relayout().unwrap();
    unsafe {
        let _ = GetWindowRect(hwnd, &mut rc);
    }
    pump_ms(50);
    let s = am(app).layout.send.unwrap();
    let (sx, sy) = (
        rc.left + (s.x + s.w / 2.0) as i32,
        rc.top + (s.y + s.h / 2.0) as i32,
    );
    move_cursor(sx, sy);
    pump_ms(700);
    assert_eq!(am(app).state.tooltip, Some(mascot_ui::ControlId::Send));
    assert!(
        am(app).measured.tooltip_text_w > 0.0,
        "Send tooltip label must measure a nonzero width"
    );

    unsafe {
        let _ = SetCursorPos(saved.x, saved.y);
        let _ = DestroyWindow(hwnd);
        while mascot_ui_win32::app::pump_once() {}
    }
    mascot_ui_win32::app::unbind();
}

/// The editor's client rect is in DIPs, so REQRESIZE returns the same size
/// at every scale for every activation shape — the regression guard for the
/// stale-dpi bug (a px-space client latched the activation-time dpi).
#[test]
fn dpi_latch_matrix() {
    for &sc in &[1.0f32, 1.25, 1.5, 2.0] {
        for &zero_activate in &[true, false] {
            let client = if zero_activate {
                RECT::default()
            } else {
                RECT {
                    left: 0,
                    top: 0,
                    right: 314,
                    bottom: 148,
                }
            };
            let ed = Editor::new(HWND::default(), client, sc, &cfg()).unwrap();
            ed.set_client_rect(RECT {
                left: 0,
                top: 0,
                right: 314,
                bottom: 148,
            });
            ed.set_text("x").unwrap();
            let (_w, h_dip) = ed.natural_size(314.0).unwrap();
            println!("scale={sc} zero_activate={zero_activate} h_dip={h_dip}");
            assert!(
                (h_dip - 19.0).abs() < 3.0,
                "scale {sc} zero={zero_activate}: h={h_dip}"
            );
        }
    }
}

/// Replays the dpi_sheet order (scale → theme × preset → render) and checks
/// the editor's single-line height tracks scale — catches stale unit mapping.
/// A second editor on "x" acts as the single-line-height probe.
#[test]
fn dpi_sheet_replay() {
    let mut dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let rig = loop {
        let cand = dir.join("assets/mascot/rig-v0.2");
        if cand.join("rig.json").exists() {
            break mascot_animation::Rig::load(&cand).unwrap();
        }
        if !dir.pop() {
            panic!("rig not found");
        }
    };
    let mut app = mascot_ui_win32::app::App::new(
        mascot_render_win32::renderer::DeviceKind::Warp,
        rig,
        HWND::default(),
        1.0,
    )
    .unwrap();
    app.open_composer();
    let themes = [mascot_ui::Theme::Light, mascot_ui::Theme::Dark];
    let states = ["composer-multiline", "response", "send-focus-visible"];
    let scales = [1.0f32, 1.25, 1.5, 2.0, 1.0]; // the capture loop + back to 1
    let mut base_h = 0.0f32;
    for &sc in &scales {
        app.scale = sc;
        app.editor.set_scale(sc).unwrap();
        app.rebuild_sprite().unwrap();
        for theme in themes {
            for st in states {
                // mirrors presets::apply + apply_theme from the lab
                match st {
                    "composer-multiline" => {
                        app.state.surface = mascot_ui::Surface::Composer;
                        let _ = app.editor.set_text(
                            "Überprüfe die Verkabelung\nmit einem sauberen Delta\n日本語の行も入れる\nand a fourth line here",
                        );
                    }
                    "response" => {
                        let _ = app.editor.set_text("");
                        app.state.surface = mascot_ui::Surface::Response;
                    }
                    _ => {
                        app.state.surface = mascot_ui::Surface::Composer;
                        let _ = app
                            .editor
                            .set_text("Refactor the rig loader to stream parts");
                        app.state.set_focus(Some(mascot_ui::ControlId::Send), true);
                    }
                }
                app.state.editor_empty = app.editor.is_empty();
                app.state.theme = theme;
                app.apply_theme();
                app.process_editor_events();
                app.relayout().unwrap();
                let img = app.render_offscreen().unwrap();
                img.save(
                    &std::env::temp_dir().join(format!("replay-{sc}-{}-{st}.png", theme.name())),
                )
                .unwrap();
                // invariant: natural_size already converts px->DIP, so a
                // correct editor returns the same single-line DIP height at
                // every scale; a stale unit mapping halves/doubles it
                app.editor.set_text("x").ok();
                let (_w, h1) = app.editor.natural_size(314.0).unwrap();
                if sc == 1.0 && st == states[0] && theme == themes[0] {
                    base_h = h1;
                }
                assert!(
                    (h1 - base_h).abs() <= 1.0,
                    "scale {sc} {} {st}: single-line h {h1} dip vs want {base_h}",
                    theme.name()
                );
                // caret stays inside the editor rect when shown
                let (created, pos, _sz) = app.editor.caret_info();
                if let Some(e) = app.layout.editor
                    && created
                {
                    assert!(
                        pos.x >= 0 && pos.y >= 0 && pos.x as f32 <= e.w && pos.y as f32 <= e.h,
                        "scale {sc} {} {st}: caret {pos:?} outside editor {e:?}",
                        theme.name()
                    );
                }
            }
        }
    }
}

/// Does a fresh D2D ctx fix the editor draw after a scale-1 draw? Draws the
/// switched editor through a *second* renderer's device context.
#[test]
fn editor_draw_fresh_ctx() {
    use windows::Win32::Graphics::Direct2D::Common::*;
    let mut dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let rig = loop {
        let cand = dir.join("assets/mascot/rig-v0.2");
        if cand.join("rig.json").exists() {
            break mascot_animation::Rig::load(&cand).unwrap();
        }
        if !dir.pop() {
            panic!("rig not found");
        }
    };
    let mut app = mascot_ui_win32::app::App::new(
        mascot_render_win32::renderer::DeviceKind::Warp,
        rig.clone(),
        HWND::default(),
        1.0,
    )
    .unwrap();
    app.open_composer();
    let _ = app.editor.set_text("line1\nline2\nline3\nline4");
    app.state.editor_empty = false;
    app.process_editor_events();
    app.relayout().unwrap();
    app.render_offscreen().unwrap(); // first draw at scale 1 on app.renderer.ctx

    app.scale = 2.0;
    app.editor.set_scale(2.0).unwrap();
    app.process_editor_events();
    app.relayout().unwrap();

    // draw the SAME editor through a brand-new device context at 192dpi
    let r2 = mascot_render_win32::renderer::Renderer::new(
        mascot_render_win32::renderer::DeviceKind::Warp,
    )
    .unwrap();
    unsafe {
        let bmp = r2.create_target_bitmap([872, 592]).unwrap();
        r2.ctx.SetTarget(&bmp);
        r2.ctx.SetDpi(192.0, 192.0);
        r2.ctx.BeginDraw();
        r2.ctx.Clear(Some(&D2D1_COLOR_F {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        }));
        app.editor
            .draw_d2d(&r2.ctx.clone().into(), (44.0, 20.0, 358.0, 96.0))
            .unwrap();
        let _ = r2.ctx.EndDraw(None, None);
        r2.ctx.SetTarget(None);
        let img = r2.read_back(&bmp, [872, 592]).unwrap();
        img.save(&std::env::temp_dir().join("probe-fresh-ctx.png"))
            .unwrap();
    }
    // and through the app's own ctx for comparison
    let img = app.render_offscreen().unwrap();
    img.save(&std::env::temp_dir().join("probe-same-ctx.png"))
        .unwrap();
}

/// App-level: multiline editor height at scale 2 must reflect all lines.
#[test]
fn app_multiline_scale2() {
    // find rig dir walking up from the crate
    let mut dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let rig = loop {
        let cand = dir.join("assets/mascot/rig-v0.2");
        if cand.join("rig.json").exists() {
            break mascot_animation::Rig::load(&cand).unwrap();
        }
        if !dir.pop() {
            panic!("rig not found");
        }
    };
    for scale in [1.0f32, 2.0] {
        let mut app = mascot_ui_win32::app::App::new(
            mascot_render_win32::renderer::DeviceKind::Warp,
            rig.clone(),
            HWND::default(),
            scale,
        )
        .unwrap();
        app.open_composer();
        let _ = app
            .editor
            .set_text("Überprüfe die Verkabelung\nmit einem sauberen Delta\n日本語の行も入れる\nand a fourth line here");
        app.state.editor_empty = false;
        app.process_editor_events();
        app.relayout().unwrap();
        let img = app.render_offscreen().unwrap();
        img.save(&std::env::temp_dir().join(format!("probe-multi-{scale}.png")))
            .unwrap();
        println!(
            "scale={scale} window={}x{} composer={:?} editor={:?} img={}x{}",
            app.layout.window.w,
            app.layout.window.h,
            app.layout.composer,
            app.layout.editor,
            img.width,
            img.height,
        );
    }
}

/// What timers does richedit install at rest (unfocused, no typing)?
#[test]
fn idle_timer_probe() {
    let ed = Editor::new(
        HWND::default(),
        RECT {
            left: 0,
            top: 0,
            right: 314,
            bottom: 40,
        },
        1.0,
        &cfg(),
    )
    .unwrap();
    ed.set_text("hello").unwrap();
    // drain creation-time events and print the timer ids left armed
    let evts = ed.drain_events();
    println!("created events: {evts:?}");
    ed.send(WM_SETFOCUS, 0, 0);
    let evts2 = ed.drain_events();
    println!("focus events: {evts2:?}");
    println!("timers armed: {:?}", ed.timer_ids());
    ed.send(WM_KILLFOCUS, 0, 0);
    let evts3 = ed.drain_events();
    println!("killfocus events: {evts3:?}");
    println!("timers armed after blur: {:?}", ed.timer_ids());
}

/// Memory attribution for the sprite stage: is the ~60MB jump the *first
/// raster on the device* (driver shader/effect heaps) or sprite-specific?
/// Snaps process private bytes after renderer init, after a trivial 16px
/// render, and after a full rig render.
#[test]
fn first_raster_memory() {
    fn priv_bytes() -> u64 {
        use windows::Win32::System::ProcessStatus::*;
        unsafe {
            let mut c: PROCESS_MEMORY_COUNTERS_EX = std::mem::zeroed();
            c.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
            let _ = GetProcessMemoryInfo(
                windows::Win32::System::Threading::GetCurrentProcess(),
                &mut c as *mut _ as *mut PROCESS_MEMORY_COUNTERS,
                c.cb,
            );
            c.PrivateUsage as u64
        }
    }
    fn threads() -> u32 {
        std::process::Command::new("powershell")
            .args(["-c", "(Get-Process -Id $PID).Threads.Count"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse().ok())
            .unwrap_or(0)
    }
    let t0 = threads();
    let mut r = mascot_render_win32::renderer::Renderer::new(
        mascot_render_win32::renderer::DeviceKind::Warp,
    )
    .unwrap();
    println!(
        "after Renderer::new: {:.1}MB t{}",
        priv_bytes() as f64 / 1e6,
        threads() - t0
    );
    // trivial first raster: a tiny bitmap + Clear only
    unsafe {
        let bmp = r.create_target_bitmap([16, 16]).unwrap();
        r.ctx.SetTarget(&bmp);
        r.ctx.SetDpi(96.0, 96.0);
        r.ctx.BeginDraw();
        r.ctx.Clear(None);
        let _ = r.ctx.EndDraw(None, None);
        r.ctx.SetTarget(None);
    }
    println!(
        "after trivial render: {:.1}MB t{}",
        priv_bytes() as f64 / 1e6,
        threads() - t0
    );
    // now the real rig render
    let mut dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let rig = loop {
        let cand = dir.join("assets/mascot/rig-v0.2");
        if cand.join("rig.json").exists() {
            break mascot_animation::Rig::load(&cand).unwrap();
        }
        if !dir.pop() {
            panic!("rig not found");
        }
    };
    r.load_rig(&rig).unwrap();
    println!(
        "after load_rig: {:.1}MB t{}",
        priv_bytes() as f64 / 1e6,
        threads() - t0
    );
    let _sprite = mascot_ui_win32::sprite::Sprite::render(&mut r, &rig, false, [80.0, 80.0], 2.0);
    println!(
        "after sprite render: {:.1}MB t{}",
        priv_bytes() as f64 / 1e6,
        threads() - t0
    );
}
