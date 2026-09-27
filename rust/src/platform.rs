use crate::config::Config;
use crate::provider::{Command, Event, Provider, qpc};
use crate::queue::BoundedQueue;
use crate::text;
use serde_json::{Value, json};
use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::sync::{Arc, Mutex};
use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::UI::HiDpi::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

pub const WM_APP_PROVIDER: u32 = WM_APP + 1;
pub const WM_APP_CONTROL: u32 = WM_APP + 2;
pub const WM_APP_SUBMIT: u32 = WM_APP + 3;

const SS_LEFT: u32 = 0;
const ID_SEND: usize = 101;
const ID_CANCEL: usize = 102;
const ID_STATUS: usize = 103;
const ID_MENU_EXIT: usize = 4001;
pub const HOTKEY_TOGGLE_ID: i32 = 1;
pub const HOTKEY_CANCEL_ID: i32 = 2;

fn dip(value: i32, dpi: u32) -> i32 {
    (value * dpi as i32 + 48) / 96
}

pub struct Composer {
    pub hwnd: HWND,
    pub input: HWND,
    pub response: HWND,
    pub status: HWND,
    pub send: HWND,
    pub cancel: HWND,
    pub font: HFONT,
    pub module: HMODULE,
    pub dpi: u32,
}

pub struct Model {
    pub provider_state: String,
    pub request_id: u64,
    pub request_count: u64,
    pub last_seq: i64,
    pub response: String,
    pub provider_pid: u32,
    pub scenario: String,
    pub run_invalid: Option<String>,
}

pub struct Ui {
    pub config: Config,
    pub thread_id: Cell<u32>,
    pub mascot: Cell<HWND>,
    pub mascot_source: Vec<u8>,
    pub mascot_bits: RefCell<Vec<u8>>,
    pub composer: RefCell<Option<Composer>>,
    pub model: RefCell<Model>,
    pub composing: Cell<bool>,
    pub snapshot: RefCell<Option<TextSnapshotLocal>>,
    pub ui_events: Arc<BoundedQueue<Event>>,
    pub commands: Arc<BoundedQueue<Value>>,
    pub records: Arc<BoundedQueue<String>>,
    pub provider: Mutex<Option<Arc<Provider>>>,
    pub shutdown_started: Cell<bool>,
    pub presents: Cell<u64>,
    pub paints: Cell<u64>,
}

#[derive(Clone)]
pub struct TextSnapshotLocal {
    pub text: String,
    pub sel_start: i32,
    pub sel_end: i32,
}

impl Ui {
    pub fn composing(&self) -> bool {
        self.composing.get()
    }

    pub fn on_ime_start(&self, input: HWND) {
        self.composing.set(true);
        let range = text::get_selection(input);
        *self.snapshot.borrow_mut() = Some(TextSnapshotLocal {
            text: text::get_text(input),
            sel_start: range.min,
            sel_end: range.max,
        });
    }

    pub fn on_ime_end(&self, _input: HWND) {
        self.composing.set(false);
    }

    pub fn on_ime_update(&self) {
        self.composing.set(true);
    }

    pub fn post_submit(&self) {
        unsafe {
            PostThreadMessageW(self.thread_id.get(), WM_APP_SUBMIT, 0, 0);
        }
    }

    pub fn record(&self, value: Value) {
        let line = value.to_string();
        if self.records.try_push(line).is_err() {
            let mut model = self.model.borrow_mut();
            model.run_invalid = Some("control output queue overflow".into());
            eprintln!("control output queue overflow; run invalid");
        }
    }

