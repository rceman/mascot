use crate::framing::Decoder;
use crate::queue::BoundedQueue;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::Read;
use std::io::Write;
use std::os::windows::process::CommandExt;
use std::process::{Child, Command as ProcessCommand, Stdio};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;
use windows_sys::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
use windows_sys::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
use windows_sys::Win32::System::Threading::{
    OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject,
};

pub fn qpc() -> i64 {
    let mut value = 0i64;
    if unsafe { QueryPerformanceCounter(&mut value) } == 0 {
        panic!("QueryPerformanceCounter failed");
    }
    value
}

pub fn qpc_frequency() -> i64 {
    let mut value = 0i64;
    if unsafe { QueryPerformanceFrequency(&mut value) } == 0 || value <= 0 {
        panic!("QueryPerformanceFrequency failed");
    }
    value
}

#[derive(Debug)]
pub enum Command {
    Request {
        id: u64,
        prompt: String,
        scenario: String,
    },
    Cancel {
        id: u64,
    },
    Shutdown,
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum Event {
    Started {
        generation: u64,
        pid: u32,
    },
    Chunk {
        generation: u64,
        id: u64,
        seq: u32,
        text: String,
        receipt_qpc: i64,
        emit_qpc: String,
    },
    Terminal {
        generation: u64,
        id: u64,
        kind: String,
        last_seq: i64,
    },
    SessionClosed {
        generation: u64,
        exit_code: Option<u32>,
        error: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Phase {
    AwaitStart,
    Streaming,
    Terminal,
}

#[derive(Debug)]
pub struct RequestProtocol {
    pub id: u64,
    pub expected_chunks: usize,
    pub next_seq: usize,
    pub phase: Phase,
    pub cancel_requested: bool,
    pub client_request_seen: bool,
}

#[derive(Debug)]
enum Work {
    Run(Command),
    ClientResponse { request_id: u64 },
    SessionFailed { generation: u64, error: String },
    SessionEof { generation: u64 },
}

struct SharedState {
    generation: u64,
    protocol: Option<RequestProtocol>,
    eof: bool,
    shutdown_sent: bool,
    shutdown_ack: bool,
    failed: Option<String>,
    stop: bool,
}

type Shared = Arc<(Mutex<SharedState>, Condvar)>;

fn shared_new() -> Shared {
    Arc::new((
        Mutex::new(SharedState {
            generation: 0,
            protocol: None,
            eof: false,
            shutdown_sent: false,
            shutdown_ack: false,
            failed: None,
            stop: false,
        }),
        Condvar::new(),
    ))
}

fn lock(shared: &Shared) -> std::sync::MutexGuard<'_, SharedState> {
    shared.0.lock().unwrap_or_else(|e| e.into_inner())
}

fn notify(shared: &Shared) {
    shared.1.notify_all();
}

#[derive(Default)]
pub struct StderrTail {
    pub tail: std::collections::VecDeque<u8>,
    pub total: u64,
}

pub struct ProviderConfig {
    pub path: String,
    pub arguments: Vec<String>,
    pub cwd: String,
    pub environment: Vec<(String, String)>,
    pub scenario_chunks: HashMap<String, u64>,
    pub cancel_timeout_ms: u64,
    pub shutdown_timeout_ms: u64,
}

pub struct Provider {
    work: Arc<BoundedQueue<Work>>,
    shared: Shared,
    stderr_tail: Arc<Mutex<StderrTail>>,
    threads: Mutex<Vec<JoinHandle<()>>>,
}

impl Provider {
    pub fn spawn(
        config: ProviderConfig,
        ui_events: Arc<BoundedQueue<Event>>,
        records: Arc<BoundedQueue<String>>,
        wake_ui: Arc<dyn Fn() + Send + Sync>,
    ) -> Arc<Self> {
        let shared = shared_new();
        let work = Arc::new(BoundedQueue::<Work>::new(16));
        let stderr_tail = Arc::new(Mutex::new(StderrTail::default()));
        let provider = Arc::new(Self {
            work: Arc::clone(&work),
            shared: Arc::clone(&shared),
            stderr_tail: Arc::clone(&stderr_tail),
            threads: Mutex::new(Vec::new()),
        });
        let coordinator_shared = Arc::clone(&shared);
        let coordinator_tail = Arc::clone(&stderr_tail);
        let coordinator = std::thread::spawn(move || {
            coordinator_run(
                config,
                work,
                coordinator_shared,
                ui_events,
                records,
                coordinator_tail,
                wake_ui,
            );
        });
        provider
            .threads
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(coordinator);
        provider
    }

    pub fn send(&self, command: Command) -> Result<(), String> {
        self.work
            .push(Work::Run(command))
            .map_err(|_| "provider command queue closed".to_string())
    }

    pub fn stderr_total(&self) -> u64 {
        self.stderr_tail
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .total
    }

    pub fn stop(&self) {
        {
            let mut state = lock(&self.shared);
            state.stop = true;
        }
        notify(&self.shared);
        self.work.close();
        let mut threads = self.threads.lock().unwrap_or_else(|e| e.into_inner());
        for handle in threads.drain(..) {
            let _ = handle.join();
        }
    }
}

struct Session {
    generation: u64,
    child: Child,
    stdin: std::process::ChildStdin,
    readers: Vec<JoinHandle<()>>,
}

fn spawn_child(
    config: &ProviderConfig,
    shared: &Shared,
    ui_events: &Arc<BoundedQueue<Event>>,
    records: &Arc<BoundedQueue<String>>,
    stderr_tail: &Arc<Mutex<StderrTail>>,
    wake_ui: &Arc<dyn Fn() + Send + Sync>,
    work: &Arc<BoundedQueue<Work>>,
) -> Result<Session, String> {
    let mut process = ProcessCommand::new(&config.path);
    process
        .args(&config.arguments)
        .current_dir(&config.cwd)
        .env_clear()
        .envs(config.environment.iter().cloned())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
    let mut child = process
        .spawn()
        .map_err(|e| format!("provider spawn: {e}"))?;
    let stdin = child.stdin.take().ok_or("provider stdin missing")?;
    let stdout = child.stdout.take().ok_or("provider stdout missing")?;
    let stderr = child.stderr.take().ok_or("provider stderr missing")?;
    let pid = child.id();
    let generation = {
        let mut state = lock(shared);
        state.generation += 1;
        state.eof = false;
        state.failed = None;
        state.protocol = None;
        state.generation
    };
    let reader_shared = Arc::clone(shared);
    let reader_events = Arc::clone(ui_events);
    let reader_records = Arc::clone(records);
    let reader_wake = Arc::clone(wake_ui);
    let reader_work = Arc::clone(work);
    let stdout_reader = std::thread::spawn(move || {
        stdout_run(
            stdout,
            reader_shared,
            reader_events,
            reader_records,
            reader_work,
            reader_wake,
            generation,
        );
    });
    let tail = Arc::clone(stderr_tail);
    let stderr_reader = std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        let mut pipe = stderr;
        loop {
            match pipe.read(&mut buf) {
                Ok(0) | Err(_) => return,
                Ok(n) => {
                    let mut guard = tail.lock().unwrap_or_else(|e| e.into_inner());
                    guard.total += n as u64;
                    for &b in &buf[..n] {
                        if guard.tail.len() == 4096 {
                            guard.tail.pop_front();
                        }
                        guard.tail.push_back(b);
                    }
                }
            }
        }
    });
    let _ = push_event(ui_events, wake_ui, Event::Started { generation, pid });
    Ok(Session {
        generation,
        child,
        stdin,
        readers: vec![stdout_reader, stderr_reader],
    })
}

