use crate::framing::Decoder;
use crate::queue::BoundedQueue;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::Read;
use std::io::Write;
use std::os::windows::io::AsRawHandle;
use std::os::windows::process::CommandExt;
use std::process::{Child, Command as ProcessCommand, Stdio};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{HANDLE, WAIT_OBJECT_0};
use windows_sys::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
use windows_sys::Win32::System::Threading::WaitForSingleObject;

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
    Stopped {
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
    pub requires_client_response: bool,
    pub client_response_sent: bool,
    pub terminal_emitted: bool,
}

impl RequestProtocol {
    fn claim_terminal(&mut self) -> Option<(u64, i64)> {
        if self.terminal_emitted {
            return None;
        }
        self.terminal_emitted = true;
        self.phase = Phase::Terminal;
        Some((self.id, self.next_seq as i64 - 1))
    }
}

#[derive(Debug)]
enum Work {
    Run(Command),
    ClientResponse {
        generation: u64,
        id: u64,
        request_id: u64,
    },
    SessionFailed {
        generation: u64,
        error: String,
    },
    SessionEof {
        generation: u64,
    },
}

struct SharedState {
    generation: u64,
    protocol: Option<RequestProtocol>,
    eof: bool,
    shutdown_sent: bool,
    shutdown_ack: bool,
    shutdown_requested: bool,
    shutdown_deadline: Option<Instant>,
    failed: Option<String>,
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
            shutdown_requested: false,
            shutdown_deadline: None,
            failed: None,
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
    shutdown_timeout_ms: u64,
}

impl Provider {
    pub fn spawn(
        config: ProviderConfig,
        ui_events: Arc<BoundedQueue<Event>>,
        records: Option<Arc<BoundedQueue<String>>>,
        wake_ui: Arc<dyn Fn() + Send + Sync>,
    ) -> Arc<Self> {
        let shared = shared_new();
        let work = Arc::new(BoundedQueue::<Work>::new(16));
        let stderr_tail = Arc::new(Mutex::new(StderrTail::default()));
        let shutdown_timeout_ms = config.shutdown_timeout_ms;
        let provider = Arc::new(Self {
            work: Arc::clone(&work),
            shared: Arc::clone(&shared),
            stderr_tail: Arc::clone(&stderr_tail),
            threads: Mutex::new(Vec::new()),
            shutdown_timeout_ms,
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
        match command {
            Command::Shutdown => {
                {
                    let mut state = lock(&self.shared);
                    if !state.shutdown_requested {
                        state.shutdown_requested = true;
                        state.shutdown_deadline =
                            Some(Instant::now() + Duration::from_millis(self.shutdown_timeout_ms));
                    }
                }
                notify(&self.shared);
                self.work.close();
                Ok(())
            }
            other => self
                .work
                .try_push(Work::Run(other))
                .map_err(|_| "provider command queue unavailable".to_string()),
        }
    }

    pub fn stderr_total(&self) -> u64 {
        self.stderr_tail
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .total
    }

    pub fn join(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let mut threads = self.threads.lock().unwrap_or_else(|e| e.into_inner());
        let mut finished = true;
        for handle in threads.iter() {
            while !handle.is_finished() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(5));
            }
            if !handle.is_finished() {
                finished = false;
            }
        }
        if finished {
            for handle in threads.drain(..) {
                let _ = handle.join();
            }
        }
        finished
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
    records: &Option<Arc<BoundedQueue<String>>>,
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
        state.shutdown_sent = false;
        state.shutdown_ack = false;
        state.generation
    };
    let reader_shared = Arc::clone(shared);
    let reader_events = Arc::clone(ui_events);
    let reader_records = records.clone();
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
        .push(event)
        .map_err(|_| "provider-to-UI queue closed".to_string())?;
    wake_ui();
    Ok(())
}

fn push_record(records: &Option<Arc<BoundedQueue<String>>>, record: String) -> Result<(), String> {
    let Some(records) = records else {
        return Ok(());
    };
    records
        .try_push(record)
        .map_err(|_| "control output queue overflow".to_string())
}

fn wait_handle(handle: HANDLE, deadline: Instant) -> bool {
    let remaining = deadline.saturating_duration_since(Instant::now());
    let ms = remaining.as_millis().min(u32::MAX as u128) as u32;
    unsafe { WaitForSingleObject(handle, ms) == WAIT_OBJECT_0 }
}

