use crate::platform::{Ui, WM_APP_CONTROL};
use crate::queue::BoundedQueue;
use serde_json::{Value, json};
use std::io::{BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use windows_sys::Win32::Foundation::CloseHandle;
use windows_sys::Win32::System::IO::CancelSynchronousIo;
use windows_sys::Win32::System::Threading::{
    GetCurrentThreadId, OpenThread, THREAD_SYNCHRONIZE, THREAD_TERMINATE, WaitForSingleObject,
};
use windows_sys::Win32::UI::WindowsAndMessaging::PostThreadMessageW;

const MAX_COMMAND_BYTES: usize = 65536;

pub struct Control {
    reader: Option<JoinHandle<()>>,
    writer: Option<JoinHandle<()>>,
    reader_handle: Arc<Mutex<usize>>,
    stop: Arc<AtomicBool>,
}

pub fn start_writer(records: Arc<BoundedQueue<String>>) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let stdout = std::io::stdout();
        let mut lock = stdout.lock();
        while let Some(line) = records.wait_pop() {
            if lock.write_all(line.as_bytes()).is_err()
                || lock.write_all(b"\n").is_err()
                || lock.flush().is_err()
            {
                return;
            }
        }
    })
}

pub fn start_reader(
    thread_id: u32,
    commands: Arc<BoundedQueue<Value>>,
    records: Arc<BoundedQueue<String>>,
    stop: Arc<AtomicBool>,
    handle_store: Arc<Mutex<usize>>,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        unsafe {
            let handle = OpenThread(
                THREAD_TERMINATE | THREAD_SYNCHRONIZE,
                0,
                GetCurrentThreadId(),
            );
            *handle_store.lock().unwrap_or_else(|e| e.into_inner()) = handle as usize;
        }
        let stdin = std::io::stdin();
        let mut lock = stdin.lock();
        loop {
            if stop.load(Ordering::SeqCst) {
                break;
            }
            let mut line = String::new();
            match lock.read_line(&mut line) {
                Ok(0) | Err(_) => {
                    let _ = commands.push(json!({"command": "shutdown", "token": null}));
                    unsafe {
                        PostThreadMessageW(thread_id, WM_APP_CONTROL, 0, 0);
                    }
                    break;
                }
                Ok(_) => {
                    if line.len() > MAX_COMMAND_BYTES {
                        let _ = records.try_push(
                            json!({"token": null, "ok": false, "error": "command frame exceeds limit"})
                                .to_string(),
                        );
                        continue;
                    }
                    match serde_json::from_str::<Value>(&line) {
                        Ok(value) => {
                            if commands.push(value).is_err() {
                                break;
                            }
                            unsafe {
                                PostThreadMessageW(thread_id, WM_APP_CONTROL, 0, 0);
                            }
                        }
                        Err(error) => {
                            let _ = records.try_push(
                                json!({"token": null, "ok": false, "error": format!("invalid command JSON: {error}")})
                                    .to_string(),
                            );
                        }
                    }
                }
            }
        }
        unsafe {
            let handle = *handle_store.lock().unwrap_or_else(|e| e.into_inner());
            if handle != 0 {
                CloseHandle(handle as *mut std::ffi::c_void);
            }
        }
    })
}

impl Control {
    pub fn new() -> Self {
        Self {
            reader: None,
            writer: None,
            reader_handle: Arc::new(Mutex::new(0)),
            stop: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn arm(&mut self, ui: &'static Ui, control: bool) {
        self.writer = Some(start_writer(Arc::clone(&ui.records)));
        if control {
            self.reader = Some(start_reader(
                ui.thread_id.get(),
                Arc::clone(&ui.commands),
                Arc::clone(&ui.records),
                Arc::clone(&self.stop),
                Arc::clone(&self.reader_handle),
            ));
        }
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(reader) = self.reader.take() {
            unsafe {
                let handle = *self.reader_handle.lock().unwrap_or_else(|e| e.into_inner())
                    as *mut std::ffi::c_void;
                if !handle.is_null() {
                    CancelSynchronousIo(handle);
                    WaitForSingleObject(handle, 2000);
                }
            }
            let _ = reader.join();
        }
        if let Some(writer) = self.writer.take() {
            let _ = writer.join();
        }
    }
}
