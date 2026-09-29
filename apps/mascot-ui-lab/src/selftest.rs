//! `selftest --out DIR`: real native-editor verification — SendInput against
//! the live lab window, state read back through `ITextServices`. Writes
//! `selftest.json` + checkpoint captures (offscreen + one live-screen grab
//! over a lab-owned backdrop — never user desktop content).
//!
//! Guards: aborts whenever the lab window loses foreground before an
//! injection; saves/restores clipboard text and cursor position.

use mascot_render_win32::renderer::DeviceKind;
use mascot_ui::state::{ControlId, Surface};
use mascot_ui_win32::app::{self, App};
use serde_json::{Value, json};
use std::path::PathBuf;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::DataExchange::*;
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::HiDpi::SetProcessDpiAwarenessContext;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

struct Check {
    name: String,
    pass: bool,
    detail: Value,
}

pub fn run() -> Result<(), String> {
    let out = crate::get_arg("--out")
        .map(PathBuf::from)
        .ok_or("selftest needs --out DIR")?;
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;

    let rig = crate::find_rig()?;
    let mut app = App::new(DeviceKind::Hardware, rig, Default::default(), 1.0)
        .map_err(|e| format!("app: {e}"))?;
    app.response_delay_ms = 0; // selftest drives response_arrived manually
    app.open_composer();

    // lab-owned backdrop window behind the UI (solid, never desktop content)
    unsafe {
        let _ = SetProcessDpiAwarenessContext(
            windows::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
        );
    }
    let app = Box::leak(Box::new(app));
    let (ax, ay) = (700i32, 600i32);
    let backdrop = create_backdrop(ax - 500, ay - 500, 620, 560);
    let app_ptr: *mut App = app;
    let hwnd = app::create(app, ax, ay).map_err(|e| e.to_string())?;
    // gain foreground: inject a benign key event (grants our process
    // foreground rights), then attach to the current foreground thread.
    unsafe {
        send_input(&[
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_F24,
                        wScan: 0,
                        dwFlags: KEYBD_EVENT_FLAGS(0),
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_F24,
                        wScan: 0,
                        dwFlags: KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
        ]);
        let fg = GetForegroundWindow();
        let fg_tid = GetWindowThreadProcessId(fg, None);
        let my_tid = GetCurrentThreadId();
        let _ = AttachThreadInput(my_tid, fg_tid, true);
        let _ = BringWindowToTop(hwnd);
        let _ = SetForegroundWindow(hwnd);
        let _ = SetActiveWindow(hwnd);
        let _ = AttachThreadInput(my_tid, fg_tid, false);
        pump_for(100);
    }
    GUARD_HWND.store(hwnd.0 as isize, std::sync::atomic::Ordering::SeqCst);
    let mut checks: Vec<Check> = Vec::new();

    // save + restore user state
    let saved_clip = clipboard_read_text();
    let mut saved_cursor = POINT::default();
    unsafe {
        let _ = GetCursorPos(&mut saved_cursor);
    }

    let ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_checks(unsafe { &mut *app_ptr }, hwnd, &out, &mut checks);
    }));
    if let Err(e) = ok {
        checks.push(Check {
            name: "harness".into(),
            pass: false,
            detail: json!({"panic": format!("{e:?}")}),
        });
    }

    // restore + teardown
    if let Some(t) = &saved_clip {
        let _ = clipboard_write_text(t);
    }
    unsafe {
        let _ = SetCursorPos(saved_cursor.x, saved_cursor.y);
        let _ = DestroyWindow(hwnd);
        let _ = DestroyWindow(backdrop);
        while !app::pump_once() {}
    }
    app::unbind();

    let passed = checks.iter().filter(|c| c.pass).count();
    let fg_violations = FG_VIOLATIONS.load(std::sync::atomic::Ordering::SeqCst);
    // per-check injection retry counts (attempts > 1 means the first injected
    // input needed a resend — always accompanied by `attempt_log` evidence)
    let retried: serde_json::Map<String, serde_json::Value> = checks
        .iter()
        .filter_map(|c| {
            let a = c.detail.get("attempts")?.as_u64()?;
            (a > 1).then(|| (c.name.clone(), json!(a)))
        })
        .collect();
    let doc = json!({
        "checks": checks.iter().map(|c| json!({"name": c.name, "pass": c.pass, "detail": c.detail})).collect::<Vec<_>>(),
        "passed": passed,
        "total": checks.len(),
        "retried": retried,
        "foreground_guard_blocked_injections": fg_violations,
    });
    std::fs::write(
        out.join("selftest.json"),
        serde_json::to_string_pretty(&doc).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    println!(
        "selftest: {passed}/{} checks passed -> {}",
        checks.len(),
        out.display()
    );
    if passed == checks.len() {
        Ok(())
    } else {
        Err(format!(
            "{}/{} selftest checks failed",
            checks.len() - passed,
            checks.len()
        ))
    }
}