fn join_readers(readers: &mut Vec<JoinHandle<()>>, deadline: Instant) -> Result<(), String> {
    for handle in readers.drain(..) {
        while !handle.is_finished() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        if !handle.is_finished() {
            return Err("provider reader did not finish within teardown bound".into());
        }
        let _ = handle.join();
    }
    Ok(())
}

fn teardown(
    session: &mut Option<Session>,
    ui_events: &Arc<BoundedQueue<Event>>,
    wake_ui: &Arc<dyn Fn() + Send + Sync>,
    error: Option<String>,
    deadline: Instant,
) -> Option<String> {
    let Some(mut active) = session.take() else {
        return error;
    };
    let mut error = error;
    let handle = active.child.as_raw_handle() as HANDLE;
    let grace = if error.is_some() {
        Instant::now() + Duration::from_millis(100)
    } else {
        deadline
    };
    let mut reaped = wait_handle(handle, grace.min(deadline));
    if !reaped {
        if active.child.kill().is_err() {
            error = Some(format!(
                "{}provider kill failed",
                error.map(|e| e + "; ").unwrap_or_default()
            ));
        }
        reaped = wait_handle(handle, deadline);
        if !reaped {
            error = Some(format!(
                "{}provider did not exit before teardown deadline",
                error.map(|e| e + "; ").unwrap_or_default()
            ));
        }
    }
    let exit_code = if reaped {
        active
            .child
            .wait()
            .ok()
            .and_then(|s| s.code().map(|c| c as u32))
    } else {
        None
    };
    drop(active.stdin);
    let reader_deadline = deadline.max(Instant::now() + Duration::from_millis(500));
    if let Err(join_error) = join_readers(&mut active.readers, reader_deadline) {
        error = Some(format!(
            "{}{join_error}",
            error.map(|e| e + "; ").unwrap_or_default()
        ));
    }
    let generation = active.generation;
    let _ = push_event(
        ui_events,
        wake_ui,
        Event::SessionClosed {
            generation,
            exit_code,
            error: error.clone(),
        },
    );
    error
}

fn service_client_response(
    active: &mut Session,
    shared: &Shared,
    deadline: Instant,
) -> Result<(), String> {
    {
        let mut state = lock(shared);
        loop {
            let Some(protocol) = state.protocol.as_ref() else {
                return Ok(());
            };
            if !protocol.requires_client_response || protocol.client_response_sent {
                return Ok(());
            }
            if protocol.client_request_seen {
                break;
            }
            if state.eof || state.failed.is_some() {
                return Ok(());
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err("client_request response deadline exceeded".into());
            }
            let (guard, _) = shared
                .1
                .wait_timeout(state, remaining)
                .unwrap_or_else(|e| e.into_inner());
            state = guard;
        }
    }
    let frame = json!({"type":"client_response","request_id":1,"result":{"accepted":true}});
    writeln!(active.stdin, "{frame}")
        .and_then(|_| active.stdin.flush())
        .map_err(|_| "client response write failed".to_string())?;
    let mut state = lock(shared);
    if let Some(protocol) = state.protocol.as_mut() {
        protocol.client_response_sent = true;
    }
    Ok(())
}

fn graceful_shutdown(
    session: &mut Option<Session>,
    shared: &Shared,
    ui_events: &Arc<BoundedQueue<Event>>,
    wake_ui: &Arc<dyn Fn() + Send + Sync>,
    timeout_ms: u64,
) -> Option<String> {
    let deadline = {
        let mut state = lock(shared);
        match state.shutdown_deadline {
            Some(deadline) => deadline,
            None => {
                let deadline = Instant::now() + Duration::from_millis(timeout_ms);
                state.shutdown_deadline = Some(deadline);
                deadline
            }
        }
    };
    let mut error: Option<String> = None;
    if let Some(active) = session.as_mut() {
        let ack_deadline = deadline
            .checked_sub(Duration::from_millis(100))
            .unwrap_or_else(Instant::now);
        let _ = service_client_response(active, shared, ack_deadline);
        {
            let mut state = lock(shared);
            state.shutdown_sent = true;
        }
        let frame = json!({"type":"shutdown"});
        if writeln!(active.stdin, "{frame}")
            .and_then(|_| active.stdin.flush())
            .is_err()
        {
            error = Some("provider shutdown write failed".into());
        }
        let ack_deadline = deadline
            .checked_sub(Duration::from_millis(100))
            .unwrap_or_else(Instant::now);
        let mut state = lock(shared);
        loop {
            if state.eof || state.shutdown_ack {
                break;
            }
            let remaining = ack_deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            let (guard, _) = shared
                .1
                .wait_timeout(state, remaining)
                .unwrap_or_else(|e| e.into_inner());
            state = guard;
        }
        let acknowledged = state.shutdown_ack;
        drop(state);
        if !acknowledged && error.is_none() {
            error = Some("provider shutdown acknowledgement timeout".into());
        }
    }
    let teardown_error = teardown(session, ui_events, wake_ui, error, deadline);
    teardown_error
}

