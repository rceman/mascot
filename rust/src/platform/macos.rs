use crate::config::Config;
use crate::provider::{Command, Event, Provider, qpc};
use crate::queue::BoundedQueue;
use objc2::rc::{Retained, autoreleasepool};
use objc2::runtime::{AnyObject, Bool, ProtocolObject};
use objc2::{AnyThread, DefinedClass, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSBezelStyle, NSBitmapImageRep,
    NSBorderType, NSButton, NSColor, NSEvent, NSEventMask, NSEventModifierFlags, NSEventType,
    NSEventTrackingRunLoopMode, NSFloatingWindowLevel, NSFont, NSImage, NSPanel, NSScreen,
    NSScrollView, NSTextDelegate, NSTextField, NSTextInputClient, NSTextView, NSTextViewDelegate,
    NSView, NSWindow, NSWindowCollectionBehavior, NSWindowDelegate, NSWindowStyleMask,
};
use objc2_foundation::{
    MainThreadMarker, NSNotification, NSObject, NSObjectProtocol, NSPoint, NSRange, NSRect, NSSize,
    NSString,
};
use serde_json::{Value, json};
use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::sync::{Arc, Mutex};

const K_VK_SPACE: u32 = 49;
const K_VK_ESCAPE: u32 = 53;
const K_VK_RETURN: u16 = 36;
const K_EVENT_CLASS_KEYBOARD: u32 = u32::from_be_bytes(*b"keyb");
const K_EVENT_HOT_KEY_PRESSED: u32 = 6;
const K_EVENT_PARAM_DIRECT_OBJECT: u32 = u32::from_be_bytes(*b"----");
const TYPE_EVENT_HOT_KEY_ID: u32 = u32::from_be_bytes(*b"hkid");
const CONTROL_KEY: u32 = 0x1000;
const OPTION_KEY: u32 = 0x0800;
const HOTKEY_TOGGLE_ID: u32 = 1;
const HOTKEY_CANCEL_ID: u32 = 2;
// Overlap in points so the mascot appears seated on the composer top edge
// rather than floating above it.
const MASCOT_PERCH_OVERLAP: f64 = 6.0;

#[repr(C)]
struct EventHotKeyID {
    signature: u32,
    id: u32,
}

#[repr(C)]
struct EventTypeSpec {
    event_class: u32,
    event_kind: u32,
}

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    fn RegisterEventHotKey(
        key_code: u32,
        modifiers: u32,
        hot_key_id: EventHotKeyID,
        target: *mut c_void,
        options: u32,
        out: *mut *mut c_void,
    ) -> i32;
    fn UnregisterEventHotKey(hot_key: *mut c_void) -> i32;
    fn GetApplicationEventTarget() -> *mut c_void;
    fn InstallEventHandler(
        target: *mut c_void,
        handler: extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> i32,
        num_types: usize,
        list: *const EventTypeSpec,
        user_data: *mut c_void,
        out_ref: *mut *mut c_void,
    ) -> i32;
    fn RemoveEventHandler(handler: *mut c_void) -> i32;
    fn GetEventParameter(
        event: *mut c_void,
        name: u32,
        desired_type: u32,
        out_actual_type: *mut u32,
        buffer_size: usize,
        out_actual_size: *mut usize,
        out_data: *mut c_void,
    ) -> i32;
}

unsafe extern "C" {
    static _dispatch_main_q: c_void;
    fn dispatch_async_f(queue: *mut c_void, context: *mut c_void, work: extern "C" fn(*mut c_void));
}

fn main_queue() -> *mut c_void {
    unsafe { &_dispatch_main_q as *const c_void as *mut c_void }
}

extern "C" fn provider_wake_main(context: *mut c_void) {
    let ui = unsafe { &*(context as *const Ui) };
    ui.dispatch_provider_events();
}