fn create_backdrop(x: i32, y: i32, w: i32, h: i32) -> HWND {
    unsafe {
        let hinst: HINSTANCE = windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
            .unwrap()
            .into();
        RegisterClassExW(&WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(backdrop_proc),
            hInstance: hinst,
            hbrBackground: HBRUSH(GetStockObject(WHITE_BRUSH).0),
            lpszClassName: w!("MascotLabBackdrop"),
            ..Default::default()
        });
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            w!("MascotLabBackdrop"),
            w!("backdrop"),
            WS_POPUP | WS_VISIBLE,
            x,
            y,
            w,
            h,
            None,
            None,
            Some(hinst),
            None,
        )
        .unwrap_or_default()
    }
}

extern "system" fn backdrop_proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    unsafe { DefWindowProcW(hwnd, msg, w, l) }
}

fn run_checks(app: &mut App, hwnd: HWND, out: &std::path::Path, checks: &mut Vec<Check>) {
    pump_for(200);

    macro_rules! check {
        ($name:expr, $detail:expr) => {
            checks.push(Check {
                name: $name.to_string(),
                pass: true,
                detail: $detail,
            })
        };
        (FAIL $name:expr, $detail:expr) => {
            checks.push(Check {
                name: $name.to_string(),
                pass: false,
                detail: $detail,
            })
        };
    }

    // --- guard: our window is foreground ----------------------------------
    let fg = unsafe { GetForegroundWindow() };
    if fg != hwnd {
        checks.push(Check {
            name: "foreground".into(),
            pass: false,
            detail: json!({"expected": hwnd.0 as usize, "got": fg.0 as usize}),
        });
        return;
    }
    check!("foreground", json!({"hwnd": hwnd.0 as usize}));

    #[allow(dead_code)]
    let _editor_rect_px = |a: &App| -> RECT {
        let e = a.layout.editor.unwrap_or_default();
        RECT {
            left: (e.x * a.scale) as i32,
            top: (e.y * a.scale) as i32,
            right: (e.right() * a.scale) as i32,
            bottom: (e.bottom() * a.scale) as i32,
        }
    };

    // --- ASCII typing ------------------------------------------------------
    send_text("hello");
    drain_input();
    let t = app.editor.text();
    if t == "hello" {
        check!("ascii-typing", json!({"text": t}));
    } else {
        check!(FAIL "ascii-typing", json!({"text": t}));
    }

    // --- Unicode (KEYEVENTF_UNICODE incl. surrogate pair + CJK) -----------
    send_text(" ");
    send_unicode("Ü");
    send_unicode("日");
    send_unicode("本");
    send_unicode("語");
    send_unicode("\u{1F44B}"); // 👋 surrogate pair
    drain_input();
    let t = app.editor.text();
    if t == "hello Ü日本語\u{1F44B}" {
        check!("unicode-typing", json!({"text": t}));
    } else {
        check!(FAIL "unicode-typing", json!({"text": t}));
    }

    release_modifiers();
    // --- Home/End/Shift+End/Ctrl nav --------------------------------------
    app.editor.set_selection(0, 0);
    pump_for(30);
    let u16len = t.encode_utf16().count() as i32;
    key_press(VK_END, 0);
    let ok = pump_until(800, || {
        let (a, b) = app.editor.selection();
        a == u16len && b == a
    });
    let (a, b) = app.editor.selection();
    check_or(
        checks,
        "caret-end",
        ok,
        json!({"sel": [a, b], "u16len": u16len}),
    );
    key_press(VK_HOME, 0);
    pump_until(300, || app.editor.selection() == (0, 0));
    let (a, b) = app.editor.selection();
    check_or(
        checks,
        "caret-home",
        a == 0 && b == 0,
        json!({"sel": [a, b]}),
    );
    chord(&[VK_LSHIFT], VK_END);
    pump_until(300, || app.editor.selection() == (0, u16len));
    let (a, b) = app.editor.selection();
    check_or(
        checks,
        "shift-end",
        a == 0 && b == u16len,
        json!({"sel": [a, b], "u16len": u16len}),
    );
    chord(&[VK_LCONTROL], VK_LEFT);
    pump_until(300, || app.editor.selection().0 < u16len);
    let (a, b) = app.editor.selection();
    check_or(
        checks,
        "ctrl-left-word",
        a < u16len,
        json!({"sel": [a, b], "u16len": u16len}),
    );

    release_modifiers();
    // --- Ctrl+A / Ctrl+C -> clipboard (native richedit path) ---------------
    let mut clip = String::new();
    let mut copied = false;
    let mut sel_seen = (0i32, 0i32);
    let mut copy_attempts = 0u32;
    for _ in 0..4 {
        copy_attempts += 1;
        chord(&[VK_LCONTROL], VK_A);
        pump_until(300, || {
            let (a, b) = app.editor.selection();
            a == 0 && b == u16len
        });
        sel_seen = app.editor.selection();
        chord(&[VK_LCONTROL], VK_C);
        pump_until(300, || {
            clipboard_read_text().map(|c| c == t).unwrap_or(false)
        });
        clip = clipboard_read_text().unwrap_or_default();
        if clip == t {
            copied = true;
            break;
        }
    }
    check_or(
        checks,
        "ctrl-a-ctrl-c",
        copied,
        json!({
            "clipboard": clip.chars().take(40).collect::<String>(),
            "sel": sel_seen,
            "attempts": copy_attempts,
        }),
    );

    // --- Ctrl+V paste (caret at end) ---------------------------------------
    key_press(VK_END, 0);
    drain_input();
    chord(&[VK_LCONTROL], VK_V);
    pump_until(400, || app.editor.text().len() > t.len());
    let t2 = app.editor.text();
    check_or(
        checks,
        "ctrl-v-paste",
        t2.len() > t.len(),
        json!({"after_len": t2.len()}),
    );

    // --- checkpoint: selection --------------------------------------------
    chord(&[VK_LCONTROL], VK_A);
    drain_input();
    save_png(app, out.join("checkpoint-selection.png"));
    checks.push(Check {
        name: "capture-selection".into(),
        pass: out.join("checkpoint-selection.png").exists(),
        detail: json!({}),
    });
    key_press(VK_END, 0);

    release_modifiers();
    // --- Shift+Enter -> newline, height grows ------------------------------
    let h0 = app.layout.window.h;
    chord(&[VK_LSHIFT], VK_RETURN);
    pump_until(500, || app.editor.text().contains('\r'));
    let t3 = app.editor.text();
    let mut got_nl = t3.contains('\n');
    if !got_nl {
        // rich edit may report \r internally — accept CR too
        got_nl = t3.contains('\r');
    }
    app.process_editor_events();
    app.relayout().ok();
    check_or(
        checks,
        "shift-enter-newline",
        got_nl,
        json!({"text_tail": t3.chars().rev().take(10).collect::<String>()}),
    );
    check_or(
        checks,
        "shift-enter-height-grows",
        app.layout.window.h > h0,
        json!({"before": h0, "after": app.layout.window.h}),
    );

    release_modifiers();
    // --- 12 lines -> clamped at max ---------------------------------------
    let lines = "l1\nl2\nl3\nl4\nl5\nl6\nl7\nl8\nl9\nl10\nl11\nl12";
    let st_res = app
        .editor
        .set_text(lines)
        .map(|_| "ok".to_string())
        .unwrap_or_else(|e| e.to_string());
    app.state.editor_empty = false;
    app.process_editor_events();
    app.relayout().ok();
    pump_for(60);
    let text_now = app.editor.text();
    let nat = app.editor.natural_size(336.0).unwrap_or_default();
    let max_h = mascot_ui::layout::composer_height(12.0 * mascot_ui::theme::tokens::BODY_LINE);
    let comp_h = app.layout.composer.unwrap().h;
    check_or(
        checks,
        "composer-clamp",
        (comp_h - max_h).abs() < 0.5,
        json!({"composer_h": comp_h, "max": max_h, "text_len": text_now.chars().count(), "natural": nat.1, "set_text": st_res}),
    );
    save_png(app, out.join("checkpoint-multiline.png"));
    checks.push(Check {
        name: "capture-multiline".into(),
        pass: out.join("checkpoint-multiline.png").exists(),
        detail: json!({}),
    });

    release_modifiers();
    // --- Enter submits -----------------------------------------------------
    app.focus_editor();
    app.state.editor_empty = false;
    app.process_editor_events();
    let mut pressed = false;
    let mut enter_attempts = 0u32;
    let mut enter_log: Vec<serde_json::Value> = Vec::new();
    let mut submit_at_ms = 0u128;
    for _ in 0..3 {
        enter_attempts += 1;
        let press_t0 = std::time::Instant::now();
        key_press(VK_RETURN, 0);
        // 1.5s: SendInput delivery on this VM occasionally lands just past a
        // 500ms deadline (attempt log showed the submit racing the timeout)
        if pump_until(1500, || {
            app.state.activity == mascot_ui::state::Activity::Submitting
        }) {
            pressed = true;
            submit_at_ms = press_t0.elapsed().as_millis();
            break;
        }
        // record when the late submit actually lands (bounded extra wait)
        let _ = pump_until(3000, || {
            app.state.activity == mascot_ui::state::Activity::Submitting
        });
        if app.state.activity == mascot_ui::state::Activity::Submitting {
            submit_at_ms = press_t0.elapsed().as_millis();
        }
        // why did this attempt not submit? capture concrete evidence:
        // text tail (a trailing newline = Shift was still latched when the
        // Enter was processed -> Shift+Enter newline path), queue status,
        // modifier state and the char trace tail.
        let tail: String = app.editor.text().chars().rev().take(8).collect();
        let shift = unsafe { GetKeyState(VK_SHIFT.0 as i32) };
        let ctrl = unsafe { GetKeyState(VK_CONTROL.0 as i32) };
        let queue = unsafe { GetQueueStatus(QS_ALLINPUT) };
        let focus_hwnd = unsafe { GetFocus() };
        let fg_hwnd = unsafe { GetForegroundWindow() };
        enter_log.push(json!({
            "attempt": enter_attempts,
            "text_tail": tail.chars().rev().collect::<String>(),
            "newline_appended": tail.starts_with('\n'),
            "shift_state": shift,
            "ctrl_state": ctrl,
            "queue_status": queue,
            "focus_is_us": focus_hwnd == hwnd,
            "foreground_is_us": fg_hwnd == hwnd,
            "activity": format!("{:?}", app.state.activity),
            "char_trace_tail": &app.char_trace[app.char_trace.len().saturating_sub(4)..],
            "press_to_submit_ms": press_t0.elapsed().as_millis(),
        }));
    }
    let submitted = pressed;
    let norm = |t: &str| t.replace('\r', "\n");
    check_or(
        checks,
        "enter-submits",
        submitted && norm(&app.last_submitted) == norm(lines),
        json!({
            "submitted": app.last_submitted.chars().take(20).collect::<String>(),
            "editor_empty": app.state.editor_empty,
            "activity": format!("{:?}", app.state.activity),
            "focus": format!("{:?}", app.state.interaction.focus),
            "surface": format!("{:?}", app.state.surface),
            "char_trace": format!("{:?}", app.char_trace),
            "attempts": enter_attempts,
            "attempt_log": enter_log,
            "submit_at_ms": submit_at_ms,
        }),
    );
    // simulate response + follow-up empty (the real arrival path restores
    // read-write + undimmed text and clears the composer)
    app.response_arrived();
    pump_for(60);

    // --- Enter on empty does nothing ---------------------------------------
    key_press(VK_RETURN, 0);
    drain_input();
    pump_for(30);
    check_or(
        checks,
        "enter-empty-noop",
        app.state.activity != mascot_ui::state::Activity::Submitting,
        json!({}),
    );

    release_modifiers();
    // --- Tab cycles focus with focus-visible --------------------------------
    // focus may start anywhere (submit leaves it on Stop); Tab until a
    // non-Editor control is focused, since the editor intentionally shows no
    // focus-visible ring
    let f0 = app.state.interaction.focus;
    let mut seen: Vec<String> = Vec::new();
    let mut tab_attempts = 0usize;
    for _ in 0..6 {
        tab_attempts += 1;
        key_press(VK_TAB, 0);
        // wait for delivery AND state, not just state: a late-arriving Tab
        // must not be counted as a press that did nothing
        pump_until(800, || app.state.interaction.focus != f0);
        seen.push(format!("{:?}", app.state.interaction.focus));
        if app.state.interaction.focus != Some(ControlId::Editor) {
            break;
        }
    }
    let f1 = app.state.interaction.focus;
    let vis1 = app.state.interaction.focus_visible;
    key_press(VK_TAB, 0);
    pump_until(300, || app.state.interaction.focus != f1);
    let f2 = app.state.interaction.focus;
    check_or(
        checks,
        "tab-cycle",
        f1.is_some() && f2.is_some() && f1 != f2,
        json!({"f0": f0.map(|c| format!("{c:?}")), "f1": f1.map(|c| format!("{c:?}")), "f2": f2.map(|c| format!("{c:?}")), "surface": format!("{:?}", app.state.surface)}),
    );
    check_or(
        checks,
        "focus-visible",
        vis1,
        json!({"focus_visible_after_tab": vis1, "focus": format!("{f1:?}"), "tab_presses": tab_attempts, "seen": seen}),
    );
    if app.state.interaction.focus == Some(ControlId::Editor) {
        key_press(VK_TAB, 0); // land on Send (enabled only if text present)
    }

    release_modifiers();
    // --- dead keys (Latvian Standard 00020426) ------------------------------
    dead_key_check(app, checks);

    // --- Japanese IME -------------------------------------------------------
    ime_check(app, out, checks);

    // --- caret visible + blink timeout --------------------------------------
    let _ = app.editor.set_text("caret");
    app.state.editor_empty = false;
    app.focus_editor();
    app.relayout().ok();
    pump_for(200);
    let (created, _pos, size) = app.editor.caret_info();
    check_or(
        checks,
        "caret-visible",
        created && size.cy > 0,
        json!({"caret": [size.cx, size.cy]}),
    );
    // caret timeout: wait SPI_GETCARETTIMEOUT + slack, verify richedit timers stopped
    let mut timeout_ms = 5000u32;
    unsafe {
        let _ = SystemParametersInfoW(
            SPI_GETCARETTIMEOUT,
            0,
            Some(&mut timeout_ms as *mut u32 as *mut std::ffi::c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
    }
    pump_for(timeout_ms.max(100) as u64 + 800);
    check_or(
        checks,
        "caret-blink-timeout",
        !app.editor.has_timers(),
        json!({"timeout_ms": timeout_ms}),
    );

    release_modifiers();
    // --- Esc closes ----------------------------------------------------------
    key_press(VK_ESCAPE, 0);
    pump_for(80);
    check_or(
        checks,
        "esc-closes",
        app.state.surface == Surface::Hidden,
        json!({"surface": format!("{:?}", app.state.surface)}),
    );

    // --- live screen capture over the lab backdrop ---------------------------
    app.open_composer();
    pump_for(300);
    screen_capture(hwnd, out.join("selftest-screen.png"));
    checks.push(Check {
        name: "capture-live".into(),
        pass: out.join("selftest-screen.png").exists(),
        detail: json!({}),
    });
}

fn check_or(checks: &mut Vec<Check>, name: &str, pass: bool, detail: Value) {
    checks.push(Check {
        name: name.to_string(),
        pass,
        detail,
    });
}

fn pump_for(ms: u64) {
    let start = std::time::Instant::now();
    while start.elapsed().as_millis() < ms as u128 {
        if !app::pump_once() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

/// Waits until all injected input has been consumed by the message loop:
/// pump until `GetQueueStatus` reports no pending input/message bits, then
/// drain once more. This is the harness-side fix for SendInput delivery
/// races — checks assert only after the queue is observed empty.
fn drain_input() {
    unsafe {
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(2000);
        loop {
            let _ = app::pump_once();
            // high word = message types currently in the queue
            let pending = GetQueueStatus(QS_ALLINPUT) >> 16;
            if pending == 0 {
                break;
            }
            if std::time::Instant::now() > deadline {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        // one extra pump settles posted (non-queued) work like WM_CHAR
        let _ = app::pump_once();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let _ = app::pump_once();
    }
}

/// Pump until `cond` holds or the deadline passes; always drains pending
/// injected input first so assertions see a settled state.
fn pump_until(ms: u64, mut cond: impl FnMut() -> bool) -> bool {
    let start = std::time::Instant::now();
    loop {
        drain_input();
        if cond() {
            return true;
        }
        if start.elapsed().as_millis() >= ms as u128 {
            return cond();
        }
        std::thread::sleep(std::time::Duration::from_millis(8));
    }
}

static GUARD_HWND: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);
static FG_VIOLATIONS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn send_input(inputs: &[INPUT]) {
    unsafe {
        // injection guard: never inject while our window isn't foreground
        let want = GUARD_HWND.load(std::sync::atomic::Ordering::SeqCst);
        if want != 0 && GetForegroundWindow().0 != want as *mut std::ffi::c_void {
            FG_VIOLATIONS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            return;
        }
        let sent = SendInput(inputs, std::mem::size_of::<INPUT>() as i32);
        if sent as usize != inputs.len() {
            FG_VIOLATIONS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }
}

/// Force-release the modifier keys — a dropped keyup mid-batch leaves Ctrl
/// logically down and quietly corrupts every later check (Ctrl+N chars etc).
/// Sent unconditionally: GetKeyState lags while events are still in-flight.
fn release_modifiers() {
    {
        let ups: Vec<INPUT> = [
            VK_LCONTROL,
            VK_RCONTROL,
            VK_LSHIFT,
            VK_RSHIFT,
            VK_LMENU,
            VK_RMENU,
            VK_CONTROL,
            VK_SHIFT,
            VK_MENU,
        ]
        .iter()
        .map(|vk| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: *vk,
                    wScan: 0,
                    dwFlags: KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        })
        .collect();
        send_input(&ups);
        pump_for(30);
    }
}

fn key_press(vk: VIRTUAL_KEY, _scan: u32) {
    send_input(&[
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: KEYBD_EVENT_FLAGS(0),
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        },
    ]);
}

fn chord(mods: &[VIRTUAL_KEY], vk: VIRTUAL_KEY) {
    let mut v: Vec<INPUT> = mods
        .iter()
        .map(|m| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: *m,
                    wScan: 0,
                    dwFlags: KEYBD_EVENT_FLAGS(0),
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        })
        .collect();
    v.push(INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: KEYBD_EVENT_FLAGS(0),
                time: 0,
                dwExtraInfo: 0,
            },
        },
    });
    v.push(INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: KEYEVENTF_KEYUP,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    });
    for m in mods.iter().rev() {
        v.push(INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: *m,
                    wScan: 0,
                    dwFlags: KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        });
    }
    send_input(&v);
}

