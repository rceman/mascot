use std::path::{Path, PathBuf};
use std::process::Command;

fn rc_exe() -> PathBuf {
    let rc = Path::new(r"C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\rc.exe");
    assert!(
        rc.is_file(),
        "pinned Windows SDK 10.0.26100.0 rc.exe not found at {}",
        rc.display()
    );
    rc.to_path_buf()
}

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
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