extern "C" fn control_wake_main(context: *mut c_void) {
    let ui = unsafe { &*(context as *const Ui) };
    ui.dispatch_control();
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

pub struct Composer {
    pub window: Retained<NSWindow>,
    pub input: Retained<InputTextView>,
    pub response: Retained<ResponseTextView>,
    pub status: Retained<NSTextField>,
    pub send: Retained<NSButton>,
    pub cancel: Retained<NSButton>,
}

#[derive(Clone)]
pub struct TextSnapshotLocal {
    pub text: String,
    pub sel_start: i32,
    pub sel_end: i32,
}

pub struct TextSnapshot {
    pub input: String,
    pub response: String,
    pub selection_start: i32,
    pub selection_end: i32,
    pub composing: bool,
}

pub struct Ui {
    pub config: Config,
    pub mascot: Cell<*mut c_void>,
    pub mascot_source: Vec<u8>,
    pub mascot_src_w: u32,
    pub mascot_src_h: u32,
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
    pub hotkey_refs: RefCell<Vec<*mut c_void>>,
    pub event_handler: Cell<*mut c_void>,
}

fn ns(text: &str) -> Retained<NSString> {
    NSString::from_str(text)
}

fn get_text(tv: &NSTextView) -> String {
    tv.string()
        .to_string()
        .replace("\r\n", "\n")
        .replace('\r', "\n")
}

fn set_text(tv: &NSTextView, value: &str) {
    tv.setString(&ns(value));
}

fn get_selection(tv: &NSTextView) -> (i32, i32) {
    let range = tv.selectedRange();
    (range.location as i32, (range.location + range.length) as i32)
}

fn utf16_units(value: &str) -> usize {
    value.encode_utf16().count()
}

impl Ui {
    pub fn count_paint(&self) {
        self.paints.set(self.paints.get() + 1);
    }

    pub fn composing(&self) -> bool {
        self.composing.get()
    }

    fn on_marked_changed(&self, tv: &NSTextView) {
        let marked = tv.hasMarkedText();
        if marked && !self.composing.get() {
            let (start, end) = get_selection(tv);
            *self.snapshot.borrow_mut() = Some(TextSnapshotLocal {
                text: get_text(tv),
                sel_start: start,
                sel_end: end,
            });
        }
        self.composing.set(marked);
    }

    pub fn control_waker(&self) -> Arc<dyn Fn() + Send + Sync> {
        let ctx = self as *const Ui as usize;
        Arc::new(move || unsafe {
            dispatch_async_f(main_queue(), ctx as *mut c_void, control_wake_main);
        })
    }

    fn ui_waker(&self) -> Arc<dyn Fn() + Send + Sync> {
        let ctx = self as *const Ui as usize;
        Arc::new(move || unsafe {
            dispatch_async_f(main_queue(), ctx as *mut c_void, provider_wake_main);
        })
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

    pub fn ensure_composer(&self, mtm: MainThreadMarker) -> Result<(), String> {
        if self.composer.borrow().is_some() {
            return Ok(());
        }
        let ui_conf = &self.config.manifest.ui;
        let width = ui_conf.composer_client_width_dip as f64;
        let height = ui_conf.composer_client_height_dip as f64;
        let margin = ui_conf.margin_dip as f64;
        let mut origin = NSPoint::new(0.0, 0.0);
        let mascot = self.mascot.get();
        if !mascot.is_null() {
            let frame: NSRect = unsafe { msg_send![mascot as *mut AnyObject, frame] };
            origin.x = frame.origin.x + frame.size.width + 8.0;
            origin.y = frame.origin.y + frame.size.height - height;
        }
        if let Some(screen) = NSScreen::mainScreen(mtm) {
            let visible = screen.visibleFrame();
            origin.x = origin
                .x
                .clamp(visible.origin.x, (visible.origin.x + visible.size.width - width).max(visible.origin.x));
            origin.y = origin
                .y
                .clamp(visible.origin.y, (visible.origin.y + visible.size.height - height).max(visible.origin.y));
        }
        let rect = NSRect::new(origin, NSSize::new(width, height));
        let window: Retained<NSWindow> = unsafe {
            msg_send![
                NSWindow::alloc(mtm),
                initWithContentRect: rect,
                styleMask: NSWindowStyleMask::Titled
                    | NSWindowStyleMask::Closable
                    | NSWindowStyleMask::Miniaturizable,
                backing: NSBackingStoreType::Buffered,
                defer: false
            ]
        };
        window.setTitle(&ns("mascot"));
        unsafe { window.setReleasedWhenClosed(false) };

        let delegate: Retained<ComposerDelegate> = unsafe {
            let this = ComposerDelegate::alloc(mtm).set_ivars(ComposerIvars {
                ui: self as *const Ui as usize,
            });
            msg_send![super(this), init]
        };
        window.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        std::mem::forget(delegate.clone());

        let Some(content) = window.contentView() else {
            return Err("composer content view missing".into());
        };

        let font = NSFont::systemFontOfSize(16.0);
        let input_top = height - margin - ui_conf.input_height_dip as f64;
        let input: Retained<InputTextView> = unsafe {
            let this = InputTextView::alloc(mtm).set_ivars(TextIvars {
                ui: self as *const Ui as usize,
            });
            msg_send![super(this), initWithFrame: NSRect::new(
                NSPoint::new(0.0, 0.0),
                NSSize::new(width - 2.0 * margin, ui_conf.input_height_dip as f64),
            )]
        };
        configure_text_view(&input, true, &font);
        input.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));

        let response_top = input_top - 12.0 - ui_conf.response_height_dip as f64;
        let response: Retained<ResponseTextView> = unsafe {
            let this = ResponseTextView::alloc(mtm).set_ivars(TextIvars {
                ui: self as *const Ui as usize,
            });
            msg_send![super(this), initWithFrame: NSRect::new(
                NSPoint::new(0.0, 0.0),
                NSSize::new(width - 2.0 * margin, ui_conf.response_height_dip as f64),
            )]
        };
        configure_text_view(&response, false, &font);
        set_text(&response, &self.config.history_prefix);

        let input_scroll = scroll_wrapper(
            mtm,
            NSRect::new(
                NSPoint::new(margin, input_top),
                NSSize::new(width - 2.0 * margin, ui_conf.input_height_dip as f64),
            ),
            &input,
        );
        let response_scroll = scroll_wrapper(
            mtm,
            NSRect::new(
                NSPoint::new(margin, response_top),
                NSSize::new(width - 2.0 * margin, ui_conf.response_height_dip as f64),
            ),
            &response,
        );
        content.addSubview(&input_scroll);
        content.addSubview(&response_scroll);

        let status_y = response_top - 12.0 - 28.0;
        let status: Retained<NSTextField> = unsafe {
            msg_send![
                NSTextField::alloc(mtm),
                initWithFrame: NSRect::new(NSPoint::new(margin, status_y), NSSize::new(416.0, 28.0))
            ]
        };
        status.setStringValue(&ns("idle"));
        status.setEditable(false);
        status.setSelectable(false);
        status.setBordered(false);
        status.setDrawsBackground(false);
        unsafe { status.setFont(Some(&font)) };
        content.addSubview(&status);

        let send: Retained<NSButton> = unsafe {
            msg_send![
                NSButton::alloc(mtm),
                initWithFrame: NSRect::new(
                    NSPoint::new(width - margin - 80.0 - 12.0 - 100.0, status_y),
                    NSSize::new(80.0, 28.0),
                )
            ]
        };
        send.setTitle(&ns("Send"));
        send.setBezelStyle(NSBezelStyle::Rounded);
        unsafe {
            send.setTarget(Some(&*delegate));
            send.setAction(Some(sel!(sendAction:)));
        }
        content.addSubview(&send);

        let cancel: Retained<NSButton> = unsafe {
            msg_send![
                NSButton::alloc(mtm),
                initWithFrame: NSRect::new(
                    NSPoint::new(width - margin - 100.0, status_y),
                    NSSize::new(100.0, 28.0),
                )
            ]
        };
        cancel.setTitle(&ns("Cancel"));
        cancel.setBezelStyle(NSBezelStyle::Rounded);
        unsafe {
            cancel.setTarget(Some(&*delegate));
            cancel.setAction(Some(sel!(cancelAction:)));
        }
        content.addSubview(&cancel);

        *self.composer.borrow_mut() = Some(Composer {
            window,
            input,
            response,
            status,
            send,
            cancel,
        });
        Ok(())
    }

    pub fn show_composer(&self, mtm: MainThreadMarker) -> Result<(), String> {
        self.ensure_composer(mtm)?;
        let app = NSApplication::sharedApplication(mtm);
        let composer = self.composer.borrow();
        let composer = composer.as_ref().ok_or("composer not created")?;
        app.activate();
        composer.window.makeKeyAndOrderFront(None);
        composer
            .window
            .makeFirstResponder(Some(&*composer.input));
        drop(composer);
        self.perch_mascot();
        Ok(())
    }

    // The mascot perches on the composer top edge while the composer is
    // visible; composer moves/resizes re-anchor it through this method.
    pub fn perch_mascot(&self) {
        let mascot = self.mascot.get();
        if mascot.is_null() {
            return;
        }
        let borrowed = self.composer.borrow();
        let Some(composer) = borrowed.as_ref() else {
            return;
        };
        if !composer.window.isVisible() {
            return;
        }
        let frame = composer.window.frame();
        let dip = self.config.manifest.asset.logical_width_dip as f64;
        let origin = NSPoint::new(
            frame.origin.x + (frame.size.width - dip) * 0.5,
            frame.origin.y + frame.size.height - MASCOT_PERCH_OVERLAP,
        );
        drop(borrowed);
        unsafe {
            let _: () = msg_send![mascot as *mut AnyObject, setFrameOrigin: origin];
        }
    }

    pub fn hide_composer(&self) {
        let composer = self.composer.borrow();
        let Some(composer) = composer.as_ref() else {
            return;
        };
        let snapshot = self.snapshot.borrow().clone();
        if self.composing.get() || composer.input.hasMarkedText() {
            unsafe {
                composer.input.unmarkText();
            }
            if let Some(snapshot) = snapshot {
                set_text(&composer.input, &snapshot.text);
                composer.input.setSelectedRange(NSRange::new(
                    snapshot.sel_start.max(0) as usize,
                    (snapshot.sel_end - snapshot.sel_start).max(0) as usize,
                ));
            }
            self.composing.set(false);
        }
        composer.window.orderOut(None);
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
        let prompt = {
            let borrowed = self.composer.borrow();
            let composer = borrowed.as_ref().ok_or("composer not created")?;
            get_text(&composer.input)
        };
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
        let borrowed = self.composer.borrow();
        if let Some(composer) = borrowed.as_ref() {
            set_text(&composer.response, &self.config.history_prefix);
        }
    }

    fn append_response_view(&self, text: &str) {
        let borrowed = self.composer.borrow();
        let Some(composer) = borrowed.as_ref() else {
            return;
        };
        let Some(storage) = (unsafe { composer.response.textStorage() }) else {
            return;
        };
        storage
            .mutableString()
            .appendString(&ns(text));
        let end = storage.mutableString().length();
        composer
            .response
            .scrollRangeToVisible(NSRange::new(end.saturating_sub(1), 0));
    }

    pub fn refresh_status(&self) {
        let text = {
            let model = self.model.borrow();
            model.provider_state.clone()
        };
        let borrowed = self.composer.borrow();
        if let Some(composer) = borrowed.as_ref() {
            composer.status.setStringValue(&ns(&text));
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
                    if let Some(mtm) = MainThreadMarker::new() {
                        NSApplication::sharedApplication(mtm).terminate(None);
                    }
                }
            }
        }
    }

    pub fn state_json(&self) -> Value {
        let composer = self.composer.borrow();
        let (visible, window_id, input_id, response_id) = composer
            .as_ref()
            .map(|c| {
                (
                    c.window.isVisible(),
                    (Retained::as_ptr(&c.window) as usize).to_string(),
                    (Retained::as_ptr(&c.input) as usize).to_string(),
                    (Retained::as_ptr(&c.response) as usize).to_string(),
                )
            })
            .unwrap_or((false, "0".into(), "0".into(), "0".into()));
        let stderr_total = self
            .provider_handle()
            .map(|p| p.stderr_total())
            .unwrap_or(0);
        let model = self.model.borrow();
        json!({
            "pid": std::process::id(),
            "mascot_hwnd": (self.mascot.get() as usize).to_string(),
            "composer_hwnd": window_id,
            "input_hwnd": input_id,
            "response_hwnd": response_id,
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
                "dib_buffers": {"value": null, "reason": "no DIB section cache; NSBitmapImageRep owned by NSImage"},
                "richedit_layout_cache_bytes": {"value": null, "reason": "opaque NSTextView layout cache"},
                "caret_draws": {"value": null, "reason": "native caret drawing does not surface through drawRect"},
                "run_invalid": model.run_invalid,
                "provider_error": model.provider_error,
            }
        })
    }

    pub fn text_snapshot(&self) -> Result<TextSnapshot, String> {
        let borrowed = self.composer.borrow();
        let composer = borrowed.as_ref().ok_or("composer not created")?;
        let input_text = get_text(&composer.input);
        let full = get_text(&composer.response);
        let response_text = full
            .strip_prefix(&self.config.history_prefix)
            .unwrap_or(&full)
            .to_string();
        let (start, end) = get_selection(&composer.input);
        Ok(TextSnapshot {
            input: input_text,
            response: response_text,
            selection_start: start,
            selection_end: end,
            composing: self.composing.get(),
        })
    }

    pub fn set_input_text(&self, value: &str) -> Result<(), String> {
        if value.contains('\0') {
            return Err("text contains NUL".into());
        }
        if utf16_units(value) > self.config.manifest.ui.input_limit_utf16_units as usize {
            return Err("text exceeds input limit".into());
        }
        let borrowed = self.composer.borrow();
        let composer = borrowed.as_ref().ok_or("composer not created")?;
        set_text(&composer.input, value);
        let (start, end) = get_selection(&composer.input);
        *self.snapshot.borrow_mut() = Some(TextSnapshotLocal {
            text: get_text(&composer.input),
            sel_start: start,
            sel_end: end,
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
            "show" => {
                let mtm = MainThreadMarker::new();
                if let Some(mtm) = mtm {
                    match self.show_composer(mtm) {
                        Ok(()) => self.reply(&token, true, None),
                        Err(error) => self.reply_error(&token, error),
                    }
                } else {
                    self.reply_error(&token, "not on main thread".into());
                }
            }
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
        let quit = |code| {
            let _ = code;
            if let Some(mtm) = MainThreadMarker::new() {
                NSApplication::sharedApplication(mtm).terminate(None);
            }
        };
        if let Some(provider) = self.provider_handle() {
            if let Err(error) = provider.send(Command::Shutdown) {
                eprintln!("provider shutdown request failed: {error}");
                quit(64);
            }
        } else {
            quit(0);
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
        {
            let borrowed = self.composer.borrow();
            if let Some(composer) = borrowed.as_ref() {
                composer.window.orderOut(None);
                composer.window.setDelegate(None);
            }
        }
        let mascot = self.mascot.get();
        if !mascot.is_null() {
            unsafe {
                let _: () = msg_send![mascot as *mut AnyObject, orderOut: std::ptr::null::<AnyObject>()];
                let _: () = msg_send![mascot as *mut AnyObject, setDelegate: std::ptr::null::<AnyObject>()];
            }
            self.mascot.set(std::ptr::null_mut());
        }
        for hotkey in self.hotkey_refs.borrow_mut().drain(..) {
            unsafe {
                UnregisterEventHotKey(hotkey);
            }
        }
        let handler = self.event_handler.get();
        if !handler.is_null() {
            unsafe {
                RemoveEventHandler(handler);
            }
            self.event_handler.set(std::ptr::null_mut());
        }
    }
}

fn configure_text_view(tv: &NSTextView, editable: bool, font: &NSFont) {
    tv.setEditable(editable);
    tv.setSelectable(true);
    tv.setRichText(false);
    tv.setImportsGraphics(false);
    tv.setUsesRuler(false);
    tv.setUsesFontPanel(false);
    tv.setFont(Some(font));
    tv.setAllowsUndo(editable);
    tv.setAutomaticQuoteSubstitutionEnabled(false);
    tv.setAutomaticDashSubstitutionEnabled(false);
    tv.setAutomaticTextReplacementEnabled(false);
    tv.setAutomaticSpellingCorrectionEnabled(false);
    tv.setAutomaticLinkDetectionEnabled(false);
    tv.setAutomaticDataDetectionEnabled(false);
    tv.setSmartInsertDeleteEnabled(false);
}

fn scroll_wrapper(
    mtm: MainThreadMarker,
    frame: NSRect,
    document: &NSTextView,
) -> Retained<NSScrollView> {
    let scroll: Retained<NSScrollView> =
        unsafe { msg_send![NSScrollView::alloc(mtm), initWithFrame: frame] };
    scroll.setHasVerticalScroller(true);
    scroll.setAutohidesScrollers(true);
    scroll.setBorderType(NSBorderType::BezelBorder);
    scroll.setDocumentView(Some(document));
    scroll
}

pub struct MascotIvars {
    ui: usize,
    image: Retained<NSImage>,
}

define_class!(
    #[unsafe(super = NSView)]
    #[thread_kind = MainThreadOnly]
    #[name = "MascotView"]
    #[ivars = MascotIvars]
    struct MascotView;

    unsafe impl NSObjectProtocol for MascotView {}

    impl MascotView {
        #[unsafe(method(hitTest:))]
        fn hit_test(&self, point: NSPoint) -> *mut NSView {
            let local = self.convertPoint_fromView(point, None);
            let bounds = self.bounds();
            if bounds.size.width <= 0.0 || bounds.size.height <= 0.0 {
                return std::ptr::null_mut();
            }
            let ui = unsafe { &*(self.ivars().ui as *const Ui) };
            let src_w = ui.mascot_src_w as usize;
            let src_h = ui.mascot_src_h as usize;
            let sx = ((local.x / bounds.size.width) * src_w as f64) as usize;
            let sy = (((bounds.size.height - local.y) / bounds.size.height) * src_h as f64) as usize;
            if local.x < 0.0 || local.y < 0.0 || sx >= src_w || sy >= src_h {
                return std::ptr::null_mut();
            }
            let alpha = ui.mascot_source[(sy * src_w + sx) * 4 + 3];
            if alpha > 0 {
                self as *const Self as *mut NSView
            } else {
                std::ptr::null_mut()
            }
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            let ui = unsafe { &*(self.ivars().ui as *const Ui) };
            let Some(window) = self.window() else {
                return;
            };
            let anchored = {
                let borrowed = ui.composer.borrow();
                borrowed.as_ref().is_some_and(|c| c.window.isVisible())
            };
            if !anchored {
                window.performWindowDragWithEvent(event);
                return;
            }
            // Anchored drag: while the mascot is perched on the composer, a
            // mascot drag moves the composer by the same delta and the
            // composer move re-perches the mascot through windowDidMove.
            let composer_start = {
                let borrowed = ui.composer.borrow();
                borrowed.as_ref().map(|c| c.window.frame())
            };
            let Some(composer_start) = composer_start else {
                window.performWindowDragWithEvent(event);
                return;
            };
            let start_frame = window.frame();
            let down = event.locationInWindow();
            let start = NSPoint::new(
                start_frame.origin.x + down.x,
                start_frame.origin.y + down.y,
            );
            let mask = NSEventMask::LeftMouseDragged | NSEventMask::LeftMouseUp;
            loop {
                let Some(next) = (unsafe {
                    window.nextEventMatchingMask_untilDate_inMode_dequeue(
                        mask,
                        None,
                        NSEventTrackingRunLoopMode,
                        true,
                    )
                }) else {
                    break;
                };
                if next.r#type() == NSEventType::LeftMouseUp {
                    break;
                }
                let local = next.locationInWindow();
                let current = window.frame();
                let now = NSPoint::new(current.origin.x + local.x, current.origin.y + local.y);
                let dx = now.x - start.x;
                let dy = now.y - start.y;
                let composer_origin = NSPoint::new(
                    composer_start.origin.x + dx,
                    composer_start.origin.y + dy,
                );
                {
                    let borrowed = ui.composer.borrow();
                    if let Some(composer) = borrowed.as_ref() {
                        composer.window.setFrameOrigin(composer_origin);
                    }
                }
                ui.perch_mascot();
            }
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: &NSEvent) -> bool {
            true
        }

        #[unsafe(method(isOpaque))]
        fn is_opaque(&self) -> bool {
            false
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let ui = unsafe { &*(self.ivars().ui as *const Ui) };
            ui.presents.set(ui.presents.get() + 1);
            unsafe {
                self.ivars().image.drawInRect(self.bounds());
            }
        }
    }
);

pub struct TextIvars {
    ui: usize,
}

define_class!(
    #[unsafe(super = NSTextView)]
    #[thread_kind = MainThreadOnly]
    #[name = "MascotInputTextView"]
    #[ivars = TextIvars]
    struct InputTextView;

    unsafe impl NSObjectProtocol for InputTextView {}

    impl InputTextView {
        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            let ui = unsafe { &*(self.ivars().ui as *const Ui) };
            let submit = event.keyCode() == K_VK_RETURN
                && event
                    .modifierFlags()
                    .contains(NSEventModifierFlags::Control);
            if submit && !self.hasMarkedText() && !ui.composing.get() {
                if let Err(error) = ui.submit() {
                    eprintln!("submit rejected: {error}");
                }
                return;
            }
            unsafe {
                let _: () = msg_send![super(self), keyDown: event];
            }
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, dirty: NSRect) {
            let ui = unsafe { &*(self.ivars().ui as *const Ui) };
            ui.count_paint();
            unsafe {
                let _: () = msg_send![super(self), drawRect: dirty];
            }
        }

        #[unsafe(method(setMarkedText:selectedRange:replacementRange:))]
        unsafe fn set_marked_text(
            &self,
            string: &AnyObject,
            selected_range: NSRange,
            replacement_range: NSRange,
        ) {
            unsafe {
                let _: () = msg_send![super(self),
                    setMarkedText: string,
                    selectedRange: selected_range,
                    replacementRange: replacement_range];
            }
            let ui = unsafe { &*(self.ivars().ui as *const Ui) };
            ui.on_marked_changed(self);
        }

        #[unsafe(method(insertText:replacementRange:))]
        unsafe fn insert_text(&self, string: &AnyObject, replacement_range: NSRange) {
            unsafe {
                let _: () = msg_send![super(self), insertText: string, replacementRange: replacement_range];
            }
            let ui = unsafe { &*(self.ivars().ui as *const Ui) };
            ui.composing.set(self.hasMarkedText());
        }

        #[unsafe(method(unmarkText))]
        fn unmark_text(&self) {
            unsafe {
                let _: () = msg_send![super(self), unmarkText];
            }
            let ui = unsafe { &*(self.ivars().ui as *const Ui) };
            ui.composing.set(false);
        }
    }
);