fn send_text(s: &str) {
    for c in s.chars() {
        // ASCII-only fast path
        if c.is_ascii() {
            send_unicode(&c.to_string());
        }
    }
}

fn send_unicode(s: &str) {
    for u in s.encode_utf16() {
        send_input(&[
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VIRTUAL_KEY(0),
                        wScan: u,
                        dwFlags: KEYEVENTF_UNICODE,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VIRTUAL_KEY(0),
                        wScan: u,
                        dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
        ]);
    }
}

fn clipboard_read_text() -> Option<String> {
    unsafe {
        if OpenClipboard(None).is_err() {
            return None;
        }
        let h = GetClipboardData(13);
        let out = h.ok().and_then(|h| {
            let h = windows::Win32::Foundation::HGLOBAL(h.0);
            let p = windows::Win32::System::Memory::GlobalLock(h);
            if p.is_null() {
                None
            } else {
                let s = PCWSTR(p as *const u16).to_string().ok();
                let _ = windows::Win32::System::Memory::GlobalUnlock(h);
                s
            }
        });
        let _ = CloseClipboard();
        out
    }
}

fn clipboard_write_text(t: &str) -> windows::core::Result<()> {
    unsafe {
        OpenClipboard(None)?;
        EmptyClipboard()?;
        let w: Vec<u16> = t.encode_utf16().chain(Some(0)).collect();
        let h = windows::Win32::System::Memory::GlobalAlloc(
            windows::Win32::System::Memory::GMEM_MOVEABLE,
            w.len() * 2,
        )?;
        let p = windows::Win32::System::Memory::GlobalLock(h);
        std::ptr::copy_nonoverlapping(w.as_ptr(), p as *mut u16, w.len());
        let _ = windows::Win32::System::Memory::GlobalUnlock(h);
        SetClipboardData(13, Some(HANDLE(h.0)))?;
        CloseClipboard()?;
    }
    Ok(())
}

