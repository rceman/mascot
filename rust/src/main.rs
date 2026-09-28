#![cfg_attr(all(not(test), windows), windows_subsystem = "windows")]

mod codex_gate;
mod config;
mod control;
mod framing;
mod platform;
mod provider;
mod queue;
#[cfg(windows)]
#[path = "text_windows.rs"]
mod text;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use serde::Deserialize;
use serde_json::json;

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
                Some(path) => platform::run_app(path, control),
                None => Err("missing manifest path".into()),
            }
        }
        Some("--codex-gate") => match args.get(2) {
            Some(path) => Ok(codex_gate::run(path)),
            None => Err("missing codex executable path".into()),
        },
        Some("--acp-gate") => match args.get(2) {
            Some(path) => Ok(codex_gate::run_acp(path)),
            None => Err("missing ACP executable path".into()),
        },
        _ => Err("usage: mascot --fixture MANIFEST [--control] | --decode-vectors VECTORS | --codex-gate CODEXE | --acp-gate ACPEXE".into()),
    };
    match code {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(64);
        }
    }
}