define_class!(
    #[unsafe(super = NSTextView)]
    #[thread_kind = MainThreadOnly]
    #[name = "MascotResponseTextView"]
    #[ivars = TextIvars]
    struct ResponseTextView;

    unsafe impl NSObjectProtocol for ResponseTextView {}

    impl ResponseTextView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, dirty: NSRect) {
            let ui = unsafe { &*(self.ivars().ui as *const Ui) };
            ui.count_paint();
            unsafe {
                let _: () = msg_send![super(self), drawRect: dirty];
            }
        }
    }
);

pub struct ComposerIvars {
    ui: usize,
}

define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[name = "MascotComposerDelegate"]
    #[ivars = ComposerIvars]
    struct ComposerDelegate;

    unsafe impl NSObjectProtocol for ComposerDelegate {}

    unsafe impl NSTextDelegate for ComposerDelegate {}

    unsafe impl NSWindowDelegate for ComposerDelegate {
        #[unsafe(method(windowShouldClose:))]
        fn window_should_close(&self, _sender: &NSWindow) -> bool {
            let ui = unsafe { &*(self.ivars().ui as *const Ui) };
            ui.hide_composer();
            false
        }

        #[unsafe(method(windowDidMove:))]
        fn window_did_move(&self, _notification: &NSNotification) {
            let ui = unsafe { &*(self.ivars().ui as *const Ui) };
            ui.perch_mascot();
        }

        #[unsafe(method(windowDidResize:))]
        fn window_did_resize(&self, _notification: &NSNotification) {
            let ui = unsafe { &*(self.ivars().ui as *const Ui) };
            ui.perch_mascot();
        }
    }

    unsafe impl NSTextViewDelegate for ComposerDelegate {
        #[unsafe(method(textView:shouldChangeTextInRange:replacementString:))]
        fn text_view_should_change_text(
            &self,
            _text_view: &NSTextView,
            affected_range: NSRange,
            replacement: Option<&NSString>,
        ) -> Bool {
            let ui = unsafe { &*(self.ivars().ui as *const Ui) };
            let borrowed = ui.composer.borrow();
            let Some(composer) = borrowed.as_ref() else {
                return Bool::YES;
            };
            let current = get_text(&composer.input);
            let replacement = replacement.map(|s| s.to_string()).unwrap_or_default();
            let mut units: Vec<u16> = current.encode_utf16().collect();
            let start = affected_range.location.min(units.len());
            let len = affected_range.length.min(units.len().saturating_sub(start));
            units.splice(start..start + len, replacement.encode_utf16()).for_each(drop);
            (units.len() <= ui.config.manifest.ui.input_limit_utf16_units as usize).into()
        }
    }

    impl ComposerDelegate {
        #[unsafe(method(sendAction:))]
        fn send_action(&self, _sender: &NSButton) {
            let ui = unsafe { &*(self.ivars().ui as *const Ui) };
            if let Err(error) = ui.submit() {
                eprintln!("submit rejected: {error}");
            }
        }

        #[unsafe(method(cancelAction:))]
        fn cancel_action(&self, _sender: &NSButton) {
            let ui = unsafe { &*(self.ivars().ui as *const Ui) };
            let _ = ui.cancel_request();
        }
    }
);