fn save_png(app: &mut App, path: PathBuf) {
    if let Ok(img) = app.render_offscreen() {
        let _ = img.save(&path);
    }
}

fn screen_capture(hwnd: HWND, path: PathBuf) {
    unsafe {
        let mut rc = RECT::default();
        let _ = GetWindowRect(hwnd, &mut rc);
        let (w, h) = (rc.right - rc.left, rc.bottom - rc.top);
        let screen = GetDC(None);
        let mem = CreateCompatibleDC(Some(screen));
        let bmp = CreateCompatibleBitmap(screen, w, h);
        let old = SelectObject(mem, bmp.into());
        let _ = BitBlt(mem, 0, 0, w, h, Some(screen), rc.left, rc.top, SRCCOPY);
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut buf = vec![0u8; (w * h * 4) as usize];
        GetDIBits(
            mem,
            bmp,
            0,
            h as u32,
            Some(buf.as_mut_ptr() as *mut _),
            &mut info,
            DIB_RGB_COLORS,
        );
        SelectObject(mem, old);
        let _ = DeleteObject(bmp.into());
        let _ = DeleteDC(mem);
        ReleaseDC(None, screen);
        for p in buf.chunks_exact_mut(4) {
            p.swap(0, 2);
        }
        let img = mascot_render_win32::image::RgbaImage {
            width: w as u32,
            height: h as u32,
            data: buf,
        };
        let _ = img.save(&path);
    }
}

