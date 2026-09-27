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

fn sha256_hex(path: &Path) -> Result<String, String> {
    let data = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let digest = Sha256::digest(&data);
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

// Minimal SHA-256 (candidate-owned; avoids a hash crate just for fixture integrity).
struct Sha256 {
    state: [u32; 8],
    buf: [u8; 64],
    buf_len: usize,
    total: u64,
}

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

impl Sha256 {
    fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            buf: [0u8; 64],
            buf_len: 0,
            total: 0,
        }
    }

    fn block(&mut self, block: &[u8]) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                block[i * 4],
                block[i * 4 + 1],
                block[i * 4 + 2],
                block[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.state;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        self.state = [
            self.state[0].wrapping_add(a),
            self.state[1].wrapping_add(b),
            self.state[2].wrapping_add(c),
            self.state[3].wrapping_add(d),
            self.state[4].wrapping_add(e),
            self.state[5].wrapping_add(f),
            self.state[6].wrapping_add(g),
            self.state[7].wrapping_add(h),
        ];
    }

    fn update(&mut self, mut data: &[u8]) {
        self.total += data.len() as u64;
        if self.buf_len > 0 {
            let take = (64 - self.buf_len).min(data.len());
            self.buf[self.buf_len..self.buf_len + take].copy_from_slice(&data[..take]);
            self.buf_len += take;
            data = &data[take..];
            if self.buf_len == 64 {
                let block = self.buf;
                self.block(&block);
                self.buf_len = 0;
            }
            if data.is_empty() {
                return;
            }
        }
        while data.len() >= 64 {
            let (block, rest) = data.split_at(64);
            self.block(block);
            data = rest;
        }
        if !data.is_empty() {
            self.buf[..data.len()].copy_from_slice(data);
            self.buf_len = data.len();
        }
    }

    fn finish(mut self) -> [u8; 32] {
        let bit_len = self.total * 8;
        self.update(&[0x80]);
        while self.buf_len != 56 {
            self.update(&[0]);
        }
        self.update(&bit_len.to_be_bytes());
        let mut out = [0u8; 32];
        for (i, word) in self.state.iter().enumerate() {
            out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
        }
        out
    }

    fn digest(data: &[u8]) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(data);
        h.finish()
    }
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
    use super::{Sha256, hex_lower};

    #[test]
    fn sha256_known_vectors() {
        assert_eq!(
            hex_lower(&Sha256::digest(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            hex_lower(&Sha256::digest(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let long = vec![b'a'; 1000];
        let mut h = Sha256::new();
        for chunk in long.chunks(37) {
            h.update(chunk);
        }
        let digest = hex_lower(&h.finish());
        let mut single = Sha256::new();
        single.update(&long);
        assert_eq!(digest, hex_lower(&single.finish()));
    }
}