extern "C" fn hotkey_handler(
    _next: *mut c_void,
    event: *mut c_void,
    user_data: *mut c_void,
) -> i32 {
    let mut id = EventHotKeyID {
        signature: 0,
        id: 0,
    };
    unsafe {
        GetEventParameter(
            event,
            K_EVENT_PARAM_DIRECT_OBJECT,
            TYPE_EVENT_HOT_KEY_ID,
            std::ptr::null_mut(),
            std::mem::size_of::<EventHotKeyID>(),
            std::ptr::null_mut(),
            &mut id as *mut EventHotKeyID as *mut c_void,
        );
    }
    let ui = unsafe { &*(user_data as *const Ui) };
    if id.id == HOTKEY_TOGGLE_ID {
        let visible = {
            let borrowed = ui.composer.borrow();
            borrowed
                .as_ref()
                .map(|c| c.window.isVisible())
                .unwrap_or(false)
        };
        if visible {
            ui.hide_composer();
        } else if let Some(mtm) = MainThreadMarker::new() {
            if let Err(error) = ui.show_composer(mtm) {
                eprintln!("show failed: {error}");
            }
        }
    } else if id.id == HOTKEY_CANCEL_ID {
        let _ = ui.cancel_request();
    }
    0
}

fn build_mascot_image(rgba: &[u8], w: u32, h: u32) -> Result<Retained<NSImage>, String> {
    let rep: Retained<NSBitmapImageRep> = unsafe {
        msg_send![
            NSBitmapImageRep::alloc(),
            initWithBitmapDataPlanes: std::ptr::null_mut::<*mut u8>(),
            pixelsWide: w as isize,
            pixelsHigh: h as isize,
            bitsPerSample: 8_isize,
            samplesPerPixel: 4_isize,
            hasAlpha: true,
            isPlanar: false,
            colorSpaceName: &*NSString::from_str("NSCalibratedRGBColorSpace"),
            bytesPerRow: (w * 4) as isize,
            bitsPerPixel: 32_isize
        ]
    };
    let data = rep.bitmapData();
    if data.is_null() {
        return Err("NSBitmapImageRep bitmap data missing".into());
    }
    unsafe {
        std::ptr::copy_nonoverlapping(rgba.as_ptr(), data, rgba.len());
    }
    let image = unsafe { NSImage::initWithSize(NSImage::alloc(), NSSize::new(64.0, 64.0)) };
    image.addRepresentation(&rep);
    Ok(image)
}