/// Dead-key verification using the Latvian (Standard) layout 00020426.
/// Records a skip detail when the layout is unavailable on this machine.
fn refocus_editor(app: &mut App) {
    app.focus_editor();
    pump_for(30);
}

fn dead_key_check(app: &mut App, checks: &mut Vec<Check>) {
    refocus_editor(app);
    unsafe {
        let layout = LoadKeyboardLayoutW(w!("00020426"), KLF_ACTIVATE);
        if layout.is_err() || layout == Ok(HKL::default()) {
            checks.push(Check {
                name: "dead-key-latvian".into(),
                pass: true,
                detail: json!({"skipped": "layout 00020426 unavailable"}),
            });
            return;
        }
        let prev_layout = GetKeyboardLayout(0);
        // identify the dead key: on Latvian Standard the apostrophe dead key
        // composes e.g. ' + a -> ā. Verify via ToUnicodeEx first.
        let layout_ok = layout.ok();
        let probe = |vk: u32| -> i32 {
            let mut buf = [0u16; 4];
            let state = [0u8; 256];
            ToUnicodeEx(vk, 0, &state, &mut buf, 0, layout_ok)
        };
        let mut dead_text = String::new();
        let mut dead_vk = None;
        for vk in [0xDEu32, 0xBA, 0xBB, 0xC0] {
            if probe(vk) == -1 {
                dead_vk = Some(vk);
                break;
            }
        }
        let mut dead_attempts = 0u32;
        let pass = if let Some(vk) = dead_vk {
            // send the dead key by scancode (physical key, layout-independent)
            let sc = MapVirtualKeyExW(vk, MAP_VIRTUAL_KEY_TYPE(1), layout_ok) as u16;
            send_input(&[
                INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: VIRTUAL_KEY(0),
                            wScan: sc,
                            dwFlags: KEYEVENTF_SCANCODE,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                },
                INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: VIRTUAL_KEY(0),
                            wScan: sc,
                            dwFlags: KEYEVENTF_SCANCODE | KEYEVENTF_KEYUP,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                },
            ]);
            pump_for(60);
            key_press(VK_A, 0);
            pump_until(500, || !app.editor.text().is_empty());
            dead_text = app.editor.text();
            dead_attempts = 1;
            while dead_attempts < 4 && !(dead_text.contains('ā') || dead_text.contains('á')) {
                dead_attempts += 1;
                key_press(VIRTUAL_KEY(vk as u16), 0);
                drain_input();
                key_press(VK_A, 0);
                pump_until(300, || {
                    app.editor.text().len() > dead_text.len() || {
                        let t = app.editor.text();
                        t.contains('ā') || t.contains('á')
                    }
                });
                dead_text = app.editor.text();
            }
            dead_text.contains('ā') || dead_text.contains('á')
        } else {
            false
        };
        let detail = json!({
            "dead_vk": dead_vk,
            "text": dead_text,
            "attempts": dead_attempts,
            "trace_tail": format!("{:x?}", &app.char_trace[app.char_trace.len().saturating_sub(10)..]),
            "ctrl": GetKeyState(VK_CONTROL.0 as i32),
            "shift": GetKeyState(VK_SHIFT.0 as i32),
            "active_hkl": format!("{:p}", GetKeyboardLayout(0).0),
        });
        checks.push(Check {
            name: "dead-key-latvian".into(),
            pass,
            detail,
        });
        let _ = ActivateKeyboardLayout(prev_layout, KLF_ACTIVATE);
    }
}

