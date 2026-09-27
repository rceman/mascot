use crate::framing::Decoder;
use crate::platform::{Ui, WM_APP_CONTROL};
use crate::queue::BoundedQueue;
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::os::windows::io::AsRawHandle;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{HANDLE, WAIT_OBJECT_0};
use windows_sys::Win32::System::IO::CancelSynchronousIo;
use windows_sys::Win32::System::Threading::WaitForSingleObject;
use windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW;

fn push_record(records: &Option<Arc<BoundedQueue<String>>>, record: String) {
    if let Some(records) = records {
        let _ = records.try_push(record);
    }
}

fn post_control(hwnd: usize) {
    unsafe {
        PostMessageW(hwnd as *mut std::ffi::c_void, WM_APP_CONTROL, 0, 0);
    }
}

fn request_shutdown(commands: &Arc<BoundedQueue<Value>>, hwnd: usize) {
    let _ = commands.push(json!({"command": "shutdown", "token": null}));
    post_control(hwnd);
}

fn reader_run(
    hwnd: usize,
    commands: Arc<BoundedQueue<Value>>,
    records: Option<Arc<BoundedQueue<String>>>,
    stop: Arc<AtomicBool>,
) {
    let mut decoder = Decoder::new();
    let mut buffer = [0u8; 4096];
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    loop {
        if stop.load(Ordering::Acquire) {
            break;
        }
        match input.read(&mut buffer) {
            Ok(0) => {
                if decoder.finish().is_err() {
                    push_record(
                        &records,
                        json!({"token": null, "ok": false, "error": "incomplete control frame"})
                            .to_string(),
                    );
                }
                request_shutdown(&commands, hwnd);
                break;
            }
            Ok(count) => {
                let result = {
                    let commands = &commands;
                    decoder.feed(
                        &buffer[..count],
                        || 0,
                        |frame, _| {
                            let value: Value =
                                serde_json::from_slice(frame).map_err(|e| e.to_string())?;
                            commands
                                .push(value)
                                .map_err(|_| "control queue closed".to_string())?;
                            post_control(hwnd);
                            Ok(())
                        },
                    )
                };
                if let Err(error) = result {
                    push_record(
                        &records,
                        json!({"token": null, "ok": false, "error": format!("invalid control frame: {error}")})
                            .to_string(),
                    );
                    request_shutdown(&commands, hwnd);
                    break;
                }
            }
            Err(_) => {
                if stop.load(Ordering::Acquire) {
                    break;
                }
                push_record(
                    &records,
                    json!({"token": null, "ok": false, "error": "control input read failed"})
                        .to_string(),
                );
                request_shutdown(&commands, hwnd);
                break;
            }
        }
    }
}

struct Thread {
    handle: JoinHandle<()>,
}

impl Thread {
    fn spawn<F>(body: F) -> Self
    where
        F: FnOnce() + Send + 'static,
    {
        Thread {
            handle: std::thread::spawn(body),
        }
    }

    fn join_bounded(self, deadline: Instant, cancel: bool) -> bool {
        let handle = self.handle;
        let raw = handle.as_raw_handle() as HANDLE;
        unsafe {
            loop {
                if handle.is_finished() {
                    let _ = handle.join();
                    return true;
                }
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return false;
                }
                let wait = remaining.min(Duration::from_millis(20)).as_millis() as u32;
                if WaitForSingleObject(raw, wait) == WAIT_OBJECT_0 {
                    let _ = handle.join();
                    return true;
                }
                if cancel {
                    CancelSynchronousIo(raw);
                }
            }
        }
    }
}

pub struct Control {
    reader: Option<Thread>,
    writer: Option<Thread>,
    commands: Option<Arc<BoundedQueue<Value>>>,
    stop: Arc<AtomicBool>,
}

impl Control {
    pub fn new() -> Self {
        Self {
            reader: None,
            writer: None,
            commands: None,
            stop: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn arm(&mut self, ui: &'static Ui, control: bool) {
        if !control {
            return;
        }
        let Some(records) = ui.records.as_ref().cloned() else {
            return;
        };
        self.commands = Some(Arc::clone(&ui.commands));
        let writer_records = Arc::clone(&records);
        self.writer = Some(Thread::spawn(move || {
            publish_writer(writer_records);
        }));
        let commands = Arc::clone(&ui.commands);
        let stop = Arc::clone(&self.stop);
        let hwnd = ui.mascot.get() as usize;
        self.reader = Some(Thread::spawn(move || {
            reader_run(hwnd, commands, Some(records), stop);
        }));
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(commands) = &self.commands {
            commands.close();
        }
        let deadline = Instant::now() + Duration::from_millis(2000);
        if let Some(writer) = self.writer.take() {
            if !writer.join_bounded(deadline, true) {
                eprintln!("control writer did not finish within teardown bound");
            }
        }
        if let Some(reader) = self.reader.take() {
            if !reader.join_bounded(deadline, true) {
                eprintln!("control reader did not finish within teardown bound");
            }
        }
    }
}

fn publish_writer(records: Arc<BoundedQueue<String>>) {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_framer_rejects_oversized_and_incomplete() {
        let commands = std::cell::Cell::new(0usize);
        let parsed = |frame: &[u8], _: i64| -> Result<(), String> {
            let _: Value = serde_json::from_slice(frame).map_err(|e| e.to_string())?;
            commands.set(commands.get() + 1);
            Ok(())
        };
        let mut decoder = Decoder::new();
        let oversized = vec![b'x'; crate::framing::FRAME_LIMIT + 64];
        assert!(decoder.feed(&oversized, || 0, &parsed).is_err());
        assert!(decoder.peak_buffer_bytes() <= 65536);
        assert_eq!(commands.get(), 0);
        assert!(decoder.finish().is_err());

        let mut decoder = Decoder::new();
        decoder
            .feed(b"{\"command\":\"state\",\"tok", || 0, &parsed)
            .unwrap();
        assert!(decoder.finish().is_err());
        assert!(decoder.peak_buffer_bytes() <= 65536);
        assert_eq!(commands.get(), 0);
    }
}
