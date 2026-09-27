#![cfg_attr(not(test), windows_subsystem = "windows")]

mod config;
mod control;
mod framing;
mod platform;
mod provider;
mod queue;
mod text;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::HiDpi::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use platform::{Ui, WM_APP_CONTROL, WM_APP_PROVIDER, WM_APP_SUBMIT};

#[derive(Deserialize)]
struct Vector {
    name: String,
    fragments_base64: Vec<String>,
}

fn decode_vectors(path: &str) -> Result<i32, String> {
    let data = std::fs::read(path).map_err(|e| format!("read vectors: {e}"))?;
    let vectors: Vec<Vector> =
        serde_json::from_slice(&data).map_err(|e| format!("parse vectors: {e}"))?;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    use std::io::Write;
    for vector in &vectors {
        let mut decoder = framing::Decoder::new();
        let mut frames: Vec<Vec<u8>> = Vec::new();
        let mut rejected = false;
        for fragment in &vector.fragments_base64 {
            let bytes = BASE64
                .decode(fragment)
                .map_err(|e| format!("vector {} fragment base64: {e}", vector.name))?;
            let collected = &mut frames;
            if decoder
                .feed(&bytes, provider::qpc, |frame, _| {
                    collected.push(frame.to_vec());
                    Ok(())
                })
                .is_err()
            {
                rejected = true;
                break;
            }
        }
        if !rejected && decoder.finish().is_err() {
            rejected = true;
        }
        let encoded: Vec<String> = frames.iter().map(|f| BASE64.encode(f)).collect();
        let frames_field = if encoded.is_empty() {
            serde_json::Value::Null
        } else {
            json!(encoded)
        };
        let row = json!({
            "name": vector.name,
            "frames_base64": frames_field,
            "rejected": rejected,
            "peak_buffer_bytes": decoder.peak_buffer_bytes(),
        });
        writeln!(out, "{row}").map_err(|e| format!("stdout: {e}"))?;
    }
    out.flush().map_err(|e| format!("stdout: {e}"))?;
    Ok(0)
}

fn set_dpi_awareness() -> Result<(), String> {
    unsafe {
        if SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) == 0 {
            let current = GetThreadDpiAwarenessContext();
            if AreDpiAwarenessContextsEqual(current, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)
                == 0
            {
                return Err("per-monitor-v2 awareness not in effect".into());
            }
        }
    }
    Ok(())
}

fn run_app(manifest: &str, control: bool) -> Result<i32, String> {
    set_dpi_awareness()?;
    let config = config::load(manifest)?;
    let mascot_source = platform::decode_png(&config.asset_path)?;

    let ui_events = Arc::new(queue::BoundedQueue::<provider::Event>::new(64));
    let commands = Arc::new(queue::BoundedQueue::<serde_json::Value>::new(16));
    let records = Arc::new(queue::BoundedQueue::<String>::new(256));

    let ui = Box::leak(Box::new(Ui {
        config,
        thread_id: std::cell::Cell::new(unsafe { GetCurrentThreadId() }),
        mascot: std::cell::Cell::new(std::ptr::null_mut()),
        mascot_source,
        mascot_bits: std::cell::RefCell::new(Vec::new()),
        composer: std::cell::RefCell::new(None),
        model: std::cell::RefCell::new(platform::Model {
            provider_state: "idle".into(),
            request_id: 0,
            request_count: 0,
            last_seq: -1,
            response: String::new(),
            provider_pid: 0,
            scenario: "normal".into(),
            run_invalid: None,
        }),
        composing: std::cell::Cell::new(false),
        snapshot: std::cell::RefCell::new(None),
        ui_events: Arc::clone(&ui_events),
        commands: Arc::clone(&commands),
        records: Arc::clone(&records),
        provider: std::sync::Mutex::new(None),
        shutdown_started: std::cell::Cell::new(false),
        presents: std::cell::Cell::new(0),
        paints: std::cell::Cell::new(0),
    }));

    let instance = text::instance();
    platform::register_classes(instance)?;
    let mascot = platform::create_mascot(ui, instance)?;
    ui.mascot.set(mascot);
    let dpi = unsafe { GetDpiForWindow(mascot) };
    ui.present_mascot(dpi);
    unsafe {
        ShowWindow(mascot, SW_SHOWNOACTIVATE);
    }

    unsafe {
        if RegisterHotKey(
            std::ptr::null_mut(),
            platform::HOTKEY_TOGGLE_ID,
            MOD_NOREPEAT | MOD_CONTROL | MOD_ALT,
            VK_SPACE as u32,
        ) == 0
        {
            return Err("hotkey registration failed".into());
        }
        if RegisterHotKey(
            std::ptr::null_mut(),
            platform::HOTKEY_CANCEL_ID,
            MOD_NOREPEAT | MOD_CONTROL | MOD_ALT,
            VK_ESCAPE as u32,
        ) == 0
        {
            return Err("cancel hotkey registration failed".into());
        }
    }

    let mut scenario_chunks = std::collections::HashMap::new();
    for name in ui.config.manifest.scenarios.keys() {
        if let Ok(chunks) = config::scenario_chunks(&ui.config.manifest, name) {
            scenario_chunks.insert(name.clone(), chunks);
        }
    }
    let provider_config = provider::ProviderConfig {
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
    let wake_ui: Arc<dyn Fn() + Send + Sync> = {
        let tid = ui.thread_id.get();
        Arc::new(move || unsafe {
            PostThreadMessageW(tid, WM_APP_PROVIDER, 0, 0);
        })
    };
    let provider =
        provider::Provider::spawn(provider_config, ui_events, Arc::clone(&records), wake_ui);
    *ui.provider.lock().unwrap_or_else(|e| e.into_inner()) = Some(provider);

    let mut control_state = control::Control::new();
    control_state.arm(ui, control);

    let mut message = MSG::default();
    unsafe {
        while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 {
            if message.hwnd.is_null() {
                match message.message {
                    WM_APP_PROVIDER => ui.dispatch_provider_events(),
                    WM_APP_CONTROL => ui.dispatch_control(),
                    WM_APP_SUBMIT => {
                        let _ = ui.submit();
                    }
                    WM_HOTKEY => {
                        if message.wParam as i32 == platform::HOTKEY_TOGGLE_ID {
                            let visible = ui
                                .composer
                                .borrow()
                                .as_ref()
                                .map(|c| IsWindowVisible(c.hwnd) != 0)
                                .unwrap_or(false);
                            if visible {
                                ui.hide_composer();
                            } else {
                                let _ = ui.show_composer();
                            }
                        } else if message.wParam as i32 == platform::HOTKEY_CANCEL_ID {
                            ui.cancel_request();
                        }
                    }
                    _ => {}
                }
            }
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }

    control_state.stop();
    ui.teardown();
    unsafe {
        UnregisterHotKey(std::ptr::null_mut(), platform::HOTKEY_TOGGLE_ID);
        UnregisterHotKey(std::ptr::null_mut(), platform::HOTKEY_CANCEL_ID);
    }
    Ok(0)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let code = match args.get(1).map(String::as_str) {
        Some("--decode-vectors") => match args.get(2) {
            Some(path) => decode_vectors(path),
            None => Err("missing vectors path".into()),
        },
        Some("--fixture") => {
            let control = args.iter().any(|a| a == "--control");
            match args.get(2) {
                Some(path) => run_app(path, control),
                None => Err("missing manifest path".into()),
            }
        }
        _ => Err("usage: mascot --fixture MANIFEST [--control] | --decode-vectors VECTORS".into()),
    };
    match code {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(64);
        }
    }
}
