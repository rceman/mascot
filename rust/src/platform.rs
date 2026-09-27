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
use windows_sys::Win32::System::Ole::OleUninitialize;
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

fn font_height(dpi: u32) -> i32 {
    -dip(16, dpi)
}

pub fn append_bounded(response: &mut String, text: &str, limit: usize) -> Result<(), String> {
    if text.contains('\0')
        || response
            .len()
            .checked_add(text.len())
            .is_none_or(|n| n > limit)
    {
        return Err("response text exceeds native buffer contract".into());
    }
    response.push_str(text);
    Ok(())
}

pub fn event_matches(
    generation: u64,
    request_id: u64,
    model_generation: u64,
    model_request_id: u64,
) -> bool {
    generation == model_generation && request_id == model_request_id
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

pub struct Surface {
    pub dc: HDC,
    pub bitmap: HBITMAP,
    pub previous: HGDIOBJ,
    pub pixels: usize,
}

impl Surface {
    fn new(source: &[u8], pixels: usize) -> Result<Self, String> {
        unsafe {
            let screen = GetDC(std::ptr::null_mut());
            if screen.is_null() {
                return Err("GetDC failed".into());
            }
            let result = (|| -> Result<Self, String> {
                let memory = CreateCompatibleDC(screen);
                if memory.is_null() {
                    return Err("CreateCompatibleDC failed".into());
                }
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
                if bitmap.is_null() || bits.is_null() {
                    if !bitmap.is_null() {
                        DeleteObject(bitmap);
                    }
                    DeleteDC(memory);
                    return Err("CreateDIBSection failed".into());
                }
                let target = std::slice::from_raw_parts_mut(bits as *mut u8, pixels * pixels * 4);
                for y in 0..pixels {
                    for x in 0..pixels {
                        let sx = x * 128 / pixels;
                        let sy = y * 128 / pixels;
                        let src = (sy * 128 + sx) * 4;
                        let dst = (y * pixels + x) * 4;
                        target[dst..dst + 4].copy_from_slice(&source[src..src + 4]);
                    }
                }
                let previous = SelectObject(memory, bitmap);
                if previous.is_null() {
                    DeleteObject(bitmap);
                    DeleteDC(memory);
                    return Err("SelectObject failed".into());
                }
                Ok(Surface {
                    dc: memory,
                    bitmap,
                    previous,
                    pixels,
                })
            })();
            ReleaseDC(std::ptr::null_mut(), screen);
            result
        }
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.previous);
            DeleteObject(self.bitmap);
            DeleteDC(self.dc);
        }
    }
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
    pub provider_error: Option<String>,
    pub generation: u64,
}

pub struct Ui {
    pub config: Config,
    pub mascot: Cell<HWND>,
    pub mascot_source: Vec<u8>,
    pub surface: RefCell<Option<Surface>>,
    pub composer: RefCell<Option<Composer>>,
    pub model: RefCell<Model>,
    pub composing: Cell<bool>,
    pub snapshot: RefCell<Option<TextSnapshotLocal>>,
    pub ui_events: Arc<BoundedQueue<Event>>,
    pub commands: Arc<BoundedQueue<Value>>,
    pub records: Option<Arc<BoundedQueue<String>>>,
    pub provider: Mutex<Option<Arc<Provider>>>,
    pub control_enabled: bool,
    pub shutdown_started: Cell<bool>,
    pub cancel_pending: Cell<bool>,
    pub pending_shutdown_token: RefCell<Option<Value>>,
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
    pub fn count_paint(&self) {
        self.paints.set(self.paints.get() + 1);
    }

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

    pub fn consume_submit_key(&self, message: &MSG) -> bool {
        if message.message != WM_KEYDOWN || message.wParam as u16 != VK_RETURN as u16 {
            return false;
        }
        let input = {
            let borrowed = self.composer.borrow();
            borrowed
                .as_ref()
                .map(|c| c.input)
                .unwrap_or(std::ptr::null_mut())
        };
        if input.is_null() || message.hwnd != input {
            return false;
        }
        if unsafe { GetKeyState(VK_CONTROL as i32) } >= 0 {
            return false;
        }
        if self.composing.get() {
            return false;
        }
        self.post_submit();
        true
    }

    pub fn post_submit(&self) {
        let mascot = self.mascot.get();
        if !mascot.is_null() {
            unsafe {
                PostMessageW(mascot, WM_APP_SUBMIT, 0, 0);
            }
        }
    }

