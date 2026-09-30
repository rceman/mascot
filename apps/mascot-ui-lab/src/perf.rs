//! `perf --out FILE.json [--runs N]`: startup, frame cost, idle behaviour and
//! churn/leak metrics on a REAL visible DComp window with a hardware device
//! (release build required — refuses debug builds).
//!
//! Each `--runs` iteration spawns a fresh `perf-child` process so cold-start
//! numbers are real; medians are aggregated here. The child pumps a real
//! message loop so idle counts count actual Presents/WM_PAINTs.

use mascot_render_win32::renderer::DeviceKind;
use mascot_ui::state::Surface;
use mascot_ui_win32::app::{self, App};
use serde_json::json;
use std::time::Instant;

pub fn run() -> Result<(), String> {
    if cfg!(debug_assertions) {
        return Err("perf requires a release build (cargo build --release)".into());
    }
    let out = crate::get_arg("--out")
        .map(std::path::PathBuf::from)
        .ok_or("perf needs --out FILE.json")?;
    let runs: u32 = crate::get_arg("--runs")
        .and_then(|v| v.parse().ok())
        .unwrap_or(3);
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;

    let mut samples: Vec<serde_json::Value> = Vec::new();
    for i in 0..runs {
        let tmp = std::env::temp_dir().join(format!("mascot-ui-perf-child-{i}.json"));
        let _ = std::fs::remove_file(&tmp);
        let status = std::process::Command::new(&exe)
            .args(["perf-child", "--out", &tmp.to_string_lossy()])
            .status()
            .map_err(|e| format!("spawn: {e}"))?;
        if !status.success() {
            return Err(format!("perf-child run {i} failed ({status})"));
        }
        let v: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&tmp).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        samples.push(v);
    }

    // aggregate: median of each run's scalar metric (raw runs preserved)
    let keys: Vec<String> = samples
        .iter()
        .flat_map(|s| s["metrics"].as_object().unwrap().keys().cloned())
        .collect();
    let mut agg = serde_json::Map::new();
    for k in keys {
        let mut vals: Vec<f64> = samples
            .iter()
            .filter_map(|s| s["metrics"].get(&k).and_then(|v| v.as_f64()))
            .collect();
        if vals.is_empty() {
            continue;
        }
        vals.sort_by(|a, b| a.partial_cmp(b).unwrap());
        agg.insert(k.clone(), json!(vals[vals.len() / 2]));
    }
    let doc = json!({
        "runs": runs,
        "env": samples[0]["env"].clone(),
        "stages_last_run": samples.last().unwrap()["stages"].clone(),
        "medians": agg,
        "raw_runs": samples,
    });
    std::fs::write(&out, serde_json::to_string_pretty(&doc).unwrap()).map_err(|e| e.to_string())?;
    println!("perf: {} runs -> {}", runs, out.display());
    Ok(())
}

// ---------------------------------------------------------------------------
// process sampling
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct Snap {
    private_bytes: u64,
    working_set: u64,
    handles: u32,
    gdi: u32,
    user: u32,
    threads: u32,
}

fn snap() -> Snap {
    use windows::Win32::System::Diagnostics::ToolHelp::*;
    use windows::Win32::System::ProcessStatus::*;
    use windows::Win32::System::Threading::*;
    unsafe {
        let proc = GetCurrentProcess();
        let mut c: PROCESS_MEMORY_COUNTERS_EX = std::mem::zeroed();
        c.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
        let _ = GetProcessMemoryInfo(proc, &mut c as *mut _ as *mut PROCESS_MEMORY_COUNTERS, c.cb);
        let mut handles = 0u32;
        let _ = GetProcessHandleCount(proc, &mut handles);
        let gdi = GetGuiResources(proc, GR_GDIOBJECTS);
        let user = GetGuiResources(proc, GR_USEROBJECTS);
        let mut threads = 0u32;
        if let Ok(s) = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) {
            let mut te = THREADENTRY32 {
                dwSize: std::mem::size_of::<THREADENTRY32>() as u32,
                ..Default::default()
            };
            let pid = GetCurrentProcessId();
            if Thread32First(s, &mut te).is_ok() {
                loop {
                    if te.th32OwnerProcessID == pid {
                        threads += 1;
                    }
                    if Thread32Next(s, &mut te).is_err() {
                        break;
                    }
                }
            }
            let _ = windows::Win32::Foundation::CloseHandle(s);
        }
        Snap {
            private_bytes: c.PrivateUsage as u64,
            working_set: c.WorkingSetSize as u64,
            handles,
            gdi,
            user,
            threads,
        }
    }
}

