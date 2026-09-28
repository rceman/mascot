// Untimed Codex app-server compatibility gate (benchmark protocol §16).
// Launches a pinned `codex app-server` over stdio through the same
// explicit-environment process machinery the provider path uses, completes
// JSON-RPC initialization, performs one read-only streamed interaction, and
// tears the process down cleanly. Emits gate evidence as JSON lines on
// stdout; exit code 0 = PASS, 1 = FAIL.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command as ProcessCommand, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

fn gate_line(step: &str, detail: &str) {
    let clean: String = detail
        .chars()
        .map(|c| if c == '"' || c == '\\' || c < ' ' { ' ' } else { c })
        .collect();
    println!("{{\"event\":\"codex_gate\",\"step\":\"{step}\",\"detail\":\"{clean}\"}}");
    let _ = std::io::stdout().flush();
}

pub fn run(codex_exe: &str) -> i32 {
    match gate(codex_exe) {
        Ok(()) => {
            gate_line("result", "PASS");
            0
        }
        Err(error) => {
            gate_line("result", "FAIL");
            eprintln!("codex-gate: {error}");
            1
        }
    }
}

fn gate(codex_exe: &str) -> Result<(), String> {
    let cwd = std::path::Path::new(codex_exe)
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or("codex path has no parent")?;
    #[cfg(windows)]
    let mut process = {
        let profile = std::env::var("USERPROFILE").unwrap_or_default();
        let temp = std::env::temp_dir().to_string_lossy().into_owned();
        let mut process = ProcessCommand::new(codex_exe);
        process
            .arg("app-server")
            .env_clear()
            .env("PATH", r"C:\Windows\System32;C:\Windows")
            .env("SystemRoot", r"C:\Windows")
            .env("WINDIR", r"C:\Windows")
            .env("TEMP", &temp)
            .env("TMP", &temp)
            .env("USERPROFILE", &profile)
            .env("HOMEDRIVE", "C:")
            .env("HOMEPATH", profile.trim_start_matches("C:"));
        process
    };
    #[cfg(unix)]
    let mut process = {
        let home = std::env::var("HOME").unwrap_or_default();
        let mut process = ProcessCommand::new(codex_exe);
        process
            .arg("app-server")
            .env_clear()
            .env("PATH", "/usr/bin:/bin:/opt/homebrew/bin")
            .env("HOME", &home)
            .env("LANG", "en_US.UTF-8");
        process
    };
    process
        .current_dir(&cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = process.spawn().map_err(|e| format!("spawn: {e}"))?;
    gate_line("spawn", "codex app-server started");
    let mut stdin = child.stdin.take().ok_or("stdin missing")?;
    let stdout = child.stdout.take().ok_or("stdout missing")?;
    let mut stderr = child.stderr.take().ok_or("stderr missing")?;

    let (tx, rx) = mpsc::channel::<Result<String, String>>();
    let stdout_tx = tx.clone();
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    let _ = stdout_tx.send(Err("eof".into()));
                    return;
                }
                Ok(_) => {
                    if stdout_tx.send(Ok(line.trim_end().to_owned())).is_err() {
                        return;
                    }
                }
                Err(e) => {
                    let _ = stdout_tx.send(Err(format!("stdout: {e}")));
                    return;
                }
            }
        }
    });
    let stderr_bytes = thread::spawn(move || {
        let mut sink = Vec::new();
        let _ = std::io::Read::read_to_end(&mut stderr, &mut sink);
        sink.len()
    });

    let send = |stdin: &mut std::process::ChildStdin, frame: &str| {
        stdin
            .write_all(frame.as_bytes())
            .and_then(|_| stdin.write_all(b"\n"))
            .and_then(|_| stdin.flush())
            .map_err(|e| format!("write: {e}"))
    };

    let deadline = Instant::now() + Duration::from_secs(30);
    let mut answered = [false; 4]; // ids 1..=3
    let mut notifications = 0_u64;
    let mut received = 0_u64;
    send(&mut stdin, "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"clientInfo\":{\"name\":\"mascot-gate\",\"version\":\"0.1\"}}}")
        .map_err(|e| format!("initialize send: {e}"))?;
    send(&mut stdin,
         "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}")?;
    send(&mut stdin,
         "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"config/read\",\"params\":{}}")?;
    send(&mut stdin,
         "{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"thread/list\",\"params\":{}}")?;

    while !answered[1] || !answered[2] || !answered[3] {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err("timeout waiting for app-server replies".into());
        }
        match rx.recv_timeout(left.min(Duration::from_millis(500))) {
            Ok(Ok(line)) => {
                received += 1;
                for id in 1..=3_usize {
                    let marker = format!("\"id\":{id}");
                    if line.contains(&marker) {
                        if line.contains("\"error\"") {
                            return Err(format!("id {id} errored: {line}"));
                        }
                        answered[id] = true;
                        gate_line("reply", &format!("id {id} answered"));
                    }
                }
                if !line.contains("\"id\"") {
                    notifications += 1;
                }
            }
            Ok(Err(e)) if e == "eof" => return Err("app-server closed stdout".into()),
            Ok(Err(e)) => return Err(e),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => return Err("reader channel closed".into()),
        }
    }
    gate_line("interaction", &format!("config/read + thread/list answered; \
{received} frames, {notifications} unsolicited notifications"));

    // Clean teardown: close stdin, allow graceful exit, terminate on timeout.
    drop(stdin);
    let wait_deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match child.try_wait().map_err(|e| format!("try_wait: {e}"))? {
            Some(code) => {
                gate_line("teardown", &format!("exited code {code:?}"));
                break;
            }
            None if Instant::now() < wait_deadline => thread::sleep(Duration::from_millis(50)),
            None => {
                let _ = child.kill();
                let _ = child.wait();
                gate_line("teardown", "terminated after stdin close timeout");
                break;
            }
        }
    }
    let stderr_len = stderr_bytes
        .join()
        .map_err(|_| String::from("stderr join"))?;
    gate_line("teardown", &format!("stderr drained {stderr_len} bytes"));
    Ok(())
}