    pub fn record(&self, value: Value) {
        if !self.control_enabled {
            return;
        }
        let Some(records) = &self.records else {
            return;
        };
        let line = value.to_string();
        if records.try_push(line).is_err() {
            let mut model = self.model.borrow_mut();
            model.run_invalid = Some("control output queue overflow".into());
            eprintln!("control output queue overflow; run invalid");
        }
    }

    fn provider_handle(&self) -> Option<Arc<Provider>> {
        self.provider
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn present_mascot(&self, dpi: u32) -> Result<(), String> {
        let pixels = dip(self.config.manifest.asset.logical_width_dip as i32, dpi) as usize;
        let surface = Surface::new(&self.mascot_source, pixels)?;
        *self.surface.borrow_mut() = Some(surface);
        let (dc, pixels) = {
            let borrowed = self.surface.borrow();
            let surface = borrowed.as_ref().ok_or("surface missing")?;
            (surface.dc, surface.pixels)
        };
        unsafe {
            let window = self.mascot.get();
            let mut bounds = RECT::default();
            if GetWindowRect(window, &mut bounds) == 0 {
                return Err("GetWindowRect failed".into());
            }
            let mut top_left = POINT {
                x: bounds.left,
                y: bounds.top,
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
            let screen = GetDC(std::ptr::null_mut());
            if screen.is_null() {
                return Err("GetDC failed".into());
            }
            let presented = UpdateLayeredWindow(
                window,
                screen,
                &mut top_left,
                &size,
                dc,
                &mut zero,
                0,
                &blend,
                ULW_ALPHA,
            );
            ReleaseDC(std::ptr::null_mut(), screen);
            if presented == 0 {
                return Err("UpdateLayeredWindow failed".into());
            }
        }
        self.presents.set(self.presents.get() + 1);
        Ok(())
    }

    fn composer_handles(&self) -> Option<(HWND, HWND, HWND, HWND, HWND, HWND)> {
        let borrowed = self.composer.borrow();
        borrowed
            .as_ref()
            .map(|c| (c.hwnd, c.input, c.response, c.status, c.send, c.cancel))
    }

    pub fn ensure_composer(&self) -> Result<HWND, String> {
        if let Some((hwnd, ..)) = self.composer_handles() {
            return Ok(hwnd);
        }
        unsafe {
            let instance = text::instance();
            let mascot = self.mascot.get();
            let dpi = GetDpiForWindow(mascot);
            let mut rect = RECT {
                left: 0,
                top: 0,
                right: dip(
                    self.config.manifest.ui.composer_client_width_dip as i32,
                    dpi,
                ),
                bottom: dip(
                    self.config.manifest.ui.composer_client_height_dip as i32,
                    dpi,
                ),
            };
            let style = WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_CLIPCHILDREN;
            AdjustWindowRectExForDpi(&mut rect, style, 0, 0, dpi);
            let mut mascot_rect = RECT::default();
            GetWindowRect(mascot, &mut mascot_rect);
            let mut place = POINT {
                x: mascot_rect.right + 8,
                y: mascot_rect.top,
            };
            let monitor = MonitorFromWindow(mascot, MONITOR_DEFAULTTONEAREST);
            let mut monitor_info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..std::mem::zeroed()
            };
            GetMonitorInfoW(monitor, &mut monitor_info);
            let width = rect.right - rect.left;
            let height = rect.bottom - rect.top;
            place.x = place.x.clamp(
                monitor_info.rcWork.left,
                (monitor_info.rcWork.right - width).max(monitor_info.rcWork.left),
            );
            place.y = place.y.clamp(
                monitor_info.rcWork.top,
                (monitor_info.rcWork.bottom - height).max(monitor_info.rcWork.top),
            );
            let class = text::wide("MascotRustComposer");
            let title = text::wide("mascot");
            let hwnd = CreateWindowExW(
                0,
                class.as_ptr(),
                title.as_ptr(),
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
        let input = self
            .composer_handles()
            .map(|(_, input, ..)| input)
            .unwrap_or(std::ptr::null_mut());
        unsafe {
            ShowWindow(hwnd, SW_SHOWNORMAL);
            SetForegroundWindow(hwnd);
            if !input.is_null() {
                SetFocus(input);
            }
        }
        Ok(())
    }

    pub fn hide_composer(&self) {
        let composer = self.composer_handles();
        let snapshot = self.snapshot.borrow().clone();
        if let Some((_, input, ..)) = composer {
            if self.composing.get() {
                text::cancel_composition(input);
                if let Some(snapshot) = snapshot {
                    text::set_text(input, &snapshot.text);
                    text::set_selection(input, snapshot.sel_start, snapshot.sel_end);
                }
                self.composing.set(false);
            }
            if let Some((hwnd, ..)) = composer {
                unsafe {
                    ShowWindow(hwnd, SW_HIDE);
                }
            }
        }
    }

    pub fn submit(&self) -> Result<(), String> {
        if self.shutdown_started.get() {
            return Err("shutdown in progress".into());
        }
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
        let provider = self.provider_handle();
        let mut model = self.model.borrow_mut();
        let id = model.request_count + 1;
        let scenario = std::mem::replace(&mut model.scenario, "normal".to_string());
        model.request_count = id;
        model.request_id = id;
        model.last_seq = -1;
        model.response.clear();
        model.provider_error = None;
        model.provider_state = "streaming".into();
        self.cancel_pending.set(false);
        drop(model);
        if let Some(provider) = provider {
            if let Err(error) = provider.send(Command::Request {
                id,
                prompt,
                scenario,
            }) {
                let mut model = self.model.borrow_mut();
                model.provider_state = "failed".into();
                model.run_invalid = Some(error.clone());
                drop(model);
                return Err(error);
            }
        }
        self.reset_response_view();
        self.refresh_status();
        Ok(())
    }

    pub fn cancel_request(&self) -> Result<(), String> {
        let id = {
            let model = self.model.borrow();
            if model.provider_state != "streaming"
                || model.request_id == 0
                || self.cancel_pending.get()
            {
                return Ok(());
            }
            model.request_id
        };
        let Some(provider) = self.provider_handle() else {
            return Ok(());
        };
        match provider.send(Command::Cancel { id }) {
            Ok(()) => {
                self.cancel_pending.set(true);
                Ok(())
            }
            Err(error) => {
                self.model.borrow_mut().run_invalid =
                    Some(format!("cancel enqueue failed: {error}"));
                Err(error)
            }
        }
    }

    fn reset_response_view(&self) {
        if let Some((_, _, response, ..)) = self.composer_handles() {
            text::set_text(response, &self.config.history_prefix);
        }
    }

    fn append_response_view(&self, text: &str) {
        if let Some((_, _, response, ..)) = self.composer_handles() {
            text::append_text(response, text);
        }
    }

    pub fn refresh_status(&self) {
        let text = {
            let model = self.model.borrow();
            model.provider_state.clone()
        };
        if let Some((_, _, _, status, ..)) = self.composer_handles() {
            let wide = text::wide(&text);
            unsafe {
                SetWindowTextW(status, wide.as_ptr());
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
                Event::Started { generation, pid } => {
                    let mut model = self.model.borrow_mut();
                    if generation >= model.generation {
                        model.generation = generation;
                        model.provider_pid = pid;
                    }
                    drop(model);
                    self.refresh_status();
                }
                Event::Chunk {
                    generation,
                    id,
                    seq,
                    text,
                    ..
                } => {
                    let accepted = {
                        let mut model = self.model.borrow_mut();
                        if !event_matches(generation, id, model.generation, model.request_id) {
                            None
                        } else {
                            let limit = self.config.manifest.ui.response_limit_utf8_bytes as usize;
                            match append_bounded(&mut model.response, &text, limit) {
                                Ok(()) => {
                                    model.last_seq = seq as i64;
                                    Some(qpc())
                                }
                                Err(error) => {
                                    model.run_invalid = Some(error);
                                    None
                                }
                            }
                        }
                    };
                    match accepted {
                        Some(accepted_qpc) => {
                            self.record(json!({
                                "event": "chunk_accepted",
                                "request_id": id,
                                "seq": seq,
                                "accepted_qpc": accepted_qpc.to_string(),
                            }));
                            self.append_response_view(&text);
                        }
                        None => {
                            let run_invalid = self.model.borrow().run_invalid.clone();
                            if run_invalid.is_some() {
                                let _ = self.cancel_request();
                            }
                        }
                    }
                }
                Event::Terminal {
                    generation,
                    id,
                    kind,
                    last_seq,
                } => {
                    let matched = {
                        let mut model = self.model.borrow_mut();
                        if event_matches(generation, id, model.generation, model.request_id) {
                            model.provider_state = kind.clone();
                            model.last_seq = last_seq;
                            self.cancel_pending.set(false);
                            true
                        } else {
                            false
                        }
                    };
                    if matched {
                        self.record(json!({
                            "event": "terminal",
                            "request_id": id,
                            "kind": kind,
                            "last_seq": last_seq,
                            "qpc": qpc().to_string(),
                        }));
                    }
                    self.refresh_status();
                }
                Event::SessionClosed {
                    generation, error, ..
                } => {
                    {
                        let mut model = self.model.borrow_mut();
                        if generation == model.generation {
                            model.provider_pid = 0;
                            if let Some(error) = error.as_ref() {
                                model.provider_state = "failed".into();
                                model.provider_error = Some(error.clone());
                            }
                        }
                    }
                    self.refresh_status();
                }
                Event::Stopped { error } => {
                    let token = self.pending_shutdown_token.borrow_mut().take();
                    if let Some(token) = token {
                        match &error {
                            Some(error) => {
                                self.record(json!({"token": token, "ok": false, "error": error}))
                            }
                            None => self.record(json!({"token": token, "ok": true})),
                        }
                    }
                    self.commands.close();
                    if let Some(records) = &self.records {
                        records.close();
                    }
                    unsafe {
                        PostQuitMessage(if error.is_some() { 1 } else { 0 });
                    }
                }
            }
        }
    }

    pub fn state_json(&self) -> Value {
        let handles = self.composer_handles();
        let visible = handles
            .map(|(hwnd, ..)| unsafe { IsWindowVisible(hwnd) != 0 })
            .unwrap_or(false);
        let (composer, input, response) = handles
            .map(|(hwnd, input, response, ..)| {
                (
                    (hwnd as usize).to_string(),
                    (input as usize).to_string(),
                    (response as usize).to_string(),
                )
            })
            .unwrap_or(("0".into(), "0".into(), "0".into()));
        let stderr_total = self
            .provider_handle()
            .map(|p| p.stderr_total())
            .unwrap_or(0);
        let dib = self.surface.borrow().is_some() as u64;
        let model = self.model.borrow();
        json!({
            "pid": std::process::id(),
            "mascot_hwnd": (self.mascot.get() as usize).to_string(),
            "composer_hwnd": composer,
            "input_hwnd": input,
            "response_hwnd": response,
            "composer_visible": visible,
            "composing": self.composing.get(),
            "provider_pid": model.provider_pid,
            "provider_state": model.provider_state,
            "request_id": model.request_id,
            "request_count": model.request_count,
            "last_seq": model.last_seq,
            "response_utf8_bytes": model.response.len(),
            "mascot_presents": self.presents.get(),
            "composer_paints": self.paints.get(),
            "queued_provider_frames": self.ui_events.len(),
            "queue_capacity_frames": self.ui_events.capacity() as u64,
            "cache_counts": {
                "stderr_tail_bytes": stderr_total.min(4096),
                "stderr_total_bytes": stderr_total,
                "retained_response_utf8_bytes": model.response.len(),
                "dib_buffers": dib,
                "richedit_layout_cache_bytes": {"value": null, "reason": "opaque system RichEdit layout cache"},
                "caret_draws": {"value": null, "reason": "native caret drawing does not surface through WM_PAINT"},
                "run_invalid": model.run_invalid,
                "provider_error": model.provider_error,
            }
        })
    }

    pub fn text_snapshot(&self) -> Result<text::TextSnapshot, String> {
        let (_, input, response, ..) = self.composer_handles().ok_or("composer not created")?;
        let input_text = text::get_text(input);
        let full = text::get_text(response);
        let response_text = full
            .strip_prefix(&self.config.history_prefix)
            .unwrap_or(&full)
            .to_string();
        let range = text::get_selection(input);
        Ok(text::TextSnapshot {
            input: input_text,
            response: response_text,
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
        let (_, input, ..) = self.composer_handles().ok_or("composer not created")?;
        text::set_text(input, value);
        let range = text::get_selection(input);
        *self.snapshot.borrow_mut() = Some(TextSnapshotLocal {
            text: text::get_text(input),
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
            "cancel" => match self.cancel_request() {
                Ok(()) => self.reply(&token, true, None),
                Err(error) => self.reply_error(&token, error),
            },
            "shutdown" => {
                self.request_shutdown(Some(token));
            }
            _ => self.reply_error(&token, format!("unknown command '{name}'")),
        }
    }

    pub fn request_shutdown(&self, token: Option<Value>) {
        if let Some(token) = token {
            *self.pending_shutdown_token.borrow_mut() = Some(token);
        }
        if self.shutdown_started.replace(true) {
            return;
        }
        self.hide_composer();
        if let Some(provider) = self.provider_handle() {
            if let Err(error) = provider.send(Command::Shutdown) {
                eprintln!("provider shutdown request failed: {error}");
                unsafe {
                    PostQuitMessage(64);
                }
            }
        } else {
            unsafe {
                PostQuitMessage(0);
            }
        }
    }

    pub fn teardown(&self) {
        let provider = self
            .provider
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        if let Some(provider) = provider {
            let joined = provider.join(std::time::Duration::from_millis(3000));
            if !joined {
                eprintln!("provider coordinator did not stop within teardown bound");
            }
        }
        self.commands.close();
        if let Some(records) = &self.records {
            records.close();
        }
        let composer = self.composer.borrow_mut().take();
        if let Some(composer) = composer {
            unsafe {
                DestroyWindow(composer.input);
                DestroyWindow(composer.response);
                DestroyWindow(composer.status);
                DestroyWindow(composer.send);
                DestroyWindow(composer.cancel);
                DestroyWindow(composer.hwnd);
                DeleteObject(composer.font);
                FreeLibrary(composer.module);
                OleUninitialize();
            }
        }
        self.surface.borrow_mut().take();
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
            let ui_ptr = (*create).lpCreateParams as *const Ui;
            if !ui_ptr.is_null() {
                (*ui_ptr).mascot.set(hwnd);
            }
            return DefWindowProcW(hwnd, message, wparam, lparam);
        }
        let ui_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const Ui;
        if ui_ptr.is_null() {
            return DefWindowProcW(hwnd, message, wparam, lparam);
        }
        let ui = &*ui_ptr;
        match message {
            WM_APP_PROVIDER => {
                ui.dispatch_provider_events();
                0
            }
            WM_APP_CONTROL => {
                ui.dispatch_control();
                0
            }
            WM_APP_SUBMIT => {
                if let Err(error) = ui.submit() {
                    eprintln!("submit rejected: {error}");
                }
                0
            }
            WM_HOTKEY => {
                if wparam as i32 == HOTKEY_TOGGLE_ID {
                    let visible = {
                        let borrowed = ui.composer.borrow();
                        borrowed
                            .as_ref()
                            .map(|c| IsWindowVisible(c.hwnd) != 0)
                            .unwrap_or(false)
                    };
                    if visible {
                        ui.hide_composer();
                    } else if let Err(error) = ui.show_composer() {
                        eprintln!("show failed: {error}");
                    }
                } else if wparam as i32 == HOTKEY_CANCEL_ID {
                    let _ = ui.cancel_request();
                }
                0
            }
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
                let new_dpi = (wparam & 0xffff) as u32;
                let size = dip(ui.config.manifest.asset.logical_width_dip as i32, new_dpi);
                SetWindowPos(
                    hwnd,
                    std::ptr::null_mut(),
                    (*suggested).left,
                    (*suggested).top,
                    size,
                    size,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
                if let Err(error) = ui.present_mascot(new_dpi) {
                    ui.model.borrow_mut().run_invalid =
                        Some(format!("mascot presentation failed: {error}"));
                }
                0
            }
            WM_RBUTTONUP | WM_NCRBUTTONUP => {
                let menu = CreatePopupMenu();
                if !menu.is_null() {
                    let label = text::wide("Exit");
                    AppendMenuW(menu, MF_STRING, ID_MENU_EXIT, label.as_ptr());
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
                        ui.request_shutdown(None);
                    }
                }
                0
            }
            WM_CLOSE => {
                ui.request_shutdown(None);
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
                        if let Err(error) = ui.submit() {
                            eprintln!("submit rejected: {error}");
                        }
                    } else if id == ID_CANCEL {
                        let _ = ui.cancel_request();
                    }
                }
                0
            }
            WM_PAINT => {
                ui.count_paint();
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
            font_height(dpi),
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
        if font.is_null() {
            FreeLibrary(module);
            OleUninitialize();
            return Err("font creation failed".into());
        }
        let instance = text::instance();
        let ui_conf = &ui.config.manifest.ui;
        let scale = |v: i32| dip(v, dpi);
        let mut created: Vec<HWND> = Vec::new();
        let result = (|| -> Result<Composer, String> {
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
            created.push(input);
            text::subclass_control(input, ui, true)?;
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
            created.push(response);
            text::subclass_control(response, ui, false)?;
            let static_name = text::wide("STATIC");
            let static_text = text::wide("idle");
            let status = CreateWindowExW(
                0,
                static_name.as_ptr(),
                static_text.as_ptr(),
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
            if status.is_null() {
                return Err("status creation failed".into());
            }
            created.push(status);
            text::subclass_control(status, ui, false)?;
            let button_name = text::wide("BUTTON");
            let send_label = text::wide("Send");
            let send = CreateWindowExW(
                0,
                button_name.as_ptr(),
                send_label.as_ptr(),
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
            if send.is_null() {
                return Err("send button creation failed".into());
            }
            created.push(send);
            text::subclass_control(send, ui, false)?;
            let cancel_label = text::wide("Cancel");
            let cancel = CreateWindowExW(
                0,
                button_name.as_ptr(),
                cancel_label.as_ptr(),
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
            if cancel.is_null() {
                return Err("cancel button creation failed".into());
            }
            created.push(cancel);
            text::subclass_control(cancel, ui, false)?;
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
        })();
        if result.is_err() {
            for child in created {
                DestroyWindow(child);
            }
            DeleteObject(font);
            FreeLibrary(module);
            OleUninitialize();
        }
        result
    }
}

unsafe fn relayout(ui: &Ui) {
    unsafe {
        let handles = ui.composer_handles();
        let Some((hwnd, input, response, status, send, cancel)) = handles else {
            return;
        };
        let dpi = GetDpiForWindow(hwnd);
        let font = CreateFontW(
            font_height(dpi),
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
            SendMessageW(input, WM_SETFONT, font as usize, 1);
            SendMessageW(response, WM_SETFONT, font as usize, 1);
            SendMessageW(status, WM_SETFONT, font as usize, 1);
            SendMessageW(send, WM_SETFONT, font as usize, 1);
            SendMessageW(cancel, WM_SETFONT, font as usize, 1);
            let mut borrowed = ui.composer.borrow_mut();
            if let Some(composer) = borrowed.as_mut() {
                DeleteObject(composer.font);
                composer.font = font;
                composer.dpi = dpi;
            }
        }
        let scale = |v: i32| dip(v, dpi);
        let ui_conf = &ui.config.manifest.ui;
        SetWindowPos(
            input,
            std::ptr::null_mut(),
            scale(12),
            scale(12),
            scale(616),
            scale(ui_conf.input_height_dip as i32),
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
        SetWindowPos(
            response,
            std::ptr::null_mut(),
            scale(12),
            scale(152),
            scale(616),
            scale(ui_conf.response_height_dip as i32),
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
        SetWindowPos(
            status,
            std::ptr::null_mut(),
            scale(12),
            scale(436),
            scale(416),
            scale(28),
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
        SetWindowPos(
            send,
            std::ptr::null_mut(),
            scale(440),
            scale(436),
            scale(80),
            scale(28),
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
        SetWindowPos(
            cancel,
            std::ptr::null_mut(),
            scale(528),
            scale(436),
            scale(100),
            scale(28),
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
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
            hbrBackground: GetSysColorBrush(COLOR_WINDOW),
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
        let x = (cursor.x + 16).clamp(
            info.rcWork.left,
            (info.rcWork.right - size).max(info.rcWork.left),
        );
        let y = (cursor.y + 16).clamp(
            info.rcWork.top,
            (info.rcWork.bottom - size).max(info.rcWork.top),
        );
        let class = text::wide("MascotRustMascot");
        let title = text::wide("mascot");
        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class.as_ptr(),
            title.as_ptr(),
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

#[cfg(test)]
mod tests {
    use super::{append_bounded, event_matches, font_height};

    #[test]
    fn font_height_scales_with_dpi() {
        assert_eq!(font_height(96), -16);
        assert_eq!(font_height(192), -32);
    }

    #[test]
    fn append_bounded_enforces_limit_and_nul() {
        let mut response = String::from("A");
        append_bounded(&mut response, "é", 3).unwrap();
        assert_eq!(response, "Aé");
        assert!(append_bounded(&mut response, "B", 3).is_err());
        assert_eq!(response, "Aé");
        assert!(append_bounded(&mut response, "x\0y", 10).is_err());
        assert_eq!(response, "Aé");
    }

    #[test]
    fn stale_generation_events_do_not_match() {
        assert!(!event_matches(1, 7, 2, 7));
        assert!(event_matches(2, 7, 2, 7));
        assert!(!event_matches(2, 8, 2, 7));
    }
}