fn coordinator_run(
    config: ProviderConfig,
    work: Arc<BoundedQueue<Work>>,
    shared: Shared,
    ui_events: Arc<BoundedQueue<Event>>,
    records: Option<Arc<BoundedQueue<String>>>,
    stderr_tail: Arc<Mutex<StderrTail>>,
    wake_ui: Arc<dyn Fn() + Send + Sync>,
) {
    let mut session: Option<Session> = None;
    loop {
        if lock(&shared).shutdown_requested {
            break;
        }
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
                        &ui_events,
                        &wake_ui,
                        None,
                        Instant::now() + Duration::from_millis(config.shutdown_timeout_ms),
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
                        requires_client_response: scenario == "client_request",
                        client_response_sent: false,
                        terminal_emitted: false,
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
                        let claim = state
                            .protocol
                            .as_mut()
                            .and_then(RequestProtocol::claim_terminal);
                        drop(state);
                        notify(&shared);
                        if let Some((failed_id, last_seq)) = claim {
                            let _ = push_event(
                                &ui_events,
                                &wake_ui,
                                Event::Terminal {
                                    generation: session.as_ref().map(|s| s.generation).unwrap_or(0),
                                    id: failed_id,
                                    kind: "failed".into(),
                                    last_seq,
                                },
                            );
                        }
                        teardown(
                            &mut session,
                            &ui_events,
                            &wake_ui,
                            Some("stdin write failed".into()),
                            Instant::now() + Duration::from_millis(config.shutdown_timeout_ms),
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
                            if protocol.id == id {
                                protocol.cancel_requested = true;
                            }
                        }
                    }
                    let deadline = Instant::now() + Duration::from_millis(config.cancel_timeout_ms);
                    if let Err(error) = service_client_response(active, &shared, deadline) {
                        {
                            let mut state = lock(&shared);
                            state.failed = Some(error.clone());
                        }
                        notify(&shared);
                        teardown(
                            &mut session,
                            &ui_events,
                            &wake_ui,
                            Some(error),
                            Instant::now() + Duration::from_millis(config.shutdown_timeout_ms),
                        );
                        continue;
                    }
                    let frame = json!({"type":"cancel","id":id});
                    let _ = writeln!(active.stdin, "{frame}").and_then(|_| active.stdin.flush());
                    let mut state = lock(&shared);
                    loop {
                        let terminal = state
                            .protocol
                            .as_ref()
                            .map(|p| p.phase == Phase::Terminal)
                            .unwrap_or(false);
                        if terminal
                            || state.failed.is_some()
                            || state.eof
                            || state.shutdown_requested
                        {
                            break;
                        }
                        let remaining = deadline.saturating_duration_since(Instant::now());
                        if remaining.is_zero() {
                            break;
                        }
                        let (guard, _) = shared
                            .1
                            .wait_timeout(state, remaining)
                            .unwrap_or_else(|e| e.into_inner());
                        state = guard;
                    }
                    let terminal_seen = state
                        .protocol
                        .as_ref()
                        .map(|p| p.phase == Phase::Terminal)
                        .unwrap_or(false);
                    let aborted = state.failed.is_some() || state.eof || state.shutdown_requested;
                    let claim = if terminal_seen || aborted {
                        None
                    } else {
                        state.failed = Some("cancel deadline exceeded".into());
                        state
                            .protocol
                            .as_mut()
                            .and_then(RequestProtocol::claim_terminal)
                    };
                    drop(state);
                    if let Some((failed_id, last_seq)) = claim {
                        notify(&shared);
                        let _ = push_event(
                            &ui_events,
                            &wake_ui,
                            Event::Terminal {
                                generation: session.as_ref().map(|s| s.generation).unwrap_or(0),
                                id: failed_id,
                                kind: "failed".into(),
                                last_seq,
                            },
                        );
                        teardown(
                            &mut session,
                            &ui_events,
                            &wake_ui,
                            Some("cancel deadline exceeded".into()),
                            Instant::now() + Duration::from_millis(config.shutdown_timeout_ms),
                        );
                    }
                }
            }
            Work::ClientResponse {
                generation,
                id,
                request_id,
            } => {
                if let Some(active) = session.as_mut() {
                    let current = {
                        let state = lock(&shared);
                        state.generation == generation
                            && state.protocol.as_ref().map(|p| p.id) == Some(id)
                            && request_id == 1
                    };
                    if current {
                        let deadline =
                            Instant::now() + Duration::from_millis(config.cancel_timeout_ms);
                        if let Err(error) = service_client_response(active, &shared, deadline) {
                            let mut state = lock(&shared);
                            state.failed = Some(error);
                            drop(state);
                            notify(&shared);
                        }
                    }
                }
            }
            Work::SessionFailed { generation, error } => {
                if session.as_ref().map(|s| s.generation) == Some(generation) {
                    teardown(
                        &mut session,
                        &ui_events,
                        &wake_ui,
                        Some(error),
                        Instant::now() + Duration::from_millis(config.shutdown_timeout_ms),
                    );
                }
            }
            Work::SessionEof { generation } => {
                if session.as_ref().map(|s| s.generation) == Some(generation) {
                    teardown(
                        &mut session,
                        &ui_events,
                        &wake_ui,
                        None,
                        Instant::now() + Duration::from_millis(config.shutdown_timeout_ms),
                    );
                }
            }
            Work::Run(Command::Shutdown) => {
                break;
            }
        }
        if lock(&shared).shutdown_requested {
            break;
        }
    }
    let shutdown_error = graceful_shutdown(
        &mut session,
        &shared,
        &ui_events,
        &wake_ui,
        config.shutdown_timeout_ms,
    );
    let _ = push_event(
        &ui_events,
        &wake_ui,
        Event::Stopped {
            error: shutdown_error,
        },
    );
}

