//! In-process resource sampling (CPU time, memory, threads, handles, GDI/USER objects).

use mascot_render_win32::Renderer;
use serde_json::{Value, json};
use std::time::Instant;
use windows::Win32::Foundation::{CloseHandle, FILETIME};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::System::Diagnostics::ToolHelp::*;
use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX};
use windows::Win32::System::Threading::*;
use windows::core::Interface;

#[derive(Debug, Clone)]
pub struct Sample {
    pub at: Instant,
    pub cpu_100ns: u64,
    pub working_set: u64,
    pub private_bytes: u64,
    pub threads: u32,
    pub handles: u32,
    pub gdi: u32,
    pub user: u32,
    pub frames: u64,
}

fn ft(f: FILETIME) -> u64 {
    ((f.dwHighDateTime as u64) << 32) | f.dwLowDateTime as u64
}

impl Sample {
    pub fn now(frames: u64) -> Sample {
        unsafe {
            let proc = GetCurrentProcess();
            let (mut c, mut e, mut k, mut u) = (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
            let _ = GetProcessTimes(proc, &mut c, &mut e, &mut k, &mut u);
            let mut mem = PROCESS_MEMORY_COUNTERS_EX { cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32, ..Default::default() };
            let _ = GetProcessMemoryInfo(proc, &mut mem as *mut _ as *mut PROCESS_MEMORY_COUNTERS, mem.cb);
            let mut handles = 0u32;
            let _ = GetProcessHandleCount(proc, &mut handles);
            let pid = GetCurrentProcessId();
            let mut threads = 0;
            if let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) {
                let mut te = THREADENTRY32 { dwSize: std::mem::size_of::<THREADENTRY32>() as u32, ..Default::default() };
                if Thread32First(snap, &mut te).is_ok() {
                    loop {
                        if te.th32OwnerProcessID == pid {
                            threads += 1;
                        }
                        if Thread32Next(snap, &mut te).is_err() {
                            break;
                        }
                    }
                }
                let _ = CloseHandle(snap);
            }
            Sample {
                at: Instant::now(),
                cpu_100ns: ft(k) + ft(u),
                working_set: mem.WorkingSetSize as u64,
                private_bytes: mem.PrivateUsage as u64,
                threads,
                handles,
                gdi: GetGuiResources(proc, GR_GDIOBJECTS),
                user: GetGuiResources(proc, GR_USEROBJECTS),
                frames,
            }
        }
    }

    pub fn to_json(&self) -> Value {
        json!({
            "working_set_mib": self.working_set as f64 / 1048576.0,
            "private_mib": self.private_bytes as f64 / 1048576.0,
            "threads": self.threads, "handles": self.handles, "gdi_objects": self.gdi, "user_objects": self.user,
            "frames_total": self.frames,
        })
    }
}

/// CPU% of one logical core and frame count between two samples.
pub fn window_json(a: &Sample, b: &Sample) -> Value {
    let secs = (b.at - a.at).as_secs_f64();
    let cpu_s = (b.cpu_100ns - a.cpu_100ns) as f64 / 1e7;
    json!({
        "seconds": secs,
        "cpu_seconds": cpu_s,
        "cpu_percent_of_one_core": if secs > 0.0 { 100.0 * cpu_s / secs } else { 0.0 },
        "frames_rendered": b.frames - a.frames,
        "fps": if secs > 0.0 { (b.frames - a.frames) as f64 / secs } else { 0.0 },
        "end": b.to_json(),
    })
}

pub fn adapter_name(r: &Renderer) -> String {
    unsafe {
        let Ok(dev) = r.d3d.cast::<IDXGIDevice>() else { return String::new() };
        let Ok(adapter) = dev.GetAdapter() else { return String::new() };
        let Ok(desc) = adapter.GetDesc() else { return String::new() };
        let end = desc.Description.iter().position(|&c| c == 0).unwrap_or(desc.Description.len());
        String::from_utf16_lossy(&desc.Description[..end])
    }
}