fn snap_json(s: &Snap) -> serde_json::Value {
    json!({
        "private_bytes": s.private_bytes,
        "working_set": s.working_set,
        "handles": s.handles,
        "gdi": s.gdi,
        "user": s.user,
        "threads": s.threads,
    })
}

/// (kernel+user) CPU time in seconds for this process.
fn cpu_secs() -> f64 {
    use windows::Win32::Foundation::FILETIME;
    use windows::Win32::System::Threading::*;
    unsafe {
        let mut c = FILETIME::default();
        let mut e = FILETIME::default();
        let mut k = FILETIME::default();
        let mut u = FILETIME::default();
        if GetProcessTimes(GetCurrentProcess(), &mut c, &mut e, &mut k, &mut u).is_err() {
            return 0.0;
        }
        let ft = |t: &FILETIME| ((t.dwHighDateTime as u64) << 32) | t.dwLowDateTime as u64;
        (ft(&k) + ft(&u)) as f64 * 1e-7
    }
}

/// Process creation time as a FILETIME (100-ns ticks since epoch).
fn creation_filetime() -> u64 {
    use windows::Win32::Foundation::FILETIME;
    use windows::Win32::System::Threading::*;
    unsafe {
        let mut c = FILETIME::default();
        let mut e = FILETIME::default();
        let mut k = FILETIME::default();
        let mut u = FILETIME::default();
        if GetProcessTimes(GetCurrentProcess(), &mut c, &mut e, &mut k, &mut u).is_err() {
            return 0;
        }
        ((c.dwHighDateTime as u64) << 32) | c.dwLowDateTime as u64
    }
}

fn filetime_now() -> u64 {
    unsafe {
        let ft = windows::Win32::System::SystemInformation::GetSystemTimeAsFileTime();
        ((ft.dwHighDateTime as u64) << 32) | ft.dwLowDateTime as u64
    }
}

fn logical_cores() -> f64 {
    use windows::Win32::System::SystemInformation::*;
    unsafe {
        let mut si = SYSTEM_INFO::default();
        GetSystemInfo(&mut si);
        (si.dwNumberOfProcessors as f64).max(1.0)
    }
}

/// Pump the thread queue for `ms`, counting wall time. Returns elapsed secs.
fn pump_for(ms: u64) -> f64 {
    let t0 = Instant::now();
    while t0.elapsed().as_millis() < ms as u128 {
        if !app::pump_once() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    t0.elapsed().as_secs_f64()
}

fn cpu_name() -> String {
    std::process::Command::new("reg")
        .args([
            "query",
            "HKLM\\HARDWARE\\DESCRIPTION\\System\\CentralProcessor\\0",
            "/v",
            "ProcessorNameString",
        ])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| {
            s.lines()
                .find(|l| l.contains("ProcessorNameString"))
                .map(|l| l.split("REG_SZ").nth(1).unwrap_or("").trim().to_string())
        })
        .unwrap_or_default()
}

fn gpu_name(app: *const App) -> String {
    unsafe {
        let app = &*app;
        windows::core::Interface::cast::<windows::Win32::Graphics::Dxgi::IDXGIDevice>(
            &app.renderer.d3d,
        )
        .and_then(|d| d.GetAdapter())
        .and_then(|a| {
            let desc = a.GetDesc()?;
            Ok::<_, windows::core::Error>(
                desc.Description
                    .iter()
                    .take_while(|c| **c != 0)
                    .map(|c| *c as u8 as char)
                    .collect::<String>(),
            )
        })
        .unwrap_or_default()
    }
}

fn git(args: &[&str]) -> String {
    let root = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| ".".into());
    std::process::Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

fn git_dirty() -> bool {
    std::process::Command::new("git")
        .args([
            "status",
            "--porcelain",
            "--",
            ".",
            ":(exclude)benchmark/results",
        ])
        .output()
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(true)
}

// ---------------------------------------------------------------------------
// the measured child
// ---------------------------------------------------------------------------

