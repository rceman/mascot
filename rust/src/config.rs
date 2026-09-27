use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
pub struct Manifest {
    pub version: String,
    pub asset: Asset,
    pub provider: ProviderSpec,
    pub protocol: Protocol,
    pub scenarios: BTreeMap<String, serde_json::Value>,
    pub ui: Ui,
}

#[derive(Debug, Deserialize)]
pub struct Asset {
    pub path: String,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub logical_width_dip: u32,
    pub logical_height_dip: u32,
    pub sha256: String,
}

#[derive(Debug, Deserialize)]
pub struct ProviderSpec {
    pub path: String,
    pub arguments: Vec<String>,
    pub cwd: String,
    pub environment: BTreeMap<String, String>,
    pub inherit_environment: bool,
}

#[derive(Debug, Deserialize)]
pub struct Protocol {
    pub max_stdout_frame_bytes_including_lf: u64,
    pub max_stdin_frame_bytes_including_lf: u64,
    pub cancel_timeout_ms: u64,
    pub shutdown_timeout_ms: u64,
    pub normal_chunks: u64,
}

#[derive(Debug, Deserialize)]
pub struct Ui {
    pub composer_client_width_dip: u32,
    pub composer_client_height_dip: u32,
    pub input_height_dip: u32,
    pub response_height_dip: u32,
    pub margin_dip: u32,
    pub hotkey: String,
    pub submit: String,
    pub cancel: String,
    pub input_limit_utf16_units: u32,
    pub response_limit_utf8_bytes: u32,
    pub history_messages: u32,
    pub hide_cancels_request: bool,
}

pub struct Config {
    pub manifest: Manifest,
    pub asset_path: PathBuf,
    pub history_prefix: String,
}

fn sha256(data: &[u8]) -> Result<[u8; 32], String> {
    use windows_sys::Win32::Security::Cryptography::*;
    let length = u32::try_from(data.len()).map_err(|_| "hash input too large")?;
    let mut algorithm = std::ptr::null_mut();
    let mut digest = [0u8; 32];
    unsafe {
        let status = BCryptOpenAlgorithmProvider(
            &mut algorithm,
            BCRYPT_SHA256_ALGORITHM,
            std::ptr::null(),
            0,
        );
        if status < 0 {
            return Err(format!("BCryptOpenAlgorithmProvider: {status:#x}"));
        }
        let status = BCryptHash(
            algorithm,
            std::ptr::null(),
            0,
            data.as_ptr(),
            length,
            digest.as_mut_ptr(),
            32,
        );
        let closed = BCryptCloseAlgorithmProvider(algorithm, 0);
        if status < 0 || closed < 0 {
            return Err(format!("BCryptHash/close: {status:#x}/{closed:#x}"));
        }
    }
    Ok(digest)
}

fn sha256_hex(path: &Path) -> Result<String, String> {
    let data = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let digest = sha256(&data)?;
    Ok(hex_lower(&digest))
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0xf) as usize] as char);
    }
    out
}

fn expected_chunks(scenario: &str, manifest: &Manifest) -> u64 {
    match scenario {
        "backpressure" => 256,
        "maximum" | "oversized" => 1,
        _ => manifest.protocol.normal_chunks,
    }
}

pub fn scenario_chunks(manifest: &Manifest, scenario: &str) -> Result<u64, String> {
    if !manifest.scenarios.contains_key(scenario) {
        return Err(format!("unknown scenario '{scenario}'"));
    }
    Ok(expected_chunks(scenario, manifest))
}

pub fn load(manifest_arg: &str) -> Result<Config, String> {
    let manifest_path = PathBuf::from(manifest_arg);
    let manifest_path = manifest_path
        .canonicalize()
        .map_err(|e| format!("manifest path: {e}"))?;
    let data = std::fs::read(&manifest_path).map_err(|e| format!("read manifest: {e}"))?;
    let manifest: Manifest =
        serde_json::from_slice(&data).map_err(|e| format!("parse manifest: {e}"))?;
    let manifest_dir = manifest_path
        .parent()
        .ok_or_else(|| "manifest has no parent".to_string())?;
    let root = manifest_dir
        .parent()
        .ok_or_else(|| "manifest has no benchmark root".to_string())?
        .to_path_buf();

    if manifest.asset.pixel_width != 128 || manifest.asset.pixel_height != 128 {
        return Err("unsupported asset pixel size".into());
    }
    if manifest.asset.logical_width_dip != 64 || manifest.asset.logical_height_dip != 64 {
        return Err("unsupported asset logical size".into());
    }
    if manifest.protocol.max_stdout_frame_bytes_including_lf != 65536
        || manifest.protocol.max_stdin_frame_bytes_including_lf != 65536
    {
        return Err("unsupported frame limits".into());
    }
    if manifest.provider.inherit_environment {
        return Err("provider must not inherit environment".into());
    }
    if !manifest.version.starts_with("windows-v") {
        return Err(format!("unsupported fixture version {}", manifest.version));
    }
    let ui = &manifest.ui;
    if ui.composer_client_width_dip != 640
        || ui.composer_client_height_dip != 480
        || ui.input_height_dip != 128
        || ui.response_height_dip != 272
        || ui.margin_dip != 12
    {
        return Err("unsupported composer geometry".into());
    }
    if ui.hotkey != "CTRL+ALT+SPACE" || ui.submit != "CTRL+ENTER" || ui.cancel != "CTRL+ALT+ESCAPE"
    {
        return Err("unsupported hotkey mapping".into());
    }
    if ui.hide_cancels_request {
        return Err("hide must not cancel the provider request".into());
    }
    for required in [
        "normal",
        "cancel",
        "client_request",
        "backpressure",
        "maximum",
        "oversized",
        "unexpected_exit",
        "stderr",
        "fragmented",
    ] {
        if !manifest.scenarios.contains_key(required) {
            return Err(format!("manifest missing scenario '{required}'"));
        }
    }

    let asset_path = root.join(manifest.asset.path.replace('/', "\\"));
    let actual = sha256_hex(&asset_path)?;
    if !actual.eq_ignore_ascii_case(&manifest.asset.sha256) {
        return Err(format!("asset sha256 mismatch: {actual}"));
    }

    let history_path = root.join("fixtures").join("history.json");
    let history_data = std::fs::read(&history_path).map_err(|e| format!("read history: {e}"))?;
    let history: Vec<String> =
        serde_json::from_slice(&history_data).map_err(|e| format!("parse history: {e}"))?;
    if history.len() != manifest.ui.history_messages as usize {
        return Err("history length does not match manifest".into());
    }
    let mut history_prefix = history.join("\n");
    history_prefix.push_str("\n\n");

    Ok(Config {
        manifest,
        asset_path,
        history_prefix,
    })
}

#[cfg(test)]
mod tests {
    use super::{hex_lower, sha256};

    #[test]
    fn sha256_known_vectors() {
        assert_eq!(
            hex_lower(&sha256(b"").unwrap()),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            hex_lower(&sha256(b"abc").unwrap()),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