// Devin ACP compatibility gate (macOS Stage B). Launches `devin acp` over
// stdio with an explicit environment, completes the ACP initialize
// handshake, opens a session, issues one read-only prompt, records streamed
// session/update notifications, and tears the process down cleanly.
pub fn run_acp(acp_exe: &str) -> i32 {
    match acp_gate(acp_exe) {
        Ok(()) => {
            acp_line("result", "PASS");
            0
        }
        Err(error) => {
            acp_line("result", "FAIL");
            eprintln!("acp-gate: {error}");
            1
        }
    }
}

fn acp_line(step: &str, detail: &str) {
    let clean: String = detail
        .chars()
        .map(|c| if c == '"' || c == '\\' || c < ' ' { ' ' } else { c })
        .collect();
    println!("{{\"event\":\"acp_gate\",\"step\":\"{step}\",\"detail\":\"{clean}\"}}");
    let _ = std::io::stdout().flush();
}

fn acp_gate(acp_exe: &str) -> Result<(), String> {
    let cwd = std::path::Path::new(acp_exe)
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or("acp executable path has no parent")?;
    let home = std::env::var("HOME").unwrap_or_default();
    let mut process = ProcessCommand::new(acp_exe);
    process
        .arg("acp")
        .current_dir(&cwd)
        .env_clear()
        .env("PATH", "/usr/bin:/bin:/opt/homebrew/bin:/usr/local/bin")
        .env("HOME", &home)
        .env("LANG", "en_US.UTF-8")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = process.spawn().map_err(|e| format!("spawn: {e}"))?;
    acp_line("spawn", "devin acp started");
    let mut stdin = child.stdin.take().ok_or("stdin missing")?;
    let stdout = child.stdout.take().ok_or("stdout missing")?;
    let mut stderr = child.stderr.take().ok_or("stderr missing")?;

    let (tx, rx) = mpsc::channel::<Result<String, String>>();
    let stdout_tx = tx.clone();
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    let _ = stdout_tx.send(Err("eof".into()));
                    return;
                }
                Ok(_) => {
                    if stdout_tx.send(Ok(line.trim_end().to_owned())).is_err() {
                        return;
                    }
                }
                Err(e) => {
                    let _ = stdout_tx.send(Err(format!("stdout: {e}")));
                    return;
                }
            }
        }
    });
    let stderr_bytes = thread::spawn(move || {
        let mut sink = Vec::new();
        let _ = std::io::Read::read_to_end(&mut stderr, &mut sink);
        sink.len()
    });

    let send = |stdin: &mut std::process::ChildStdin, frame: &str| {
        stdin
            .write_all(frame.as_bytes())
            .and_then(|_| stdin.write_all(b"\n"))
            .and_then(|_| stdin.flush())
            .map_err(|e| format!("write: {e}"))
    };

    let deadline = Instant::now() + Duration::from_secs(60);
    let mut got = [false; 4];
    let mut notifications = 0_u64;
    let mut updates = 0_u64;
    let mut session_id = String::new();
    send(&mut stdin, "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":1,\"clientCapabilities\":{\"fs\":{\"readTextFile\":false,\"writeTextFile\":false},\"terminal\":false},\"clientInfo\":{\"name\":\"mascot-acp-gate\",\"version\":\"0.1\"}}}")
        .map_err(|e| format!("initialize send: {e}"))?;
    send(&mut stdin, "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"session/new\",\"params\":{\"cwd\":\"/tmp\",\"mcpServers\":[]}}")
        .map_err(|e| format!("session/new send: {e}"))?;

    while !got[1] || !got[2] || !got[3] {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err("timeout waiting for ACP replies".into());
        }
        match rx.recv_timeout(left.min(Duration::from_millis(500))) {
            Ok(Ok(line)) => {
                if line.contains("\"method\":\"session/update\"")
                    || line.contains("\"sessionUpdate\"")
                {
                    updates += 1;
                }
                if !line.contains("\"id\"") {
                    notifications += 1;
                }
                for id in 1..=3_usize {
                    if line.contains(&format!("\"id\":{id}")) {
                        if line.contains("\"error\"") {
                            return Err(format!("id {id} errored: {line}"));
                        }
                        if !got[id] {
                            got[id] = true;
                            acp_line("reply", &format!("id {id} answered"));
                        }
                    }
                }
                if got[2] && session_id.is_empty() {
                    if let Some(start) = line.find("\"sessionId\":\"") {
                        let rest = &line[start + 13..];
                        if let Some(end) = rest.find('"') {
                            session_id = rest[..end].to_string();
                            acp_line("session", &format!("sessionId {session_id}"));
                            let prompt = format!(
                                "{{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"session/prompt\",\"params\":{{\"sessionId\":\"{session_id}\",\"prompt\":[{{\"type\":\"text\",\"text\":\"Reply with exactly: ok\"}}]}}}}"
                            );
                            send(&mut stdin, &prompt)
                                .map_err(|e| format!("prompt send: {e}"))?;
                        }
                    }
                }
            }
            Ok(Err(e)) if e == "eof" => return Err("acp server closed stdout".into()),
            Ok(Err(e)) => return Err(e),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => return Err("reader channel closed".into()),
        }
    }
    acp_line(
        "interaction",
        &format!("initialize + session/new + session/prompt answered; {updates} session/update notifications"),
    );

    drop(stdin);
    let wait_deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match child.try_wait().map_err(|e| format!("try_wait: {e}"))? {
            Some(code) => {
                acp_line("teardown", &format!("exited code {code:?}"));
                break;
            }
            None if Instant::now() < wait_deadline => {
                thread::sleep(Duration::from_millis(50))
            }
            None => {
                let _ = child.kill();
                let _ = child.wait();
                acp_line("teardown", "terminated after stdin close timeout");
                break;
            }
        }
    }
    let stderr_len = stderr_bytes.join().map_err(|_| "stderr join".to_string())?;
    acp_line("teardown", &format!("stderr drained {stderr_len} bytes"));
    Ok(())
}