pub fn run_child() -> Result<(), String> {
    if cfg!(debug_assertions) {
        return Err("perf requires a release build (cargo build --release)".into());
    }
    let out = crate::get_arg("--out")
        .map(std::path::PathBuf::from)
        .ok_or("perf-child needs --out")?;

    let created_ft = creation_filetime();
    let proc_start = Instant::now();
    let mut stages: Vec<(String, Snap, f64)> = Vec::new();
    fn stage(stages: &mut Vec<(String, Snap, f64)>, name: &'static str, t0: Instant) {
        stages.push((
            name.to_string(),
            snap(),
            t0.elapsed().as_secs_f64() * 1000.0,
        ));
    }

    let t0 = Instant::now();
    let rig = crate::find_rig()?;
    let rig_ms = t0.elapsed().as_secs_f64() * 1000.0;

    let mut app = {
        let stages_ref = &mut stages;
        App::new_instrumented(DeviceKind::Hardware, rig, Default::default(), 1.0, |s| {
            stage(stages_ref, s, proc_start);
        })
        .map_err(|e| format!("app: {e}"))?
    };
    app.response_delay_ms = 0;
    app.open_composer();
    app.unfocus_editor();

    // real visible window, bottom-right of the primary work area
    let (ax, ay);
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::*;
        let mut rc = windows::Win32::Foundation::RECT::default();
        let _ = SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some(&mut rc as *mut _ as *mut std::ffi::c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
        ax = rc.right - 60;
        ay = rc.bottom - 60;
    }
    let app = Box::leak(Box::new(app));
    let app_ptr: *mut App = app;

    /// Fresh app borrow for a single statement. `*app` is also written by
    /// the wndproc through `APP_PTR` inside `DispatchMessageW`, so a `&mut`
    /// bound across a pump gives LLVM a `noalias` license to cache field
    /// loads. Never bind the result across `pump_*`/`pump_once` calls.
    fn am<'x>(app: *mut App) -> &'x mut App {
        unsafe { &mut *app }
    }
    // Non-activating window: the idle measurement is "composer open,
    // UNFOCUSED" — a WS_VISIBLE window would take keyboard focus (timing of
    // the WM_SETFOCUS is what made idle presents flaky).
    let hwnd = app::create_opts(app, ax, ay, true).map_err(|e| e.to_string())?;
    let app = app_ptr;
    // present the first frame now and measure creation->first Present+Commit
    while am(app).dirty {
        am(app).present_if_dirty().map_err(|e| e.to_string())?;
        app::pump_once();
    }
    let first_show_ms = (filetime_now().saturating_sub(created_ft)) as f64 / 10_000.0;
    stage(&mut stages, "first_show", proc_start);
    // GPU heap trim: IDXGIDevice3::Trim + ID2D1Device::ClearResources —
    // releases driver-side heaps built during init/sprite/first frame.
    am(app).trim_gpu();
    stage(&mut stages, "after_gpu_trim", proc_start);
    // drain creation/show-time editor events so the idle window starts clean
    am(app).process_editor_events();
    am(app).dirty = false;
    am(app).dirty_reasons.clear();
    let cores = logical_cores();

    // -- static idle 10 s, composer open, unfocused --------------------------
    let (p0, w0, c0) = (am(app).present_count, am(app).paint_count, cpu_secs());
    let r0 = am(app).present_reasons.len();
    let secs = pump_for(10_000);
    let idle_presents = am(app).present_count - p0;
    let idle_paints = am(app).paint_count - w0;
    let idle_cpu_pct = (cpu_secs() - c0) / (secs * cores) * 100.0;
    let idle_reasons: Vec<String> = am(app).present_reasons[r0..].to_vec();

    // -- focused idle: blink phase, then post-timeout quiet ------------------
    am(app).focus_editor();
    app::pump_once();
    let blink_p0 = am(app).present_count;
    let blink_r0 = am(app).present_reasons.len();
    pump_for(2_200); // blink phase: ~4 caret toggles at the 530ms default
    let blink_frames = am(app).present_count - blink_p0;
    let blink_reasons: Vec<String> = am(app).present_reasons[blink_r0..].to_vec();
    // wait for the caret timeout to freeze the blink (default 5 s)
    let mut timeout_ms = 5000u32;
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::*;
        let _ = SystemParametersInfoW(
            SPI_GETCARETTIMEOUT,
            0,
            Some(&mut timeout_ms as *mut u32 as *mut std::ffi::c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
    }
    pump_for(timeout_ms as u64 + 800);
    let quiet_p0 = am(app).present_count;
    let quiet_r0 = am(app).present_reasons.len();
    let quiet_secs = pump_for(10_000);
    let quiet_presents = am(app).present_count - quiet_p0;
    let quiet_reasons: Vec<String> = am(app).present_reasons[quiet_r0..].to_vec();
    let _ = quiet_secs;

    // -- theme switch x20 (state change -> present returned) ------------------
    let mut theme_ms = Vec::with_capacity(20);
    for _ in 0..20 {
        am(app).state.theme = am(app).state.theme.other();
        let t = Instant::now();
        am(app).apply_theme();
        am(app).present_if_dirty().map_err(|e| e.to_string())?;
        theme_ms.push(t.elapsed().as_secs_f64() * 1000.0);
        app::pump_once();
    }
    theme_ms.sort_by(|a, b| a.partial_cmp(b).unwrap());

    // -- submit -> response x20 ----------------------------------------------
    let mut submit_ms = Vec::with_capacity(20);
    for _ in 0..20 {
        let _ = am(app).editor.set_text("perf probe: ship the composer");
        am(app).state.editor_empty = false;
        am(app).focus_editor();
        am(app).process_editor_events();
        let t = Instant::now();
        am(app).submit();
        am(app).response_arrived();
        am(app).present_if_dirty().map_err(|e| e.to_string())?;
        submit_ms.push(t.elapsed().as_secs_f64() * 1000.0);
        am(app).state.surface = Surface::Composer;
        app::pump_once();
    }
    submit_ms.sort_by(|a, b| a.partial_cmp(b).unwrap());

    // -- open/close x100 warm show -------------------------------------------
    let res_before = snap();
    let mut open_ms = Vec::with_capacity(100);
    for _ in 0..100 {
        am(app).escape();
        app::pump_once();
        let t = Instant::now();
        am(app).open_composer();
        while am(app).dirty {
            am(app).present_if_dirty().map_err(|e| e.to_string())?;
            app::pump_once();
        }
        open_ms.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    open_ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let res_after_100 = snap();

    // -- growth: 1000 open/close cycles, sampled every 100 --------------------
    let mut growth: Vec<serde_json::Value> = Vec::new();
    for i in 0..1000 {
        am(app).escape();
        am(app).open_composer();
        while am(app).dirty {
            am(app).present_if_dirty().map_err(|e| e.to_string())?;
            app::pump_once();
        }
        if (i + 1) % 100 == 0 {
            let s = snap();
            growth.push(json!({
                "cycle": i + 1,
                "private_bytes": s.private_bytes,
                "working_set": s.working_set,
                "handles": s.handles,
                "gdi": s.gdi,
                "user": s.user,
                "threads": s.threads,
            }));
        }
    }
    // plateau check: mean private bytes of first vs last 100-cycle window
    let (first, last) = (
        &growth[..2.min(growth.len())],
        &growth[growth.len().saturating_sub(2)..],
    );
    let mean = |v: &[serde_json::Value]| -> f64 {
        v.iter()
            .filter_map(|x| x["private_bytes"].as_f64())
            .sum::<f64>()
            / v.len().max(1) as f64
    };
    let (first_mean, last_mean) = (mean(first), mean(last));
    let plateau = last_mean - first_mean < 4.0 * 1024.0 * 1024.0;

    let env = json!({
        "os_build": crate::capture::os_build(),
        "cpu": cpu_name(),
        "gpu": gpu_name(app),
        "device": "hardware",
        "scale": 1.0,
        "head": git(&["rev-parse", "HEAD"]),
        "dirty": git_dirty(),
        "profile": if cfg!(debug_assertions) { "dev" } else { "release" },
        "font": am(app).painter.fonts.family,
    });
    let metrics = json!({
        "rig_load_ms": rig_ms,
        "first_show_ms": first_show_ms,
        "idle_presents_10s": idle_presents,
        "idle_present_reasons": idle_reasons,
        "idle_paint_msgs_10s": idle_paints,
        "idle_cpu_pct": idle_cpu_pct,
        "focused_blink_presents": blink_frames,
        "focused_blink_reasons": blink_reasons,
        "focused_quiet_presents_10s": quiet_presents,
        "focused_quiet_reasons": quiet_reasons,
        "theme_switch_ms_p50": theme_ms[theme_ms.len() / 2],
        "theme_switch_ms_p95": theme_ms[(theme_ms.len() * 95 / 100).min(theme_ms.len() - 1)],
        "submit_response_ms_p50": submit_ms[submit_ms.len() / 2],
        "submit_response_ms_p95": submit_ms[(submit_ms.len() * 95 / 100).min(submit_ms.len() - 1)],
        "open_warm_ms_p50": open_ms[open_ms.len() / 2],
        "open_warm_ms_p95": open_ms[(open_ms.len() * 95 / 100).min(open_ms.len() - 1)],
        "growth_plateau_after_1000": plateau,
        "growth_private_first_mean": first_mean,
        "growth_private_last_mean": last_mean,
    });
    let stage_json: Vec<serde_json::Value> = stages
        .iter()
        .map(|(n, s, t)| {
            let mut o = snap_json(s);
            o["stage"] = json!(n);
            o["t_ms"] = json!(t);
            o
        })
        .collect();
    let out_json = json!({
        "env": env,
        "metrics": metrics,
        "stages": stage_json,
        "open_close_100": {"before": snap_json(&res_before), "after": snap_json(&res_after_100)},
        "growth_samples": growth,
    });
    std::fs::write(&out, serde_json::to_string_pretty(&out_json).unwrap())
        .map_err(|e| e.to_string())?;

    // close the window
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd);
        while !app::pump_once() {}
    }
    app::unbind();
    Ok(())
}