fn stdout_run(
    mut pipe: std::process::ChildStdout,
    shared: Shared,
    ui_events: Arc<BoundedQueue<Event>>,
    records: Option<Arc<BoundedQueue<String>>>,
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
                if lock(&shared).failed.is_some() {
                    fatal = lock(&shared).failed.clone();
                    break;
                }
            }
            Err(e) => {
                fatal = Some(format!("provider stdout read: {e}"));
                break;
            }
        }
    }
    let claim;
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
                if protocol.phase != Phase::Terminal
                    && !state.shutdown_requested
                    && !state.shutdown_sent
                {
                    fatal = Some("provider stdout ended mid-request".into());
                }
            }
        }
        if let Some(error) = fatal.clone() {
            if state.failed.is_none() {
                state.failed = Some(error);
            }
        }
        claim = if fatal.is_some() {
            state
                .protocol
                .as_mut()
                .and_then(RequestProtocol::claim_terminal)
        } else {
            None
        };
        drop(state);
    }
    notify(&shared);
    if let Some(error) = fatal {
        if let Some((id, last_seq)) = claim {
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
        if work
            .try_push(Work::SessionFailed { generation, error })
            .is_err()
        {
            let state = lock(&shared);
            if !state.shutdown_requested {
                drop(state);
                let _ = push_record(
                    &records,
                    json!({"token": null, "ok": false, "error": "provider work queue unavailable"})
                        .to_string(),
                );
            }
        }
    } else {
        let _ = work.try_push(Work::SessionEof { generation });
    }
}

