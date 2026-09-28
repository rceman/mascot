use crate::framing::Decoder;
use crate::platform::Ui;
use crate::queue::BoundedQueue;
use serde_json::{Value, json};
use std::io::Write;
use std::sync::Arc;
#[cfg(windows)]
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
#[cfg(windows)]
use std::io::Read;
#[cfg(windows)]
use std::os::windows::io::AsRawHandle;
#[cfg(windows)]
use windows_sys::Win32::Foundation::{HANDLE, WAIT_OBJECT_0};
#[cfg(windows)]
use windows_sys::Win32::System::IO::CancelSynchronousIo;
#[cfg(windows)]
use windows_sys::Win32::System::Threading::WaitForSingleObject;

fn push_record(records: &Option<Arc<BoundedQueue<String>>>, record: String) {
    if let Some(records) = records {
        let _ = records.try_push(record);
    }
}

fn request_shutdown(commands: &Arc<BoundedQueue<Value>>, wake: &Arc<dyn Fn() + Send + Sync>) {
    let _ = commands.push(json!({"command": "shutdown", "token": null}));
    wake();
}

#[cfg(windows)]
fn reader_run(
    wake: Arc<dyn Fn() + Send + Sync>,
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
                request_shutdown(&commands, &wake);
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
                            wake();
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
                    request_shutdown(&commands, &wake);
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
                request_shutdown(&commands, &wake);
                break;
            }
        }
    }
}

#[cfg(unix)]
fn reader_run(
    wake: Arc<dyn Fn() + Send + Sync>,
    commands: Arc<BoundedQueue<Value>>,
    records: Option<Arc<BoundedQueue<String>>>,
    stop_read: std::os::fd::RawFd,
) {
    use std::os::fd::RawFd;
    let mut decoder = Decoder::new();
    let mut buffer = [0u8; 4096];
    loop {
        let mut fds = [
            libc::pollfd {
                fd: 0 as RawFd,
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: stop_read,
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        let ready = unsafe { libc::poll(fds.as_mut_ptr(), 2, -1) };
        if ready < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            push_record(
                &records,
                json!({"token": null, "ok": false, "error": format!("control poll failed: {error}")})
                    .to_string(),
            );
            request_shutdown(&commands, &wake);
            break;
        }
        if fds[1].revents != 0 {
            break;
        }
        if fds[0].revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) == 0 {
            continue;
        }
        let count = unsafe {
            libc::read(
                0,
                buffer.as_mut_ptr() as *mut std::ffi::c_void,
                buffer.len(),
            )
        };
        if count == 0 {
            if decoder.finish().is_err() {
                push_record(
                    &records,
                    json!({"token": null, "ok": false, "error": "incomplete control frame"})
                        .to_string(),
                );
            }
            request_shutdown(&commands, &wake);
            break;
        }
        if count < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            push_record(
                &records,
                json!({"token": null, "ok": false, "error": "control input read failed"})
                    .to_string(),
            );
            request_shutdown(&commands, &wake);
            break;
        }
        let result = {
            let commands = &commands;
            decoder.feed(
                &buffer[..count as usize],
                || 0,
                |frame, _| {
                    let value: Value =
                        serde_json::from_slice(frame).map_err(|e| e.to_string())?;
                    commands
                        .push(value)
                        .map_err(|_| "control queue closed".to_string())?;
                    wake();
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
            request_shutdown(&commands, &wake);
            break;
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

    #[cfg(windows)]
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

    #[cfg(unix)]
    fn join_bounded(self, deadline: Instant, _cancel: bool) -> bool {
        let handle = self.handle;
        while !handle.is_finished() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        if handle.is_finished() {
            let _ = handle.join();
            true
        } else {
            false
        }
    }
}

pub struct Control {
    reader: Option<Thread>,
    writer: Option<Thread>,
    commands: Option<Arc<BoundedQueue<Value>>>,
    #[cfg(windows)]
    stop: Arc<AtomicBool>,
    #[cfg(unix)]
    stop_write: std::os::fd::RawFd,
}

impl Control {
    pub fn new() -> Self {
        Self {
            reader: None,
            writer: None,
            commands: None,
            #[cfg(windows)]
            stop: Arc::new(AtomicBool::new(false)),
            #[cfg(unix)]
            stop_write: -1,
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
        let wake = ui.control_waker();
        #[cfg(windows)]
        {
            let stop = Arc::clone(&self.stop);
            self.reader = Some(Thread::spawn(move || {
                reader_run(wake, commands, Some(records), stop);
            }));
        }
        #[cfg(unix)]
        {
            let mut fds = [0i32; 2];
            if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
                eprintln!("control stop pipe creation failed");
                return;
            }
            let (read_fd, write_fd) = (fds[0], fds[1]);
            self.stop_write = write_fd;
            self.reader = Some(Thread::spawn(move || {
                reader_run(wake, commands, Some(records), read_fd);
                unsafe { libc::close(read_fd) };
            }));
        }
    }

    pub fn stop(&mut self) {
        #[cfg(windows)]
        self.stop.store(true, Ordering::Release);
        #[cfg(unix)]
        if self.stop_write >= 0 {
            let byte = [1u8; 1];
            unsafe {
                libc::write(
                    self.stop_write,
                    byte.as_ptr() as *const std::ffi::c_void,
                    1,
                );
                libc::close(self.stop_write);
            }
            self.stop_write = -1;
        }
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