fn push_event(
    events: &Arc<BoundedQueue<Event>>,
    wake_ui: &Arc<dyn Fn() + Send + Sync>,
    event: Event,
) -> Result<(), String> {
    events
        .try_push(event)
        .map_err(|_| "provider-to-UI queue overflow".to_string())?;
    wake_ui();
    Ok(())
}

fn push_record(records: &Arc<BoundedQueue<String>>, record: String) -> Result<(), String> {
    records
        .try_push(record)
        .map_err(|_| "control output queue overflow".to_string())
}

fn wait_process_exit(pid: u32, timeout_ms: u32) -> bool {
    unsafe {
        let handle = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
        if handle.is_null() {
            return true;
        }
        let result = WaitForSingleObject(handle, timeout_ms);
        CloseHandle(handle);
        result == WAIT_OBJECT_0
    }
}

fn teardown(
    session: &mut Option<Session>,
    shared: &Shared,
    ui_events: &Arc<BoundedQueue<Event>>,
    wake_ui: &Arc<dyn Fn() + Send + Sync>,
    error: Option<String>,
    shutdown_timeout_ms: u64,
) {
    lock(shared).protocol = None;
    if let Some(mut active) = session.take() {
        let pid = active.child.id();
        if !wait_process_exit(pid, 50) {
            let _ = active.child.kill();
            let _ = wait_process_exit(pid, shutdown_timeout_ms as u32);
        }
        let exit_code = active
            .child
            .wait()
            .ok()
            .and_then(|s| s.code())
            .map(|c| c as u32);
        drop(active.stdin);
        for handle in active.readers.drain(..) {
            let _ = handle.join();
        }
        let generation = active.generation;
        let _ = push_event(
            ui_events,
            wake_ui,
            Event::SessionClosed {
                generation,
                exit_code,
                error,
            },
        );
    }
}