fn handle_frame(
    frame: &[u8],
    receipt_qpc: i64,
    generation: u64,
    shared: &Shared,
    ui_events: &Arc<BoundedQueue<Event>>,
    records: &Option<Arc<BoundedQueue<String>>>,
    work: &Arc<BoundedQueue<Work>>,
    wake_ui: &Arc<dyn Fn() + Send + Sync>,
) -> Result<(), String> {
    let value: Value =
        serde_json::from_slice(frame).map_err(|e| format!("invalid frame JSON: {e}"))?;
    let kind = value
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| "frame missing type".to_string())?
        .to_string();
    let frame_id = value.get("id").and_then(Value::as_u64).unwrap_or(0);
    enum Deliver {
        None,
        Chunk {
            id: u64,
            seq: u32,
            text: String,
            emit_qpc: String,
        },
        Terminal {
            id: u64,
            kind: String,
            last_seq: i64,
        },
    }
    let mut deliver = Deliver::None;
    let mut frame_record: Option<String> = None;
    {
        let mut state = lock(shared);
        if state.generation != generation {
            return Err("stale session frame".into());
        }
        match kind.as_str() {
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
                protocol.next_seq += 1;
                frame_record = Some(
                    json!({
                        "event": "frame_received",
                        "request_id": frame_id,
                        "seq": seq,
                        "receipt_qpc": receipt_qpc.to_string(),
                        "emit_qpc": emit_qpc.clone(),
                        "qpc_frequency": qpc_frequency(),
                    })
                    .to_string(),
                );
                deliver = Deliver::Chunk {
                    id: frame_id,
                    seq: seq as u32,
                    text,
                    emit_qpc,
                };
            }
            "client_request" => {
                let protocol = state
                    .protocol
                    .as_mut()
                    .ok_or("client_request without request")?;
                if protocol.id != frame_id {
                    return Err("client_request id mismatch".into());
                }
                if protocol.phase != Phase::Streaming {
                    return Err("client_request outside streaming".into());
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
                let active_id = protocol.id;
                drop(state);
                notify(shared);
                match work.try_push(Work::ClientResponse {
                    generation,
                    id: active_id,
                    request_id,
                }) {
                    Ok(()) => {}
                    Err(_) => {
                        if lock(shared).shutdown_requested {
                            return Ok(());
                        }
                        let _ = push_record(
                            records,
                            json!({"token": null, "ok": false, "error": "provider work queue overflow"})
                                .to_string(),
                        );
                        return Err("provider work queue overflow".into());
                    }
                }
                return Ok(());
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
                let Some((id, last_seq)) = protocol.claim_terminal() else {
                    return Err("duplicate terminal".into());
                };
                deliver = Deliver::Terminal {
                    id,
                    kind: "complete".into(),
                    last_seq,
                };
            }
            "cancelled" => {
                let shutdown_sent = state.shutdown_sent || state.shutdown_requested;
                let protocol = state.protocol.as_mut().ok_or("cancelled without request")?;
                if protocol.id != frame_id {
                    return Err("cancelled id mismatch".into());
                }
                if protocol.phase == Phase::Terminal || protocol.terminal_emitted {
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
                let Some((id, seq)) = protocol.claim_terminal() else {
                    return Err("duplicate terminal".into());
                };
                deliver = Deliver::Terminal {
                    id,
                    kind: "cancelled".into(),
                    last_seq: seq,
                };
            }
            "shutdown_ack" => {
                if !state.shutdown_sent {
                    return Err("unsolicited shutdown_ack".into());
                }
                if state.shutdown_ack {
                    return Err("duplicate shutdown_ack".into());
                }
                state.shutdown_ack = true;
            }
            _ => return Err(format!("unexpected frame type '{kind}'")),
        }
    }
    notify(shared);
    if let Some(record) = frame_record {
        push_record(records, record)?;
    }
    match deliver {
        Deliver::None => Ok(()),
        Deliver::Chunk {
            id,
            seq,
            text,
            emit_qpc,
        } => push_event(
            ui_events,
            wake_ui,
            Event::Chunk {
                generation,
                id,
                seq,
                text,
                receipt_qpc,
                emit_qpc,
            },
        ),
        Deliver::Terminal { id, kind, last_seq } => push_event(
            ui_events,
            wake_ui,
            Event::Terminal {
                generation,
                id,
                kind,
                last_seq,
            },
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

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
        state.generation = 1;
        state.protocol = Some(RequestProtocol {
            id,
            expected_chunks: expected,
            next_seq: 0,
            phase: Phase::AwaitStart,
            cancel_requested: false,
            client_request_seen: false,
            requires_client_response: false,
            client_response_sent: false,
            terminal_emitted: false,
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
        handle_frame(
            frame,
            7,
            1,
            shared,
            events,
            &Some(records.clone()),
            work,
            &no_wake(),
        )
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
    fn terminal_claim_once() {
        let (shared, events, records, work) = test_shared();
        activate(&shared, 7, 3);
        feed(&shared, &events, &records, &work, &start_frame(7, 3)).unwrap();
        feed(&shared, &events, &records, &work, &chunk_frame(7, 0, "a")).unwrap();
        feed(&shared, &events, &records, &work, &chunk_frame(7, 1, "b")).unwrap();
        {
            let mut state = lock(&shared);
            let first = state.protocol.as_mut().unwrap().claim_terminal();
            assert_eq!(first, Some((7, 1)));
            assert!(state.protocol.as_mut().unwrap().claim_terminal().is_none());
        }
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
            Some(Work::ClientResponse {
                generation: 1,
                id: 7,
                request_id: 1
            })
        ));
        assert!(feed(&shared, &events, &records, &work, request).is_err());
    }

    #[test]
    fn client_request_before_start_rejected() {
        let (shared, events, records, work) = test_shared();
        activate(&shared, 7, 1);
        let request = br#"{"type":"client_request","id":7,"request_id":1,"method":"benchmark.confirm","params":{"value":"ok"}}"#;
        assert!(feed(&shared, &events, &records, &work, request).is_err());
    }

    #[test]
    fn client_request_after_terminal_rejected() {
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
        let request = br#"{"type":"client_request","id":7,"request_id":1,"method":"benchmark.confirm","params":{"value":"ok"}}"#;
        assert!(feed(&shared, &events, &records, &work, request).is_err());
    }

    #[test]
    fn shutdown_ack_requires_request_and_is_once() {
        let (shared, events, records, work) = test_shared();
        lock(&shared).generation = 1;
        let ack = br#"{"type":"shutdown_ack"}"#;
        assert!(feed(&shared, &events, &records, &work, ack).is_err());
        {
            lock(&shared).shutdown_sent = true;
        }
        feed(&shared, &events, &records, &work, ack).unwrap();
        assert!(feed(&shared, &events, &records, &work, ack).is_err());
    }

    #[test]
    fn shutdown_ack_accepted_mid_stream_when_requested() {
        let (shared, events, records, work) = test_shared();
        activate(&shared, 7, 2);
        feed(&shared, &events, &records, &work, &start_frame(7, 2)).unwrap();
        feed(&shared, &events, &records, &work, &chunk_frame(7, 0, "a")).unwrap();
        lock(&shared).shutdown_sent = true;
        feed(
            &shared,
            &events,
            &records,
            &work,
            br#"{"type":"shutdown_ack"}"#,
        )
        .unwrap();
        assert!(lock(&shared).shutdown_ack);
    }

    #[test]
    fn event_queue_backpressure_blocks_reader_not_drops() {
        let shared = shared_new();
        let events = Arc::new(BoundedQueue::<Event>::new(1));
        let records = Arc::new(BoundedQueue::<String>::new(256));
        let work = Arc::new(BoundedQueue::<Work>::new(16));
        activate(&shared, 7, 2);
        feed(&shared, &events, &records, &work, &start_frame(7, 2)).unwrap();
        events
            .push(Event::Started {
                generation: 1,
                pid: 1,
            })
            .unwrap();
        let (tx, rx) = mpsc::channel::<Result<(), String>>();
        let shared2 = Arc::clone(&shared);
        let events2 = Arc::clone(&events);
        let records2 = Arc::clone(&records);
        let work2 = Arc::clone(&work);
        let reader = std::thread::spawn(move || {
            let frame = chunk_frame(7, 0, "a");
            let result = handle_frame(
                &frame,
                7,
                1,
                &shared2,
                &events2,
                &Some(records2),
                &work2,
                &no_wake(),
            );
            let _ = tx.send(result);
        });
        match rx.recv_timeout(Duration::from_millis(20)) {
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            other => panic!("expected blocked push, got {other:?}"),
        }
        assert!(matches!(events.pop(), Some(Event::Started { .. })));
        let result = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        result.unwrap();
        reader.join().unwrap();
        match events.pop() {
            Some(Event::Chunk {
                id: 7,
                seq: 0,
                text,
                ..
            }) => assert_eq!(text, "a"),
            other => panic!("expected single chunk event, got {other:?}"),
        }
        assert!(events.pop().is_none());
    }
}