    pub fn present_mascot(&self, dpi: u32) {
        let pixels = dip(self.config.manifest.asset.logical_width_dip as i32, dpi) as usize;
        let mut scaled = vec![0u8; pixels * pixels * 4];
        for y in 0..pixels {
            for x in 0..pixels {
                let sx = x * 128 / pixels;
                let sy = y * 128 / pixels;
                let src = (sy * 128 + sx) * 4;
                let dst = (y * pixels + x) * 4;
                scaled[dst..dst + 4].copy_from_slice(&self.mascot_source[src..src + 4]);
            }
        }
        *self.mascot_bits.borrow_mut() = scaled;
        unsafe {
            let screen = GetDC(std::ptr::null_mut());
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: pixels as i32,
                    biHeight: -(pixels as i32),
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [RGBQUAD {
                    rgbBlue: 0,
                    rgbGreen: 0,
                    rgbRed: 0,
                    rgbReserved: 0,
                }],
            };
            let mut bits: *mut c_void = std::ptr::null_mut();
            let bitmap = CreateDIBSection(
                screen,
                &info,
                DIB_RGB_COLORS,
                &mut bits,
                std::ptr::null_mut(),
                0,
            );
            if bitmap.is_null() {
                ReleaseDC(std::ptr::null_mut(), screen);
                return;
            }
            std::ptr::copy_nonoverlapping(
                self.mascot_bits.borrow().as_ptr(),
                bits as *mut u8,
                pixels * pixels * 4,
            );
            let memory = CreateCompatibleDC(screen);
            let old = SelectObject(memory, bitmap);
            let window = self.mascot.get();
            let mut origin = POINT { x: 0, y: 0 };
            GetWindowRect(window, &mut origin as *mut _ as *mut RECT);
            let mut top_left = POINT {
                x: origin.x,
                y: origin.y,
            };
            let size = SIZE {
                cx: pixels as i32,
                cy: pixels as i32,
            };
            let mut zero = POINT { x: 0, y: 0 };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            if UpdateLayeredWindow(
                window,
                screen,
                &mut top_left,
                &size,
                memory,
                &mut zero,
                0,
                &blend,
                ULW_ALPHA,
            ) != 0
            {
                self.presents.set(self.presents.get() + 1);
            }
            SelectObject(memory, old);
            DeleteDC(memory);
            DeleteObject(bitmap);
            ReleaseDC(std::ptr::null_mut(), screen);
        }
    }

    pub fn ensure_composer(&self) -> Result<HWND, String> {
        if let Some(existing) = self.composer.borrow().as_ref() {
            return Ok(existing.hwnd);
        }
        unsafe {
            let instance = text::instance();
            let mut rect = RECT {
                left: 0,
                top: 0,
                right: dip(
                    self.config.manifest.ui.composer_client_width_dip as i32,
                    GetDpiForWindow(self.mascot.get()),
                ),
                bottom: dip(
                    self.config.manifest.ui.composer_client_height_dip as i32,
                    GetDpiForWindow(self.mascot.get()),
                ),
            };
            let dpi = GetDpiForWindow(self.mascot.get());
            let style = WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_CLIPCHILDREN;
            AdjustWindowRectExForDpi(&mut rect, style, 0, 0, dpi);
            let mut place = POINT { x: 0, y: 0 };
            let mut mascot_rect = RECT::default();
            GetWindowRect(self.mascot.get(), &mut mascot_rect);
            place.x = mascot_rect.right + 8;
            place.y = mascot_rect.top;
            let monitor = MonitorFromPoint(
                POINT {
                    x: place.x,
                    y: place.y,
                },
                MONITOR_DEFAULTTONEAREST,
            );
            let mut monitor_info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..std::mem::zeroed()
            };
            GetMonitorInfoW(monitor, &mut monitor_info);
            let width = rect.right - rect.left;
            let height = rect.bottom - rect.top;
            place.x = place
                .x
                .clamp(monitor_info.rcWork.left, monitor_info.rcWork.right - width);
            place.y = place
                .y
                .clamp(monitor_info.rcWork.top, monitor_info.rcWork.bottom - height);
            let hwnd = CreateWindowExW(
                0,
                text::wide("MascotRustComposer").as_ptr(),
                text::wide("mascot").as_ptr(),
                style,
                place.x,
                place.y,
                width,
                height,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                self as *const Ui as *const c_void,
            );
            if hwnd.is_null() {
                return Err(format!(
                    "composer creation failed: {}",
                    std::io::Error::last_os_error()
                ));
            }
            self.enforce_composer_client(hwnd);
            Ok(hwnd)
        }
    }

    fn enforce_composer_client(&self, hwnd: HWND) {
        unsafe {
            let dpi = GetDpiForWindow(hwnd);
            let mut rect = RECT::default();
            GetClientRect(hwnd, &mut rect);
            let want_w = dip(
                self.config.manifest.ui.composer_client_width_dip as i32,
                dpi,
            );
            let want_h = dip(
                self.config.manifest.ui.composer_client_height_dip as i32,
                dpi,
            );
            if rect.right - rect.left == want_w && rect.bottom - rect.top == want_h {
                return;
            }
            let style = WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_CLIPCHILDREN;
            let mut frame = RECT {
                left: 0,
                top: 0,
                right: want_w,
                bottom: want_h,
            };
            AdjustWindowRectExForDpi(&mut frame, style, 0, 0, dpi);
            let mut origin = RECT::default();
            GetWindowRect(hwnd, &mut origin);
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                origin.left,
                origin.top,
                frame.right - frame.left,
                frame.bottom - frame.top,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }

    pub fn show_composer(&self) -> Result<(), String> {
        let hwnd = self.ensure_composer()?;
        unsafe {
            ShowWindow(hwnd, SW_SHOWNORMAL);
            SetForegroundWindow(hwnd);
            if let Some(composer) = self.composer.borrow().as_ref() {
                SetFocus(composer.input);
            }
        }
        Ok(())
    }

    pub fn hide_composer(&self) {
        let borrowed = self.composer.borrow();
        if let Some(composer) = borrowed.as_ref() {
            if self.composing.get() {
                text::cancel_composition(composer.input);
                if let Some(snapshot) = self.snapshot.borrow().clone() {
                    text::set_text(composer.input, &snapshot.text);
                    text::set_selection(composer.input, snapshot.sel_start, snapshot.sel_end);
                }
                self.composing.set(false);
            }
            unsafe {
                ShowWindow(composer.hwnd, SW_HIDE);
            }
        }
    }

    pub fn submit(&self) -> Result<(), String> {
        {
            let model = self.model.borrow();
            if model.provider_state == "streaming" {
                return Err("a request is already active".into());
            }
        }
        if self.composing.get() {
            return Err("submit disabled while composing".into());
        }
        let input = {
            let borrowed = self.composer.borrow();
            borrowed.as_ref().ok_or("composer not created")?.input
        };
        let prompt = text::get_text(input);
        let provider = self
            .provider
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let mut model = self.model.borrow_mut();
        let id = model.request_count + 1;
        let scenario = std::mem::replace(&mut model.scenario, "normal".to_string());
        model.request_count = id;
        model.request_id = id;
        model.last_seq = -1;
        model.response.clear();
        model.provider_state = "streaming".into();
        if let Some(provider) = provider {
            provider
                .send(Command::Request {
                    id,
                    prompt,
                    scenario,
                })
                .map_err(|e| {
                    model.provider_state = "failed".into();
                    e
                })?;
        }
        drop(model);
        self.refresh_response();
        self.refresh_status();
        Ok(())
    }

    pub fn cancel_request(&self) {
        let model = self.model.borrow();
        if model.provider_state == "streaming" && model.request_id > 0 {
            let id = model.request_id;
            drop(model);
            let provider = self
                .provider
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone();
            if let Some(provider) = provider {
                let _ = provider.send(Command::Cancel { id });
            }
        }
    }

    fn refresh_response(&self) {
        let model = self.model.borrow();
        let body = model.response.clone();
        drop(model);
        let mut borrowed = self.composer.borrow_mut();
        if let Some(composer) = borrowed.as_mut() {
            let mut content = self.config.history_prefix.clone();
            content.push_str(&body);
            text::set_text(composer.response, &content);
            unsafe {
                SendMessageW(
                    composer.response,
                    text::EM_SETSEL,
                    usize::MAX,
                    usize::MAX as isize,
                );
                SendMessageW(composer.response, text::EM_SCROLLCARET, 0, 0);
            }
        }
    }

    pub fn refresh_status(&self) {
        let text = self.model.borrow().provider_state.clone();
        let borrowed = self.composer.borrow();
        if let Some(composer) = borrowed.as_ref() {
            unsafe {
                SetWindowTextW(composer.status, text::wide(&text).as_ptr());
            }
        }
    }

    pub fn dispatch_provider_events(&self) {
        for _ in 0..64 {
            let event = match self.ui_events.pop() {
                Some(event) => event,
                None => break,
            };
            match event {
                Event::Started { pid, .. } => {
                    self.model.borrow_mut().provider_pid = pid;
                    self.refresh_status();
                }
                Event::Chunk { id, seq, text, .. } => {
                    let (accepted_qpc, mut model) = {
                        let stamp = qpc();
                        (stamp, self.model.borrow_mut())
                    };
                    if model.request_id != id {
                        continue;
                    }
                    let limit = self.config.manifest.ui.response_limit_utf8_bytes as usize;
                    let mut chunk_text = text;
                    if model.response.len() + chunk_text.len() > limit {
                        let remaining = limit.saturating_sub(model.response.len());
                        let mut cut = remaining;
                        while cut > 0 && !chunk_text.is_char_boundary(cut) {
                            cut -= 1;
                        }
                        chunk_text.truncate(cut);
                    }
                    model.response.push_str(&chunk_text);
                    model.last_seq = seq as i64;
                    drop(model);
                    self.record(json!({
                        "event": "chunk_accepted",
                        "request_id": id,
                        "seq": seq,
                        "accepted_qpc": accepted_qpc.to_string(),
                    }));
                    self.refresh_response();
                }
                Event::Terminal {
                    id, kind, last_seq, ..
                } => {
                    let mut model = self.model.borrow_mut();
                    if model.request_id == id || kind == "failed" {
                        model.provider_state = kind.clone();
                        model.last_seq = last_seq;
                    }
                    drop(model);
                    self.record(json!({
                        "event": "terminal",
                        "request_id": id,
                        "kind": kind,
                        "last_seq": last_seq,
                        "qpc": qpc().to_string(),
                    }));
                    self.refresh_status();
                }
                Event::SessionClosed { error, .. } => {
                    let mut model = self.model.borrow_mut();
                    model.provider_pid = 0;
                    if let Some(error) = error.as_ref() {
                        model.provider_state = "failed".into();
                        model.run_invalid = Some(error.clone());
                    }
                    drop(model);
                    self.refresh_status();
                }
            }
        }
    }

    pub fn state_json(&self) -> Value {
        let model = self.model.borrow();
        let borrowed = self.composer.borrow();
        let (composer, input, response, visible) = match borrowed.as_ref() {
            Some(c) => {
                let visible = unsafe { IsWindowVisible(c.hwnd) } != 0;
                (
                    (c.hwnd as usize).to_string(),
                    (c.input as usize).to_string(),
                    (c.response as usize).to_string(),
                    visible,
                )
            }
            None => ("0".into(), "0".into(), "0".into(), false),
        };
        let provider_pid = model.provider_pid;
        drop(model);
        drop(borrowed);
        let stderr_total = self
            .provider
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|p| p.stderr_total())
            .unwrap_or(0);
        let model = self.model.borrow();
        json!({
            "pid": std::process::id(),
            "mascot_hwnd": (self.mascot.get() as usize).to_string(),
            "composer_hwnd": composer,
            "input_hwnd": input,
            "response_hwnd": response,
            "composer_visible": visible,
            "composing": self.composing.get(),
            "provider_pid": provider_pid,
            "provider_state": model.provider_state,
            "request_id": model.request_id,
            "request_count": model.request_count,
            "last_seq": model.last_seq,
            "response_utf8_bytes": model.response.len(),
            "mascot_presents": self.presents.get(),
            "composer_paints": self.paints.get(),
            "queued_provider_frames": self.ui_events.len(),
            "queue_capacity_frames": 64,
            "cache_counts": {
                "stderr_tail_bytes": stderr_total.min(4096),
                "stderr_total_bytes": stderr_total,
                "retained_response_utf8_bytes": model.response.len(),
                "dib_buffers": if self.mascot_bits.borrow().is_empty() { 0 } else { 1 },
                "run_invalid": model.run_invalid,
            }
        })
    }

    pub fn text_snapshot(&self) -> Result<text::TextSnapshot, String> {
        let borrowed = self.composer.borrow();
        let composer = borrowed.as_ref().ok_or("composer not created")?;
        let input = text::get_text(composer.input);
        let full = text::get_text(composer.response);
        let response = full
            .strip_prefix(&self.config.history_prefix)
            .unwrap_or(&full)
            .to_string();
        let range = text::get_selection(composer.input);
        Ok(text::TextSnapshot {
            input,
            response,
            selection_start: range.min,
            selection_end: range.max,
            composing: self.composing.get(),
        })
    }

    pub fn set_input_text(&self, value: &str) -> Result<(), String> {
        if value.contains('\0') {
            return Err("text contains NUL".into());
        }
        if text::utf16_units(value) > self.config.manifest.ui.input_limit_utf16_units as usize {
            return Err("text exceeds input limit".into());
        }
        let borrowed = self.composer.borrow();
        let composer = borrowed.as_ref().ok_or("composer not created")?;
        text::set_text(composer.input, value);
        let range = text::get_selection(composer.input);
        *self.snapshot.borrow_mut() = Some(TextSnapshotLocal {
            text: text::get_text(composer.input),
            sel_start: range.min,
            sel_end: range.max,
        });
        Ok(())
    }

    pub fn dispatch_control(&self) {
        for _ in 0..16 {
            let command = match self.commands.pop() {
                Some(command) => command,
                None => break,
            };
            self.handle_control(command);
        }
    }

    fn reply(&self, token: &Value, state: bool, extra: Option<Value>) {
        let mut value = json!({"token": token, "ok": true});
        if state {
            value["state"] = self.state_json();
        }
        if let Some(extra) = extra {
            value["text"] = extra;
        }
        self.record(value);
    }

    fn reply_error(&self, token: &Value, error: String) {
        self.record(json!({"token": token, "ok": false, "error": error}));
    }

    fn handle_control(&self, command: Value) {
        let token = command.get("token").cloned().unwrap_or(Value::Null);
        let name = command
            .get("command")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        match name.as_str() {
            "state" => self.reply(&token, true, None),
            "text" => match self.text_snapshot() {
                Ok(snapshot) => self.reply(
                    &token,
                    true,
                    Some(json!({
                        "input": snapshot.input,
                        "response": snapshot.response,
                        "selection_start": snapshot.selection_start,
                        "selection_end": snapshot.selection_end,
                        "composing": snapshot.composing,
                    })),
                ),
                Err(error) => self.reply_error(&token, error),
            },
            "set_text" => {
                let value = command
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                match self.set_input_text(&value) {
                    Ok(()) => self.reply(&token, true, None),
                    Err(error) => self.reply_error(&token, error),
                }
            }
            "scenario" => {
                let name = command
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let mut model = self.model.borrow_mut();
                if model.provider_state == "streaming" {
                    self.reply_error(&token, "request active".into());
                } else if self.config.manifest.scenarios.contains_key(&name) {
                    model.scenario = name;
                    drop(model);
                    self.reply(&token, true, None);
                } else {
                    drop(model);
                    self.reply_error(&token, format!("unknown scenario '{name}'"));
                }
            }
            "show" => match self.show_composer() {
                Ok(()) => self.reply(&token, true, None),
                Err(error) => self.reply_error(&token, error),
            },
            "hide" => {
                self.hide_composer();
                self.reply(&token, true, None);
            }
            "submit" => match self.submit() {
                Ok(()) => self.reply(&token, true, None),
                Err(error) => self.reply_error(&token, error),
            },
            "cancel" => {
                self.cancel_request();
                self.reply(&token, true, None);
            }
            "shutdown" => {
                self.reply(&token, true, None);
                self.shutdown();
            }
            _ => self.reply_error(&token, format!("unknown command '{name}'")),
        }
    }

    pub fn shutdown(&self) {
        if self.shutdown_started.replace(true) {
            return;
        }
        let provider = self
            .provider
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        if let Some(provider) = provider {
            let _ = provider.send(Command::Shutdown);
        }
        unsafe {
            if let Some(composer) = self.composer.borrow().as_ref() {
                ShowWindow(composer.hwnd, SW_HIDE);
            }
        }
        self.records.close();
        unsafe {
            PostQuitMessage(0);
        }
    }

    pub fn teardown(&self) {
        let provider = self
            .provider
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        if let Some(provider) = provider {
            provider.stop();
        }
        self.commands.close();
        self.records.close();
        let mut borrowed = self.composer.borrow_mut();
        if let Some(composer) = borrowed.take() {
            unsafe {
                DestroyWindow(composer.input);
                DestroyWindow(composer.response);
                DestroyWindow(composer.status);
                DestroyWindow(composer.send);
                DestroyWindow(composer.cancel);
                DestroyWindow(composer.hwnd);
                DeleteObject(composer.font);
                FreeLibrary(composer.module);
            }
        }
        let mascot = self.mascot.get();
        if !mascot.is_null() {
            unsafe {
                DestroyWindow(mascot);
            }
            self.mascot.set(std::ptr::null_mut());
        }
    }
}