fn coordinator_run(
    config: ProviderConfig,
    work: Arc<BoundedQueue<Work>>,
    shared: Shared,
    ui_events: Arc<BoundedQueue<Event>>,
    records: Arc<BoundedQueue<String>>,
    stderr_tail: Arc<Mutex<StderrTail>>,
    wake_ui: Arc<dyn Fn() + Send + Sync>,
) {
    let mut session: Option<Session> = None;
    let mut done = false;
    while !done {
        let item = match work.wait_pop() {
            Some(item) => item,
            None => break,
        };
        match item {
            Work::Run(Command::Request {
                id,
                prompt,
                scenario,
            }) => {
                let expected = config
                    .scenario_chunks
                    .get(&scenario)
                    .copied()
                    .unwrap_or(100) as usize;
                let needs_child = session.is_none() || lock(&shared).failed.is_some();
                if needs_child {
                    teardown(
                        &mut session,
                        &shared,
                        &ui_events,
                        &wake_ui,
                        None,
                        config.shutdown_timeout_ms,
                    );
                    match spawn_child(
                        &config,
                        &shared,
                        &ui_events,
                        &records,
                        &stderr_tail,
                        &wake_ui,
                        &work,
                    ) {
                        Ok(new_session) => session = Some(new_session),
                        Err(error) => {
                            let generation = lock(&shared).generation;
                            let _ = push_event(
                                &ui_events,
                                &wake_ui,
                                Event::Terminal {
                                    generation,
                                    id,
                                    kind: "failed".into(),
                                    last_seq: -1,
                                },
                            );
                            let _ = push_event(
                                &ui_events,
                                &wake_ui,
                                Event::SessionClosed {
                                    generation,
                                    exit_code: None,
                                    error: Some(error),
                                },
                            );
                            continue;
                        }
                    }
                }
                {
                    let mut state = lock(&shared);
                    state.protocol = Some(RequestProtocol {
                        id,
                        expected_chunks: expected,
                        next_seq: 0,
                        phase: Phase::AwaitStart,
                        cancel_requested: false,
                        client_request_seen: false,
                    });
                }
                let frame = json!({"type":"request","id":id,"prompt":prompt,"scenario":scenario});
                if let Some(active) = session.as_mut() {
                    if writeln!(active.stdin, "{frame}")
                        .and_then(|_| active.stdin.flush())
                        .is_err()
                    {
                        let mut state = lock(&shared);
                        state.failed = Some("provider stdin write failed".into());
                        let generation = state.generation;
                        drop(state);
                        notify(&shared);
                        let _ = push_event(
                            &ui_events,
                            &wake_ui,
                            Event::Terminal {
                                generation,
                                id,
                                kind: "failed".into(),
                                last_seq: -1,
                            },
                        );
                        teardown(
                            &mut session,
                            &shared,
                            &ui_events,
                            &wake_ui,
                            Some("stdin write failed".into()),
                            config.shutdown_timeout_ms,
                        );
                    }
                }
            }
            Work::Run(Command::Cancel { id }) => {
                let active = session.as_mut();
                if let Some(active) = active {
                    {
                        let mut state = lock(&shared);
                        if let Some(protocol) = state.protocol.as_mut() {
                            protocol.cancel_requested = true;
                        }
                    }
                    let frame = json!({"type":"cancel","id":id});
                    let _ = writeln!(active.stdin, "{frame}").and_then(|_| active.stdin.flush());
                    let deadline =
                        std::time::Instant::now() + Duration::from_millis(config.cancel_timeout_ms);
                    let mut state = lock(&shared);
                    loop {
                        let terminal = state
                            .protocol
                            .as_ref()
                            .map(|p| p.phase == Phase::Terminal)
                            .unwrap_or(false);
                        if terminal || state.failed.is_some() || state.eof || state.stop {
                            break;
                        }
                        let remaining =
                            deadline.saturating_duration_since(std::time::Instant::now());
                        if remaining.is_zero() {
                            break;
                        }
                        let (guard, _) = shared
                            .1
                            .wait_timeout(state, remaining)
                            .unwrap_or_else(|e| e.into_inner());
                        state = guard;
                    }
                    let failed_after_cancel = state.failed.is_some() || state.eof;
                    let terminal_seen = state
                        .protocol
                        .as_ref()
                        .map(|p| p.phase == Phase::Terminal)
                        .unwrap_or(false);
                    drop(state);
                    if !terminal_seen && !failed_after_cancel {
                        {
                            let mut state = lock(&shared);
                            state.failed = Some("cancel deadline exceeded".into());
                        }
                        notify(&shared);
                        let generation = session.as_ref().map(|s| s.generation).unwrap_or(0);
                        let _ = push_event(
                            &ui_events,
                            &wake_ui,
                            Event::Terminal {
                                generation,
                                id,
                                kind: "failed".into(),
                                last_seq: -1,
                            },
                        );
                        teardown(
                            &mut session,
                            &shared,
                            &ui_events,
                            &wake_ui,
                            Some("cancel deadline exceeded".into()),
                            config.shutdown_timeout_ms,
                        );
                    }
                }
            }
            Work::ClientResponse { request_id } => {
                if let Some(active) = session.as_mut() {
                    let frame = json!({"type":"client_response","request_id":request_id,"result":{"accepted":true}});
                    if writeln!(active.stdin, "{frame}")
                        .and_then(|_| active.stdin.flush())
                        .is_err()
                    {
                        let mut state = lock(&shared);
                        state.failed = Some("client response write failed".into());
                        drop(state);
                        notify(&shared);
                    }
                }
            }
            Work::SessionFailed { generation, error } => {
                if session.as_ref().map(|s| s.generation) == Some(generation) {
                    teardown(
                        &mut session,
                        &shared,
                        &ui_events,
                        &wake_ui,
                        Some(error),
                        config.shutdown_timeout_ms,
                    );
                }
            }
            Work::SessionEof { generation } => {
                if session.as_ref().map(|s| s.generation) == Some(generation) {
                    teardown(
                        &mut session,
                        &shared,
                        &ui_events,
                        &wake_ui,
                        None,
                        config.shutdown_timeout_ms,
                    );
                }
            }
            Work::Run(Command::Shutdown) => {
                if let Some(active) = session.as_mut() {
                    {
                        let mut state = lock(&shared);
                        state.shutdown_sent = true;
                    }
                    let frame = json!({"type":"shutdown"});
                    let _ = writeln!(active.stdin, "{frame}").and_then(|_| active.stdin.flush());
                    let deadline = std::time::Instant::now()
                        + Duration::from_millis(config.shutdown_timeout_ms);
                    let mut state = lock(&shared);
                    loop {
                        if state.eof || state.stop {
                            break;
                        }
                        let remaining =
                            deadline.saturating_duration_since(std::time::Instant::now());
                        if remaining.is_zero() {
                            break;
                        }
                        let (guard, _) = shared
                            .1
                            .wait_timeout(state, remaining)
                            .unwrap_or_else(|e| e.into_inner());
                        state = guard;
                    }
                    drop(state);
                }
                teardown(
                    &mut session,
                    &shared,
                    &ui_events,
                    &wake_ui,
                    None,
                    config.shutdown_timeout_ms,
                );
                done = true;
            }
        }
        {
            let state = lock(&shared);
            if state.stop {
                done = true;
            }
        }
    }
    teardown(&mut session, &shared, &ui_events, &wake_ui, None, 2000);
}