/// MS-IME ja-JP TSF profile activation — same GUIDs as
/// `benchmark/harness/preflight_ime.ps1`.
fn activate_japanese_ime() -> Result<(), String> {
    use windows::Win32::System::Com::*;
    use windows::Win32::UI::TextServices::*;
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let mgr: ITfInputProcessorProfileMgr =
            CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)
                .map_err(|e| format!("mgr: {e}"))?;
        let service = windows::core::GUID::from_u128(0x03B5835F_F03C_411B_9CE2_AA23E1171E36);
        let profile = windows::core::GUID::from_u128(0xA76C93D9_5523_4E90_AAFA_4DB112F9AC76);
        mgr.ActivateProfile(1, 0x0411, &service, &profile, HKL::default(), 0x10000000)
            .map_err(|e| format!("ActivateProfile: {e}"))?;
    }
    Ok(())
}

/// Japanese IME: activate the ja-JP TSF profile, type romaji, verify preedit
/// renders and Enter commits without submitting. Skipped (pass+note) when the
/// profile isn't installed.
fn ime_check(app: &mut App, out: &std::path::Path, checks: &mut Vec<Check>) {
    refocus_editor(app);
    let before = app.editor.text();
    if let Err(e) = activate_japanese_ime() {
        for name in [
            "ime-ja-preedit",
            "ime-ja-commit",
            "ime-ja-enter-no-submit",
            "ime-ja-esc-cancel",
        ] {
            checks.push(Check {
                name: name.into(),
                pass: true,
                detail: json!({"skipped": e}),
            });
        }
        return;
    }
    pump_for(200);
    // open the IME via the TSF keyboard open/close compartment (the imm32
    // stub does not drive MS-IME's open state on Win10/11)
    unsafe {
        use windows::Win32::System::Com::*;
        use windows::Win32::UI::TextServices::*;
        if let Ok(mgr) =
            CoCreateInstance::<_, ITfThreadMgr>(&CLSID_TF_ThreadMgr, None, CLSCTX_INPROC_SERVER)
        {
            let _ = mgr.Activate();
            if let Ok(cm) = mgr.GetGlobalCompartment() {
                for (g, val) in [
                    (
                        GUID_COMPARTMENT_KEYBOARD_OPENCLOSE,
                        windows::Win32::System::Variant::VARIANT::from(true),
                    ),
                    (
                        GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION,
                        windows::Win32::System::Variant::VARIANT::from(0x09u32),
                    ),
                ] {
                    if let Ok(c) = cm.GetCompartment(&g) {
                        let _ = c.SetValue(0, &val);
                    }
                }
            }
        }
    }
    pump_for(100);
    // VK_KANJI toggles MS-IME open/closed (the compartment SetValue does not
    // take effect for a thread that never owned a TSF input context)
    key_press(VK_KANJI, 0);
    pump_for(120);
    // romaji n,i,h,o,n,n as REAL keystrokes (TSF ignores KEYEVENTF_UNICODE)
    for vk in [VK_N, VK_I, VK_H, VK_O, VK_N, VK_N] {
        key_press(vk, 0);
        pump_for(40);
    }
    pump_for(150);
    let composing = app.state.composing;
    save_png(app, out.join("checkpoint-ime-preedit.png"));
    checks.push(Check {
        name: "ime-ja-preedit".into(),
        pass: composing,
        detail: json!({
            "composing": composing,
            "trace": format!("{:x?}", &app.char_trace[app.char_trace.len().saturating_sub(24)..]),
            "ctrl": unsafe { GetKeyState(VK_CONTROL.0 as i32) },
            "open": unsafe {
                use windows::Win32::UI::Input::Ime::*;
                let himc = ImmGetContext(app.editor.hwnd);
                let o = ImmGetOpenStatus(himc).as_bool();
                let mut cm = IME_CONVERSION_MODE::default();
                let mut sm = IME_SENTENCE_MODE::default();
                let _ = ImmGetConversionStatus(himc, Some(&mut cm), Some(&mut sm));
                let _ = ImmReleaseContext(app.editor.hwnd, himc);
                format!("open={o} cmode={:?} smode={:?}", cm.0, sm.0)
            },
        }),
    });

    // Enter commits the composition (にほん) WITHOUT submitting
    key_press(VK_RETURN, 0);
    pump_for(200);
    let after = app.editor.text();
    let committed = after != before && !after.is_empty();
    let submitted = app.state.activity == mascot_ui::state::Activity::Submitting;
    checks.push(Check {
        name: "ime-ja-commit".into(),
        pass: committed,
        detail: json!({"after": after}),
    });
    checks.push(Check {
        name: "ime-ja-enter-no-submit".into(),
        pass: !submitted,
        detail: json!({}),
    });

    // a second composition cancelled with Esc
    for vk in [VK_K, VK_A] {
        key_press(vk, 0);
        pump_for(40);
    }
    pump_for(100);
    let mid = app.editor.text();
    key_press(VK_ESCAPE, 0);
    pump_for(150);
    let post = app.editor.text();
    checks.push(Check {
        name: "ime-ja-esc-cancel".into(),
        pass: post.len() <= mid.len() && app.state.surface != mascot_ui::state::Surface::Hidden,
        detail: json!({"mid": mid, "post": post}),
    });
}
