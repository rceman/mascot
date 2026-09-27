use std::path::{Path, PathBuf};
use std::process::Command;

fn rc_exe() -> PathBuf {
    let kits = Path::new(r"C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\rc.exe");
    if kits.is_file() {
        return kits.to_path_buf();
    }
    let base = Path::new(r"C:\Program Files (x86)\Windows Kits\10\bin");
    let mut found = None;
    if let Ok(entries) = std::fs::read_dir(base) {
        for entry in entries.flatten() {
            let candidate = entry.path().join("x64").join("rc.exe");
            if candidate.is_file() {
                found = Some(candidate);
            }
        }
    }
    found.unwrap_or_else(|| panic!("rc.exe not found under {}", base.display()))
}

fn main() {
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR");
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let output = Path::new(&out_dir).join("app.res");
    let status = Command::new(rc_exe())
        .current_dir(&manifest_dir)
        .args(["/nologo"])
        .arg("/fo")
        .arg(&output)
        .arg("app.rc")
        .status()
        .expect("failed to launch rc.exe");
    if !status.success() {
        panic!("rc.exe failed: {status}");
    }
    println!("cargo:rerun-if-changed=app.rc");
    println!("cargo:rerun-if-changed=app.manifest");
    println!("cargo:rustc-link-arg-bin=mascot={}", output.display());
}