fn stdout_run(
    mut pipe: std::process::ChildStdout,
    shared: Shared,
    ui_events: Arc<BoundedQueue<Event>>,
    records: Arc<BoundedQueue<String>>,
    work: Arc<BoundedQueue<Work>>,
    wake_ui: Arc<dyn Fn() + Send + Sync>,
    generation: u64,
) {
    let mut decoder = Decoder::new();
    let mut buf = [0u8; 8192];
    let mut fatal: Option<String> = None;
    loop {
        match pipe.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                let input = &buf[..n];
                let shared_ref = &shared;
                let events_ref = &ui_events;
                let records_ref = &records;
                let work_ref = &work;
                let wake_ref = &wake_ui;
                let result = decoder.feed(
                    input,
                    || qpc(),
                    |frame, receipt| {
                        handle_frame(
                            frame,
                            receipt,
                            generation,
                            shared_ref,
                            events_ref,
                            records_ref,
                            work_ref,
                            wake_ref,
                        )
                    },
                );
                if let Err(error) = result {
                    fatal = Some(error);
                    break;
                }
                {
                    let state = lock(&shared);
                    if state.failed.is_some() || state.stop {
                        if let Some(error) = state.failed.clone() {
                            fatal = Some(error);
                        }
                        break;
                    }
                }
            }
            Err(e) => {
                fatal = Some(format!("provider stdout read: {e}"));
                break;
            }
        }
    }
    {
        let mut state = lock(&shared);
        state.eof = true;
        if fatal.is_none() {
            if let Err(error) = decoder.finish() {
                fatal = Some(error);
            }
        }
        if fatal.is_none() {
            if let Some(protocol) = state.protocol.as_ref() {
                if protocol.phase != Phase::Terminal && !state.shutdown_sent {
                    fatal = Some("provider stdout ended mid-request".into());
                }
            }
        }
        if let Some(error) = fatal.clone() {
            if state.failed.is_none() {
                state.failed = Some(error.clone());
            }
        }
        let still_terminal_or_idle = state
            .protocol
            .as_ref()
            .map(|p| p.phase == Phase::Terminal)
            .unwrap_or(true);
        drop(state);
        notify(&shared);
        if let Some(error) = fatal {
            let state = lock(&shared);
            let id = state.protocol.as_ref().map(|p| p.id).unwrap_or(0);
            let last_seq = state
                .protocol
                .as_ref()
                .map(|p| p.next_seq as i64 - 1)
                .unwrap_or(-1);
            let already_terminal = still_terminal_or_idle;
            drop(state);
            if !already_terminal {
                let _ = push_event(
                    &ui_events,
                    &wake_ui,
                    Event::Terminal {
                        generation,
                        id,
                        kind: "failed".into(),
                        last_seq,
                    },
                );
            }
            let _ = work.push(Work::SessionFailed { generation, error });
        } else {
            let _ = work.push(Work::SessionEof { generation });
        }
    }
}