pub fn run_app(manifest: &str, control: bool) -> Result<i32, String> {
    autoreleasepool(|_pool| run_app_inner(manifest, control))
}

fn run_app_inner(manifest: &str, control: bool) -> Result<i32, String> {
    let Some(mtm) = MainThreadMarker::new() else {
        return Err("main thread marker unavailable".into());
    };
    let config = crate::config::load(manifest)?;
    let (mascot_source, mascot_src_w, mascot_src_h) = decode_png(&config.asset_path)?;
    if mascot_src_w != config.manifest.asset.pixel_width
        || mascot_src_h != config.manifest.asset.pixel_height
    {
        return Err("decoded asset dimensions differ from the manifest".into());
    }
    let image = build_mascot_image(&mascot_source, mascot_src_w, mascot_src_h)?;

    let ui_events = Arc::new(BoundedQueue::<crate::provider::Event>::new(64));
    let commands = Arc::new(BoundedQueue::<serde_json::Value>::new(16));
    let records = if control {
        Some(Arc::new(BoundedQueue::<String>::new(256)))
    } else {
        None
    };

    let ui = Box::leak(Box::new(Ui {
        config,
        mascot: Cell::new(std::ptr::null_mut()),
        mascot_source,
        mascot_src_w,
        mascot_src_h,
        composer: RefCell::new(None),
        model: RefCell::new(Model {
            provider_state: "idle".into(),
            request_id: 0,
            request_count: 0,
            last_seq: -1,
            response: String::new(),
            provider_pid: 0,
            scenario: "normal".into(),
            run_invalid: None,
            provider_error: None,
            generation: 0,
        }),
        composing: Cell::new(false),
        snapshot: RefCell::new(None),
        ui_events: Arc::clone(&ui_events),
        commands: Arc::clone(&commands),
        records: records.clone(),
        provider: Mutex::new(None),
        control_enabled: control,
        shutdown_started: Cell::new(false),
        cancel_pending: Cell::new(false),
        pending_shutdown_token: RefCell::new(None),
        presents: Cell::new(0),
        paints: Cell::new(0),
        hotkey_refs: RefCell::new(Vec::new()),
        event_handler: Cell::new(std::ptr::null_mut()),
    }));

    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    let dip = ui.config.manifest.asset.logical_width_dip as f64;
    let mouse = NSEvent::mouseLocation();
    let mut x = mouse.x + 16.0;
    let mut y = mouse.y - 16.0 - dip;
    if let Some(screen) = NSScreen::mainScreen(mtm) {
        let visible = screen.visibleFrame();
        x = x.clamp(visible.origin.x, (visible.origin.x + visible.size.width - dip).max(visible.origin.x));
        y = y.clamp(visible.origin.y, (visible.origin.y + visible.size.height - dip).max(visible.origin.y));
    }

    let panel: Retained<NSPanel> = unsafe {
        msg_send![
            NSPanel::alloc(mtm),
            initWithContentRect: NSRect::new(NSPoint::new(x, y), NSSize::new(dip, dip)),
            styleMask: NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel,
            backing: NSBackingStoreType::Buffered,
            defer: false
        ]
    };
    panel.setOpaque(false);
    panel.setBackgroundColor(Some(&NSColor::clearColor()));
    panel.setHasShadow(false);
    panel.setLevel(NSFloatingWindowLevel);
    panel.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::Stationary
            | NSWindowCollectionBehavior::IgnoresCycle
            | NSWindowCollectionBehavior::FullScreenAuxiliary,
    );
    unsafe { panel.setReleasedWhenClosed(false) };
    panel.setHidesOnDeactivate(false);

    let view: Retained<MascotView> = unsafe {
        let this = MascotView::alloc(mtm).set_ivars(MascotIvars {
            ui: ui as *const Ui as usize,
            image: Retained::clone(&image),
        });
        msg_send![super(this), initWithFrame: NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(dip, dip))]
    };
    panel.setContentView(Some(&view));
    ui.mascot.set(Retained::as_ptr(&panel) as *mut c_void);
    std::mem::forget(view);
    panel.orderFrontRegardless();
    std::mem::forget(panel);

    let types = [EventTypeSpec {
        event_class: K_EVENT_CLASS_KEYBOARD,
        event_kind: K_EVENT_HOT_KEY_PRESSED,
    }];
    let mut handler = std::ptr::null_mut();
    unsafe {
        if InstallEventHandler(
            GetApplicationEventTarget(),
            hotkey_handler,
            types.len(),
            types.as_ptr(),
            ui as *const Ui as *mut c_void,
            &mut handler,
        ) != 0
        {
            return Err("hotkey event handler install failed".into());
        }
        ui.event_handler.set(handler);
        let mut toggle_ref = std::ptr::null_mut();
        if RegisterEventHotKey(
            K_VK_SPACE,
            CONTROL_KEY | OPTION_KEY,
            EventHotKeyID {
                signature: u32::from_be_bytes(*b"MSCT"),
                id: HOTKEY_TOGGLE_ID,
            },
            GetApplicationEventTarget(),
            0,
            &mut toggle_ref,
        ) != 0
        {
            return Err("hotkey registration failed".into());
        }
        ui.hotkey_refs.borrow_mut().push(toggle_ref);
        let mut cancel_ref = std::ptr::null_mut();
        if RegisterEventHotKey(
            K_VK_ESCAPE,
            CONTROL_KEY | OPTION_KEY,
            EventHotKeyID {
                signature: u32::from_be_bytes(*b"MSCT"),
                id: HOTKEY_CANCEL_ID,
            },
            GetApplicationEventTarget(),
            0,
            &mut cancel_ref,
        ) != 0
        {
            return Err("cancel hotkey registration failed".into());
        }
        ui.hotkey_refs.borrow_mut().push(cancel_ref);
    }

    let mut scenario_chunks = std::collections::HashMap::new();
    for name in ui.config.manifest.scenarios.keys() {
        if let Ok(chunks) = crate::config::scenario_chunks(&ui.config.manifest, name) {
            scenario_chunks.insert(name.clone(), chunks);
        }
    }
    let provider_config = crate::provider::ProviderConfig {
        path: ui.config.manifest.provider.path.clone(),
        arguments: ui.config.manifest.provider.arguments.clone(),
        cwd: ui.config.manifest.provider.cwd.clone(),
        environment: ui
            .config
            .manifest
            .provider
            .environment
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
        scenario_chunks,
        cancel_timeout_ms: ui.config.manifest.protocol.cancel_timeout_ms,
        shutdown_timeout_ms: ui.config.manifest.protocol.shutdown_timeout_ms,
    };
    let provider = crate::provider::Provider::spawn(
        provider_config,
        ui_events,
        records.clone(),
        ui.ui_waker(),
    );
    *ui.provider.lock().unwrap_or_else(|e| e.into_inner()) = Some(provider);

    let mut control_state = crate::control::Control::new();
    control_state.arm(ui, control);

    app.run();

    control_state.stop();
    ui.teardown();
    Ok(0)
}

/// Decode the asset PNG into straight (non-premultiplied) RGBA plus its pixel
/// dimensions. NSBitmapImageRep is created with bitmapFormat=0, which treats
/// the data as non-premultiplied.
pub fn decode_png(path: &std::path::Path) -> Result<(Vec<u8>, u32, u32), String> {
    let file = std::fs::File::open(path).map_err(|e| format!("open asset: {e}"))?;
    let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().map_err(|e| format!("png info: {e}"))?;
    let mut buffer = vec![0u8; reader.output_buffer_size().ok_or("png size unknown")?];
    let info = reader
        .next_frame(&mut buffer)
        .map_err(|e| format!("png frame: {e}"))?;
    Ok((buffer[..info.buffer_size()].to_vec(), info.width, info.height))
}

#[cfg(test)]
mod tests {
    use super::{append_bounded, event_matches};

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