unsafe extern "system" fn mascot_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        if message == WM_NCCREATE {
            let create = lparam as *const CREATESTRUCTW;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*create).lpCreateParams as isize);
            return DefWindowProcW(hwnd, message, wparam, lparam);
        }
        let ui_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const Ui;
        if ui_ptr.is_null() {
            return DefWindowProcW(hwnd, message, wparam, lparam);
        }
        let ui = &*ui_ptr;
        match message {
            WM_NCHITTEST => {
                let x = (lparam as i16) as i32;
                let y = ((lparam >> 16) as i16) as i32;
                let mut rect = RECT::default();
                GetWindowRect(hwnd, &mut rect);
                let width = rect.right - rect.left;
                let height = rect.bottom - rect.top;
                if width <= 0 || height <= 0 {
                    return HTTRANSPARENT as LRESULT;
                }
                let px = x - rect.left;
                let py = y - rect.top;
                if px < 0 || py < 0 || px >= width || py >= height {
                    return HTTRANSPARENT as LRESULT;
                }
                let sx = (px as usize * 128 / width as usize).min(127);
                let sy = (py as usize * 128 / height as usize).min(127);
                let alpha = ui.mascot_source[(sy * 128 + sx) * 4 + 3];
                if alpha > 0 {
                    HTCAPTION as LRESULT
                } else {
                    HTTRANSPARENT as LRESULT
                }
            }
            WM_DPICHANGED => {
                let suggested = lparam as *const RECT;
                SetWindowPos(
                    hwnd,
                    std::ptr::null_mut(),
                    (*suggested).left,
                    (*suggested).top,
                    (*suggested).right - (*suggested).left,
                    (*suggested).bottom - (*suggested).top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
                ui.present_mascot(GetDpiForWindow(hwnd));
                0
            }
            WM_RBUTTONUP => {
                let menu = CreatePopupMenu();
                if !menu.is_null() {
                    AppendMenuW(menu, MF_STRING, ID_MENU_EXIT, text::wide("Exit").as_ptr());
                    let mut point = POINT { x: 0, y: 0 };
                    GetCursorPos(&mut point);
                    SetForegroundWindow(hwnd);
                    let choice = TrackPopupMenu(
                        menu,
                        TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY,
                        point.x,
                        point.y,
                        0,
                        hwnd,
                        std::ptr::null(),
                    );
                    DestroyMenu(menu);
                    if choice as usize == ID_MENU_EXIT {
                        ui.shutdown();
                    }
                }
                0
            }
            WM_CLOSE => {
                ui.shutdown();
                0
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}

unsafe extern "system" fn composer_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        if message == WM_NCCREATE {
            let create = lparam as *const CREATESTRUCTW;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*create).lpCreateParams as isize);
            return DefWindowProcW(hwnd, message, wparam, lparam);
        }
        let ui_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const Ui;
        if ui_ptr.is_null() {
            return DefWindowProcW(hwnd, message, wparam, lparam);
        }
        let ui = &*ui_ptr;
        match message {
            WM_CREATE => match create_children(ui, hwnd) {
                Ok(composer) => {
                    *ui.composer.borrow_mut() = Some(composer);
                    0
                }
                Err(_) => -1,
            },
            WM_COMMAND => {
                let id = (wparam & 0xffff) as usize;
                let notify = (wparam >> 16) as u32;
                if notify == BN_CLICKED as u32 {
                    if id == ID_SEND {
                        let _ = ui.submit();
                    } else if id == ID_CANCEL {
                        ui.cancel_request();
                    }
                }
                0
            }
            WM_PAINT => {
                ui.paints.set(ui.paints.get() + 1);
                DefWindowProcW(hwnd, message, wparam, lparam)
            }
            WM_DPICHANGED => {
                let suggested = lparam as *const RECT;
                let new_dpi = (wparam & 0xffff) as u32;
                let mut frame = RECT {
                    left: 0,
                    top: 0,
                    right: dip(
                        ui.config.manifest.ui.composer_client_width_dip as i32,
                        new_dpi,
                    ),
                    bottom: dip(
                        ui.config.manifest.ui.composer_client_height_dip as i32,
                        new_dpi,
                    ),
                };
                let style = WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_CLIPCHILDREN;
                AdjustWindowRectExForDpi(&mut frame, style, 0, 0, new_dpi);
                SetWindowPos(
                    hwnd,
                    std::ptr::null_mut(),
                    (*suggested).left,
                    (*suggested).top,
                    frame.right - frame.left,
                    frame.bottom - frame.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
                relayout(ui);
                0
            }
            WM_CLOSE => {
                ui.hide_composer();
                0
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}

unsafe fn create_children(ui: &Ui, hwnd: HWND) -> Result<Composer, String> {
    unsafe {
        let module = text::load_richedit()?;
        let dpi = GetDpiForWindow(hwnd);
        let font = CreateFontW(
            -((16 * dpi as i32 + 36) / 72),
            0,
            0,
            0,
            FW_NORMAL as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            OUT_DEFAULT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            CLEARTYPE_QUALITY as u32,
            DEFAULT_PITCH as u32 | FF_DONTCARE as u32,
            text::wide("Segoe UI").as_ptr(),
        );
        let instance = text::instance();
        let ui_conf = &ui.config.manifest.ui;
        let scale = |v: i32| dip(v, dpi);
        let input = text::create_edit(
            hwnd,
            instance,
            scale(12),
            scale(12),
            scale(616),
            scale(ui_conf.input_height_dip as i32),
            false,
            ui_conf.input_limit_utf16_units as usize,
            32,
            font,
            ID_SEND + 10,
        )?;
        text::subclass_input(input, ui)?;
        let response = text::create_edit(
            hwnd,
            instance,
            scale(12),
            scale(152),
            scale(616),
            scale(ui_conf.response_height_dip as i32),
            true,
            ui_conf.response_limit_utf8_bytes as usize + 8192,
            0,
            font,
            ID_SEND + 11,
        )?;
        let status = CreateWindowExW(
            0,
            text::wide("STATIC").as_ptr(),
            text::wide("idle").as_ptr(),
            WS_CHILD | WS_VISIBLE | SS_LEFT,
            scale(12),
            scale(436),
            scale(416),
            scale(28),
            hwnd,
            ID_STATUS as *mut c_void,
            instance,
            std::ptr::null(),
        );
        let send = CreateWindowExW(
            0,
            text::wide("BUTTON").as_ptr(),
            text::wide("Send").as_ptr(),
            WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON as u32,
            scale(440),
            scale(436),
            scale(80),
            scale(28),
            hwnd,
            ID_SEND as *mut c_void,
            instance,
            std::ptr::null(),
        );
        let cancel = CreateWindowExW(
            0,
            text::wide("BUTTON").as_ptr(),
            text::wide("Cancel").as_ptr(),
            WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON as u32,
            scale(528),
            scale(436),
            scale(100),
            scale(28),
            hwnd,
            ID_CANCEL as *mut c_void,
            instance,
            std::ptr::null(),
        );
        SendMessageW(status, WM_SETFONT, font as usize, 1);
        SendMessageW(send, WM_SETFONT, font as usize, 1);
        SendMessageW(cancel, WM_SETFONT, font as usize, 1);
        text::set_text(response, &ui.config.history_prefix);
        Ok(Composer {
            hwnd,
            input,
            response,
            status,
            send,
            cancel,
            font,
            module,
            dpi,
        })
    }
}

unsafe fn relayout(ui: &Ui) {
    unsafe {
        let mut borrowed = ui.composer.borrow_mut();
        if let Some(composer) = borrowed.as_mut() {
            let dpi = GetDpiForWindow(composer.hwnd);
            composer.dpi = dpi;
            let font = CreateFontW(
                -((16 * dpi as i32 + 36) / 72),
                0,
                0,
                0,
                FW_NORMAL as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET as u32,
                OUT_DEFAULT_PRECIS as u32,
                CLIP_DEFAULT_PRECIS as u32,
                CLEARTYPE_QUALITY as u32,
                DEFAULT_PITCH as u32 | FF_DONTCARE as u32,
                text::wide("Segoe UI").as_ptr(),
            );
            if !font.is_null() {
                SendMessageW(composer.input, WM_SETFONT, font as usize, 1);
                SendMessageW(composer.response, WM_SETFONT, font as usize, 1);
                SendMessageW(composer.status, WM_SETFONT, font as usize, 1);
                SendMessageW(composer.send, WM_SETFONT, font as usize, 1);
                SendMessageW(composer.cancel, WM_SETFONT, font as usize, 1);
                DeleteObject(composer.font);
                composer.font = font;
            }
            let scale = |v: i32| dip(v, dpi);
            let ui_conf = &ui.config.manifest.ui;
            SetWindowPos(
                composer.input,
                std::ptr::null_mut(),
                scale(12),
                scale(12),
                scale(616),
                scale(ui_conf.input_height_dip as i32),
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            SetWindowPos(
                composer.response,
                std::ptr::null_mut(),
                scale(12),
                scale(152),
                scale(616),
                scale(ui_conf.response_height_dip as i32),
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            SetWindowPos(
                composer.status,
                std::ptr::null_mut(),
                scale(12),
                scale(436),
                scale(416),
                scale(28),
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            SetWindowPos(
                composer.send,
                std::ptr::null_mut(),
                scale(440),
                scale(436),
                scale(80),
                scale(28),
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            SetWindowPos(
                composer.cancel,
                std::ptr::null_mut(),
                scale(528),
                scale(436),
                scale(100),
                scale(28),
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }
}

pub fn decode_png(path: &std::path::Path) -> Result<Vec<u8>, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("open asset: {e}"))?;
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().map_err(|e| format!("png info: {e}"))?;
    let mut buffer = vec![0u8; reader.output_buffer_size().unwrap_or(128 * 128 * 4 + 64)];
    let info = reader
        .next_frame(&mut buffer)
        .map_err(|e| format!("png frame: {e}"))?;
    if info.width != 128 || info.height != 128 {
        return Err("asset pixel size mismatch".into());
    }
    let rgba = &buffer[..info.buffer_size()];
    let mut premul = vec![0u8; rgba.len()];
    for i in (0..rgba.len()).step_by(4) {
        let (r, g, b, a) = (
            rgba[i] as u32,
            rgba[i + 1] as u32,
            rgba[i + 2] as u32,
            rgba[i + 3] as u32,
        );
        premul[i] = ((b * a + 127) / 255) as u8;
        premul[i + 1] = ((g * a + 127) / 255) as u8;
        premul[i + 2] = ((r * a + 127) / 255) as u8;
        premul[i + 3] = a as u8;
    }
    Ok(premul)
}

pub fn register_classes(instance: HINSTANCE) -> Result<(), String> {
    unsafe {
        let mascot_name = text::wide("MascotRustMascot");
        let mascot_class = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(mascot_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: std::ptr::null_mut(),
            hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
            hbrBackground: std::ptr::null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: mascot_name.as_ptr(),
        };
        if RegisterClassW(&mascot_class) == 0 {
            return Err("mascot class registration failed".into());
        }
        let composer_name = text::wide("MascotRustComposer");
        let composer_class = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(composer_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: std::ptr::null_mut(),
            hCursor: LoadCursorW(std::ptr::null_mut(), IDC_ARROW),
            hbrBackground: std::ptr::null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: composer_name.as_ptr(),
        };
        if RegisterClassW(&composer_class) == 0 {
            return Err("composer class registration failed".into());
        }
    }
    Ok(())
}

pub fn create_mascot(ui: &Ui, instance: HINSTANCE) -> Result<HWND, String> {
    unsafe {
        let mut cursor = POINT { x: 0, y: 0 };
        GetCursorPos(&mut cursor);
        let monitor = MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..std::mem::zeroed()
        };
        GetMonitorInfoW(monitor, &mut info);
        let dpi = GetDpiForSystem();
        let size = dip(ui.config.manifest.asset.logical_width_dip as i32, dpi);
        let x = (cursor.x + 16).clamp(info.rcWork.left, info.rcWork.right - size);
        let y = (cursor.y + 16).clamp(info.rcWork.top, info.rcWork.bottom - size);
        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            text::wide("MascotRustMascot").as_ptr(),
            text::wide("mascot").as_ptr(),
            WS_POPUP,
            x,
            y,
            size,
            size,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance,
            ui as *const Ui as *const c_void,
        );
        if hwnd.is_null() {
            return Err(format!(
                "mascot creation failed: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(hwnd)
    }
}