fn handle_frame(
    frame: &[u8],
    receipt_qpc: i64,
    generation: u64,
    shared: &Shared,
    ui_events: &Arc<BoundedQueue<Event>>,
    records: &Arc<BoundedQueue<String>>,
    work: &Arc<BoundedQueue<Work>>,
    wake_ui: &Arc<dyn Fn() + Send + Sync>,
) -> Result<(), String> {
    let value: Value =
        serde_json::from_slice(frame).map_err(|e| format!("invalid frame JSON: {e}"))?;
    let kind = value
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| "frame missing type".to_string())?;
    let frame_id = value.get("id").and_then(Value::as_u64).unwrap_or(0);
    let mut state = lock(shared);
    match kind {
        "start" => {
            let protocol = state
                .protocol
                .as_mut()
                .ok_or("start without active request")?;
            if protocol.id != frame_id {
                return Err("start id mismatch".into());
            }
            if protocol.phase != Phase::AwaitStart {
                return Err("duplicate start".into());
            }
            let chunks = value
                .get("chunks")
                .and_then(Value::as_u64)
                .ok_or("start missing chunks")? as usize;
            if chunks != protocol.expected_chunks {
                return Err("start chunk count mismatch".into());
            }
            let frequency = value
                .get("qpc_frequency")
                .and_then(Value::as_i64)
                .ok_or("start missing qpc_frequency")?;
            if frequency != qpc_frequency() {
                return Err("qpc frequency mismatch".into());
            }
            protocol.phase = Phase::Streaming;
            Ok(())
        }
        "chunk" => {
            let protocol = state.protocol.as_mut().ok_or("chunk without request")?;
            if protocol.id != frame_id {
                return Err("chunk id mismatch".into());
            }
            if protocol.phase != Phase::Streaming {
                return Err("chunk outside streaming".into());
            }
            let seq = value
                .get("seq")
                .and_then(Value::as_u64)
                .ok_or("chunk missing seq")? as usize;
            if seq != protocol.next_seq || seq >= protocol.expected_chunks {
                return Err("chunk seq out of order".into());
            }
            let text = value
                .get("text")
                .and_then(Value::as_str)
                .ok_or("chunk missing text")?
                .to_string();
            let emit_qpc = value
                .get("emit_qpc")
                .and_then(Value::as_str)
                .ok_or("chunk missing emit_qpc")?
                .to_string();
            if emit_qpc.len() != 20
                || !emit_qpc.bytes().all(|b| b.is_ascii_digit())
                || emit_qpc.parse::<i64>().map(|v| v <= 0).unwrap_or(true)
            {
                return Err("invalid emit_qpc".into());
            }
            let frequency = qpc_frequency();
            let record = json!({
                "event": "frame_received",
                "request_id": frame_id,
                "seq": seq,
                "receipt_qpc": receipt_qpc.to_string(),
                "emit_qpc": emit_qpc.clone(),
                "qpc_frequency": frequency,
            })
            .to_string();
            push_record(records, record)?;
            push_event(
                ui_events,
                wake_ui,
                Event::Chunk {
                    generation,
                    id: frame_id,
                    seq: seq as u32,
                    text,
                    receipt_qpc,
                    emit_qpc,
                },
            )?;
            protocol.next_seq += 1;
            Ok(())
        }
        "client_request" => {
            let protocol = state
                .protocol
                .as_mut()
                .ok_or("client_request without request")?;
            if protocol.id != frame_id {
                return Err("client_request id mismatch".into());
            }
            let request_id = value
                .get("request_id")
                .and_then(Value::as_u64)
                .ok_or("client_request missing request_id")?;
            let method = value
                .get("method")
                .and_then(Value::as_str)
                .ok_or("client_request missing method")?;
            let params_ok = value
                .get("params")
                .and_then(|p| p.get("value"))
                .and_then(Value::as_str)
                == Some("ok");
            if request_id != 1 || method != "benchmark.confirm" || !params_ok {
                return Err("unexpected client_request".into());
            }
            if protocol.client_request_seen {
                return Err("duplicate client_request".into());
            }
            protocol.client_request_seen = true;
            work.push(Work::ClientResponse { request_id })
                .map_err(|_| "provider command queue closed".to_string())?;
            Ok(())
        }
        "complete" => {
            let protocol = state.protocol.as_mut().ok_or("complete without request")?;
            if protocol.id != frame_id {
                return Err("complete id mismatch".into());
            }
            if protocol.phase != Phase::Streaming {
                return Err("complete outside streaming".into());
            }
            if protocol.next_seq != protocol.expected_chunks {
                return Err("premature complete".into());
            }
            protocol.phase = Phase::Terminal;
            let id = protocol.id;
            let last_seq = protocol.next_seq as i64 - 1;
            drop(state);
            notify(shared);
            push_event(
                ui_events,
                wake_ui,
                Event::Terminal {
                    generation,
                    id,
                    kind: "complete".into(),
                    last_seq,
                },
            )
        }
        "cancelled" => {
            let shutdown_sent = state.shutdown_sent;
            let protocol = state.protocol.as_mut().ok_or("cancelled without request")?;
            if protocol.id != frame_id {
                return Err("cancelled id mismatch".into());
            }
            if protocol.phase == Phase::Terminal {
                return Err("duplicate terminal".into());
            }
            if !protocol.cancel_requested && !shutdown_sent {
                return Err("cancelled without request".into());
            }
            let last_seq = value
                .get("last_seq")
                .and_then(Value::as_i64)
                .ok_or("cancelled missing last_seq")?;
            if last_seq != protocol.next_seq as i64 - 1 {
                return Err("cancelled last_seq mismatch".into());
            }
            protocol.phase = Phase::Terminal;
            let id = protocol.id;
            drop(state);
            notify(shared);
            push_event(
                ui_events,
                wake_ui,
                Event::Terminal {
                    generation,
                    id,
                    kind: "cancelled".into(),
                    last_seq,
                },
            )
        }
        "shutdown_ack" => {
            if !state.shutdown_sent {
                return Err("unsolicited shutdown_ack".into());
            }
            state.shutdown_ack = true;
            drop(state);
            notify(shared);
            Ok(())
        }
        _ => Err(format!("unexpected frame type '{kind}'")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_shared() -> (
        Shared,
        Arc<BoundedQueue<Event>>,
        Arc<BoundedQueue<String>>,
        Arc<BoundedQueue<Work>>,
    ) {
        (
            shared_new(),
            Arc::new(BoundedQueue::new(64)),
            Arc::new(BoundedQueue::new(256)),
            Arc::new(BoundedQueue::new(16)),
        )
    }

    fn activate(shared: &Shared, id: u64, expected: usize) {
        let mut state = lock(shared);
        state.protocol = Some(RequestProtocol {
            id,
            expected_chunks: expected,
            next_seq: 0,
            phase: Phase::AwaitStart,
            cancel_requested: false,
            client_request_seen: false,
        });
    }

    fn start_frame(id: u64, chunks: usize) -> Vec<u8> {
        json!({"type":"start","id":id,"chunks":chunks,"qpc_frequency":qpc_frequency()})
            .to_string()
            .into_bytes()
    }

    fn chunk_frame(id: u64, seq: u64, text: &str) -> Vec<u8> {
        json!({"type":"chunk","id":id,"seq":seq,"text":text,"emit_qpc":"00000000000000000100"})
            .to_string()
            .into_bytes()
    }

    fn no_wake() -> Arc<dyn Fn() + Send + Sync> {
        Arc::new(|| {})
    }

    fn feed(
        shared: &Shared,
        events: &Arc<BoundedQueue<Event>>,
        records: &Arc<BoundedQueue<String>>,
        work: &Arc<BoundedQueue<Work>>,
        frame: &[u8],
    ) -> Result<(), String> {
        handle_frame(frame, 7, 1, shared, events, records, work, &no_wake())
    }

    #[test]
    fn normal_sequence_accepted() {
        let (shared, events, records, work) = test_shared();
        activate(&shared, 7, 2);
        feed(&shared, &events, &records, &work, &start_frame(7, 2)).unwrap();
        feed(&shared, &events, &records, &work, &chunk_frame(7, 0, "a")).unwrap();
        feed(&shared, &events, &records, &work, &chunk_frame(7, 1, "b")).unwrap();
        feed(
            &shared,
            &events,
            &records,
            &work,
            br#"{"type":"complete","id":7}"#,
        )
        .unwrap();
        let state = lock(&shared);
        assert_eq!(state.protocol.as_ref().unwrap().phase, Phase::Terminal);
        assert_eq!(state.protocol.as_ref().unwrap().next_seq, 2);
        let mut terminal = 0;
        let mut chunks = 0;
        while let Some(event) = events.pop() {
            match event {
                Event::Chunk { seq, .. } => {
                    chunks += 1;
                    assert!(seq < 2);
                }
                Event::Terminal { kind, last_seq, .. } => {
                    terminal += 1;
                    assert_eq!(kind, "complete");
                    assert_eq!(last_seq, 1);
                }
                _ => {}
            }
        }
        assert_eq!(chunks, 2);
        assert_eq!(terminal, 1);
        assert!(records.pop().is_some());
    }

    #[test]
    fn wrong_id_rejected() {
        let (shared, events, records, work) = test_shared();
        activate(&shared, 7, 2);
        feed(&shared, &events, &records, &work, &start_frame(7, 2)).unwrap();
        assert!(feed(&shared, &events, &records, &work, &chunk_frame(8, 0, "a")).is_err());
    }

    #[test]
    fn duplicate_seq_rejected() {
        let (shared, events, records, work) = test_shared();
        activate(&shared, 7, 2);
        feed(&shared, &events, &records, &work, &start_frame(7, 2)).unwrap();
        feed(&shared, &events, &records, &work, &chunk_frame(7, 0, "a")).unwrap();
        assert!(feed(&shared, &events, &records, &work, &chunk_frame(7, 0, "a")).is_err());
    }

    #[test]
    fn skipped_seq_rejected() {
        let (shared, events, records, work) = test_shared();
        activate(&shared, 7, 2);
        feed(&shared, &events, &records, &work, &start_frame(7, 2)).unwrap();
        assert!(feed(&shared, &events, &records, &work, &chunk_frame(7, 1, "b")).is_err());
    }

    #[test]
    fn premature_complete_rejected() {
        let (shared, events, records, work) = test_shared();
        activate(&shared, 7, 2);
        feed(&shared, &events, &records, &work, &start_frame(7, 2)).unwrap();
        feed(&shared, &events, &records, &work, &chunk_frame(7, 0, "a")).unwrap();
        assert!(
            feed(
                &shared,
                &events,
                &records,
                &work,
                br#"{"type":"complete","id":7}"#
            )
            .is_err()
        );
    }

    #[test]
    fn duplicate_terminal_rejected() {
        let (shared, events, records, work) = test_shared();
        activate(&shared, 7, 1);
        feed(&shared, &events, &records, &work, &start_frame(7, 1)).unwrap();
        feed(&shared, &events, &records, &work, &chunk_frame(7, 0, "a")).unwrap();
        feed(
            &shared,
            &events,
            &records,
            &work,
            br#"{"type":"complete","id":7}"#,
        )
        .unwrap();
        assert!(
            feed(
                &shared,
                &events,
                &records,
                &work,
                br#"{"type":"complete","id":7}"#
            )
            .is_err()
        );
    }

    #[test]
    fn frequency_mismatch_rejected() {
        let (shared, events, records, work) = test_shared();
        activate(&shared, 7, 2);
        let frame = json!({"type":"start","id":7,"chunks":2,"qpc_frequency":12345}).to_string();
        assert!(feed(&shared, &events, &records, &work, frame.as_bytes()).is_err());
    }

    #[test]
    fn cancel_after_one_chunk_permits_reuse() {
        let (shared, events, records, work) = test_shared();
        activate(&shared, 7, 2);
        feed(&shared, &events, &records, &work, &start_frame(7, 2)).unwrap();
        feed(&shared, &events, &records, &work, &chunk_frame(7, 0, "a")).unwrap();
        {
            let mut state = lock(&shared);
            state.protocol.as_mut().unwrap().cancel_requested = true;
        }
        feed(
            &shared,
            &events,
            &records,
            &work,
            br#"{"type":"cancelled","id":7,"last_seq":0}"#,
        )
        .unwrap();
        activate(&shared, 8, 1);
        feed(&shared, &events, &records, &work, &start_frame(8, 1)).unwrap();
        feed(&shared, &events, &records, &work, &chunk_frame(8, 0, "z")).unwrap();
        feed(
            &shared,
            &events,
            &records,
            &work,
            br#"{"type":"complete","id":8}"#,
        )
        .unwrap();
        let state = lock(&shared);
        assert_eq!(state.protocol.as_ref().unwrap().phase, Phase::Terminal);
    }

    #[test]
    fn cancelled_without_request_rejected() {
        let (shared, events, records, work) = test_shared();
        activate(&shared, 7, 2);
        feed(&shared, &events, &records, &work, &start_frame(7, 2)).unwrap();
        feed(&shared, &events, &records, &work, &chunk_frame(7, 0, "a")).unwrap();
        assert!(
            feed(
                &shared,
                &events,
                &records,
                &work,
                br#"{"type":"cancelled","id":7,"last_seq":0}"#
            )
            .is_err()
        );
    }

    #[test]
    fn client_request_answers_once() {
        let (shared, events, records, work) = test_shared();
        activate(&shared, 7, 1);
        feed(&shared, &events, &records, &work, &start_frame(7, 1)).unwrap();
        let request = br#"{"type":"client_request","id":7,"request_id":1,"method":"benchmark.confirm","params":{"value":"ok"}}"#;
        feed(&shared, &events, &records, &work, request).unwrap();
        assert!(matches!(
            work.pop(),
            Some(Work::ClientResponse { request_id: 1 })
        ));
        assert!(feed(&shared, &events, &records, &work, request).is_err());
    }
}
