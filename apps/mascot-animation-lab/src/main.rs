//! Mascot rig v0.2 animation lab (developer tooling, not product UI).
//!
//! Rendering model: frames are produced only from WM_PAINT. A static mascot is
//! painted once and the thread then blocks in GetMessageW. While a clip plays,
//! each paint advances the player, presents (vsync-paced) and re-invalidates;
//! when the player reports static the invalidation chain stops. The idle
//! director uses a one-shot timer to wake for the next clip.
//!
//! CLI: mascot-animation-lab [--rig DIR] [--warp] [--perf OUT.json] [--capture DIR]

#![cfg_attr(not(test), windows_subsystem = "windows")]

mod metrics;

use mascot_animation::idle::{IdleAction, IdleDirector};
use mascot_animation::{BoundClip, Player, PlayerEvent, Pose, Rig, RootPlacement};
use mascot_render_win32::image::RgbaImage;
use mascot_render_win32::swapchain::WindowTarget;
use mascot_render_win32::{DeviceKind, RenderOptions, Renderer, View};
use serde_json::json;
use std::cell::RefCell;
use std::path::PathBuf;
use std::time::Instant;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::*;
use windows::Win32::UI::HiDpi::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{HSTRING, PCWSTR, w};

const ID_CLIP: i32 = 100;
const ID_RESET: i32 = 200;
const ID_SHOT: i32 = 201;
const ID_CHECK: i32 = 300;
const ID_BONES: i32 = 400;
const ID_SLIDER: i32 = 401;
const TIMER_IDLE: usize = 1;
const TIMER_SCRIPT: usize = 2;
const PANEL_W: i32 = 270;
/// commctrl.h: TBM_GETPOS = WM_USER (not exported by the bindings)
const TBM_GETPOS: u32 = WM_USER;

const CHECKS: &[&str] = &["Mirror (root)", "Outline", "Shadow", "Bone overlay", "Pivots", "Bounds", "Bone labels", "Idle director"];
const MAJOR_BONES: &[&str] = &[
    "head", "ear_near", "ear_far", "arm_near_upper", "arm_far_upper", "leg_near_upper", "leg_far_upper", "tail",
    "neck", "chest", "body", "laptop_screen",
];

#[derive(Clone, Copy, PartialEq)]
enum ScriptKind {
    Perf,
    Capture,
}

struct Script {
    kind: ScriptKind,
    step: u32,
    out: PathBuf,
    samples: Vec<(String, metrics::Sample)>,
    queue: Vec<usize>,
    active_from: Option<Instant>,
    frame_times: Vec<f64>,
    frames_at: Vec<(String, u64)>,
}

struct App {
    main: HWND,
    canvas: HWND,
    list: HWND,
    slider: HWND,
    value: HWND,
    status: HWND,
    checks: Vec<HWND>,
    rig: Rig,
    clips: Vec<BoundClip>,
    renderer: Renderer,
    target: Option<WindowTarget>,
    player: Player,
    manual: Vec<f32>,
    pose: Pose,
    opts: RenderOptions,
    mirror: bool,
    selected: usize,
    last_frame: Option<Instant>,
    frames: u64,
    last_render_ms: f64,
    started: Instant,
    idle: IdleDirector,
    idle_enabled: bool,
    script: Option<Script>,
    font: HFONT,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP.with(|a| a.try_borrow_mut().ok().and_then(|mut g| g.as_mut().map(f)))
}

fn loword(v: usize) -> i32 {
    (v & 0xffff) as i32
}
fn hiword(v: usize) -> i32 {
    ((v >> 16) & 0xffff) as i32
}

unsafe fn send(h: HWND, msg: u32, wp: usize, lp: isize) -> LRESULT {
    unsafe { SendMessageW(h, msg, Some(WPARAM(wp)), Some(LPARAM(lp))) }
}

impl App {
    fn bone_by_name(&self, n: &str) -> usize {
        self.rig.skeleton.find(n).expect("bone")
    }

    fn paint(&mut self) {
        let now = Instant::now();
        let mut completed = Vec::new();
        if let Some(t) = self.last_frame {
            let dt = (now - t).as_secs_f32().min(0.1);
            for e in self.player.advance(dt, &self.clips) {
                if let PlayerEvent::Completed(c) = e {
                    completed.push(c);
                }
            }
        }
        self.player.evaluate(&self.clips, &mut self.pose);
        for (i, m) in self.manual.iter().enumerate() {
            self.pose.bones[i].rotation_deg += m;
        }
        let world = self.rig.skeleton.world(&self.pose, RootPlacement { mirror: self.mirror });
        let items = self.rig.draw_list(&world, &self.pose);
        // WM_SIZE can arrive while the app is borrowed (nested SendMessage from
        // MoveWindow), so the swap chain size is reconciled here as well
        let mut rc = RECT::default();
        unsafe {
            let _ = GetClientRect(self.canvas, &mut rc);
        }
        let client = [rc.right.max(1) as u32, rc.bottom.max(1) as u32];
        if let Some(t) = &mut self.target {
            if t.size != client {
                let _ = t.resize(&self.renderer, client);
            }
        }
        if let Some(t) = &self.target {
            let view = View::fit(self.rig.canvas_size(), t.size, 30.0);
            let t0 = Instant::now();
            let bmp = t.bitmap.clone().unwrap();
            let size = t.size;
            if let Err(e) = self.renderer.render(&bmp, size, &self.rig, &items, &world, view, &self.opts) {
                eprintln!("render: {e}");
            }
            let _ = t.present();
            self.last_render_ms = t0.elapsed().as_secs_f64() * 1000.0;
            self.frames += 1;
        }
        let animating = !self.player.is_static();
        if let Some(s) = &mut self.script {
            if s.active_from.is_some() {
                s.frame_times.push(self.started.elapsed().as_secs_f64());
            }
        }
        for c in completed {
            self.on_completed(c);
        }
        if !self.player.is_static() {
            self.last_frame = Some(now);
            unsafe {
                let _ = InvalidateRect(Some(self.canvas), None, false);
            }
        } else {
            self.last_frame = None;
        }
        if animating != !self.player.is_static() || self.frames % 15 == 0 || !animating {
            self.update_status();
        }
    }

    fn update_status(&self) {
        let playing: Vec<&str> = self.player.active().iter().map(|p| self.clips[p.clip].clip.name.as_str()).collect();
        let state = if playing.is_empty() { "STATIC (no frame loop)".to_string() } else { format!("playing: {}", playing.join(", ")) };
        let text = format!(
            "{state}\r\nframes rendered: {}\r\nlast frame GPU submit: {:.2} ms\r\ndevice: {:?}",
            self.frames, self.last_render_ms, self.renderer.kind
        );
        unsafe {
            let _ = SetWindowTextW(self.status, &HSTRING::from(text));
        }
    }

    fn invalidate(&self) {
        unsafe {
            let _ = InvalidateRect(Some(self.canvas), None, false);
        }
    }

    fn play(&mut self, clip: usize) {
        if self.player.is_static() {
            self.last_frame = Some(Instant::now());
        }
        self.player.play(clip);
        self.update_status();
        self.invalidate();
    }

    fn on_completed(&mut self, _clip: usize) {
        if let Some(s) = &mut self.script {
            if s.kind == ScriptKind::Perf && s.step == 2 {
                if let Some(next) = s.queue.pop() {
                    self.player.play(next);
                    return;
                }
            }
        }
        if self.player.is_static() {
            if self.idle_enabled {
                self.schedule_idle();
            }
            if self.script.as_ref().is_some_and(|s| s.kind == ScriptKind::Perf && s.step == 2) {
                self.script_step();
            }
        }
    }

    fn schedule_idle(&mut self) {
        let ms = (self.idle.next_delay() * 1000.0) as u32;
        unsafe {
            SetTimer(Some(self.main), TIMER_IDLE, ms, None);
        }
    }

    fn select_bone(&mut self, bone: usize) {
        self.selected = bone;
        self.opts.selected_bone = Some(bone);
        let [lo, hi] = self.rig.skeleton.bones[bone].safe_rotation_deg;
        unsafe {
            send(self.slider, TBM_SETRANGEMIN, 0, (lo * 10.0) as isize);
            send(self.slider, TBM_SETRANGEMAX, 1, (hi * 10.0) as isize);
            send(self.slider, TBM_SETPOS, 1, (self.manual[bone] * 10.0) as isize);
            if let Some(idx) = MAJOR_BONES.iter().position(|b| *b == self.rig.skeleton.bones[bone].id) {
                send(self.list, LB_SETCURSEL, idx, 0);
            }
        }
        self.update_value();
        self.invalidate();
    }

    fn update_value(&self) {
        let b = &self.rig.skeleton.bones[self.selected];
        let text = format!("{}: {:+.1} deg  (safe {:+}..{:+})", b.id, self.manual[self.selected], b.safe_rotation_deg[0], b.safe_rotation_deg[1]);
        unsafe {
            let _ = SetWindowTextW(self.value, &HSTRING::from(text));
        }
    }

    fn set_manual(&mut self, deg: f32) {
        self.manual[self.selected] = self.rig.clamp_to_safe(self.selected, deg);
        unsafe {
            send(self.slider, TBM_SETPOS, 1, (self.manual[self.selected] * 10.0) as isize);
        }
        self.update_value();
        self.invalidate();
    }

    fn reset(&mut self) {
        self.player.reset();
        self.manual.iter_mut().for_each(|m| *m = 0.0);
        self.last_frame = None;
        self.select_bone(self.selected);
        self.update_status();
        self.invalidate();
    }

    fn apply_checks(&mut self) {
        let on = |h: HWND| unsafe { send(h, BM_GETCHECK, 0, 0).0 == BST_CHECKED.0 as isize };
        self.mirror = on(self.checks[0]);
        self.opts.outline = on(self.checks[1]);
        self.opts.shadow = on(self.checks[2]);
        self.opts.bones = on(self.checks[3]);
        self.opts.pivots = on(self.checks[4]);
        self.opts.bounds = on(self.checks[5]);
        self.opts.labels = on(self.checks[6]);
        let idle = on(self.checks[7]);
        if idle && !self.idle_enabled {
            self.schedule_idle();
        } else if !idle {
            unsafe {
                let _ = KillTimer(Some(self.main), TIMER_IDLE);
            }
        }
        self.idle_enabled = idle;
        self.invalidate();
    }

    fn set_check(&mut self, i: usize, v: bool) {
        unsafe {
            send(self.checks[i], BM_SETCHECK, if v { BST_CHECKED.0 as usize } else { 0 }, 0);
        }
        self.apply_checks();
    }

    fn pick_bone(&mut self, x: i32, y: i32) {
        let Some(t) = &self.target else { return };
        let view = View::fit(self.rig.canvas_size(), t.size, 30.0).affine();
        let world = self.rig.skeleton.world(&self.pose, RootPlacement { mirror: self.mirror });
        let best = MAJOR_BONES
            .iter()
            .map(|n| self.bone_by_name(n))
            .map(|i| {
                let p = view.apply(world[i].apply([0.0, 0.0]));
                (i, (p[0] - x as f32).hypot(p[1] - y as f32))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((i, d)) = best {
            if d < 40.0 {
                self.select_bone(i);
            }
        }
    }

    fn screenshot(&self, path: &std::path::Path) {
        let img = capture_window(self.main);
        let _ = img.save(path);
    }

    // ---- automation (perf measurement / screenshot capture) -----------------
    fn script_step(&mut self) {
        let Some(kind) = self.script.as_ref().map(|s| s.kind) else { return };
        let step = self.script.as_ref().unwrap().step;
        let mut next_ms: Option<u32> = None;
        match (kind, step) {
            (ScriptKind::Perf, 0) => {
                self.sample("idle_start");
                next_ms = Some(10_000);
            }
            (ScriptKind::Perf, 1) => {
                self.sample("idle_end");
                let order = ["stretch", "tail_flick", "look_left", "ear_twitch", "double_blink", "small_head_tilt", "look_right", "posture_adjust"];
                let mut q: Vec<usize> = order.iter().map(|n| self.clips.iter().position(|c| c.clip.name == *n).unwrap()).collect();
                q.reverse();
                let first = q.pop().unwrap();
                let s = self.script.as_mut().unwrap();
                s.queue = q;
                s.active_from = Some(Instant::now());
                s.step = 2;
                self.sample("active_start");
                self.play(first);
                return;
            }
            (ScriptKind::Perf, 2) => {
                self.sample("active_end");
                self.script.as_mut().unwrap().active_from = None;
                next_ms = Some(10_000);
            }
            (ScriptKind::Perf, 3) => {
                self.sample("post_idle_end");
                self.write_perf();
                unsafe { PostQuitMessage(0) };
            }
            (ScriptKind::Capture, 0) => {
                self.shot("lab_rest.png");
                self.set_check(3, true);
                self.set_check(4, true);
                self.set_check(6, true);
                let h = self.bone_by_name("head");
                self.select_bone(h);
                self.set_manual(10.0);
                next_ms = Some(700);
            }
            (ScriptKind::Capture, 1) => {
                self.shot("lab_debug_overlay_head+10.png");
                self.set_check(5, true);
                let a = self.bone_by_name("arm_near_upper");
                self.select_bone(a);
                self.set_manual(-15.0);
                next_ms = Some(700);
            }
            (ScriptKind::Capture, 2) => {
                self.shot("lab_debug_bounds_arm-15.png");
                for i in [3, 4, 5, 6] {
                    self.set_check(i, false);
                }
                self.reset();
                self.set_check(0, true);
                self.set_check(2, true);
                let c = self.clips.iter().position(|c| c.clip.name == "stretch").unwrap();
                self.play(c);
                next_ms = Some(1200);
            }
            (ScriptKind::Capture, 3) => {
                self.shot("lab_mirrored_shadow_mid_stretch.png");
                next_ms = Some(2500);
            }
            (ScriptKind::Capture, 4) => {
                self.shot("lab_mirrored_after_clip_static.png");
                unsafe { PostQuitMessage(0) };
            }
            _ => {}
        }
        let s = self.script.as_mut().unwrap();
        s.step += 1;
        if let Some(ms) = next_ms {
            unsafe {
                SetTimer(Some(self.main), TIMER_SCRIPT, ms, None);
            }
        }
    }

    fn shot(&mut self, name: &str) {
        // make sure the latest state is on screen before capturing
        unsafe {
            let _ = UpdateWindow(self.canvas);
        }
        let out = self.script.as_ref().unwrap().out.join(name);
        self.screenshot(&out);
    }

    fn sample(&mut self, label: &str) {
        let s = metrics::Sample::now(self.frames);
        let sc = self.script.as_mut().unwrap();
        sc.frames_at.push((label.into(), self.frames));
        sc.samples.push((label.into(), s));
    }

    fn write_perf(&self) {
        let s = self.script.as_ref().unwrap();
        let get = |n: &str| s.samples.iter().find(|(l, _)| l == n).map(|(_, v)| v.clone()).unwrap();
        let (i0, i1, a0, a1, p1) = (get("idle_start"), get("idle_end"), get("active_start"), get("active_end"), get("post_idle_end"));
        let intervals: Vec<f64> = s.frame_times.windows(2).map(|w| (w[1] - w[0]) * 1000.0).collect();
        let mut sorted = intervals.clone();
        sorted.sort_by(|a, b| a.total_cmp(b));
        let pct = |p: f64| if sorted.is_empty() { 0.0 } else { sorted[((sorted.len() - 1) as f64 * p).round() as usize] };
        let mean = if intervals.is_empty() { 0.0 } else { intervals.iter().sum::<f64>() / intervals.len() as f64 };
        let report = json!({
            "tool": "mascot-animation-lab --perf",
            "device": format!("{:?}", self.renderer.kind),
            "adapter": metrics::adapter_name(&self.renderer),
            "window_client_px": self.target.as_ref().map(|t| t.size),
            "idle": metrics::window_json(&i0, &i1),
            "active": metrics::window_json(&a0, &a1),
            "post_animation_idle": metrics::window_json(&a1, &p1),
            "frame_cadence_ms": {"frames": intervals.len() + 1, "mean": mean, "p50": pct(0.5), "p95": pct(0.95), "p99": pct(0.99),
                                  "max": sorted.last().copied().unwrap_or(0.0), "min": sorted.first().copied().unwrap_or(0.0)},
            "snapshots": s.samples.iter().map(|(l, v)| json!({"label": l, "sample": v.to_json()})).collect::<Vec<_>>(),
        });
        let _ = std::fs::write(&s.out, serde_json::to_string_pretty(&report).unwrap() + "\n");
    }
}

fn capture_window(hwnd: HWND) -> RgbaImage {
    unsafe {
        let mut rc = RECT::default();
        let _ = GetClientRect(hwnd, &mut rc);
        let (w, h) = (rc.right - rc.left, rc.bottom - rc.top);
        let screen = GetDC(None);
        let mem = CreateCompatibleDC(Some(screen));
        let bmp = CreateCompatibleBitmap(screen, w, h);
        let old = SelectObject(mem, bmp.into());
        // PW_CLIENTONLY | PW_RENDERFULLCONTENT (captures flip-model swap chains)
        let _ = windows::Win32::Storage::Xps::PrintWindow(hwnd, mem, windows::Win32::Storage::Xps::PRINT_WINDOW_FLAGS(3));
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut buf = vec![0u8; (w * h * 4) as usize];
        GetDIBits(mem, bmp, 0, h as u32, Some(buf.as_mut_ptr() as *mut _), &mut info, DIB_RGB_COLORS);
        SelectObject(mem, old);
        let _ = DeleteObject(bmp.into());
        let _ = DeleteDC(mem);
        ReleaseDC(None, screen);
        for p in buf.chunks_exact_mut(4) {
            p.swap(0, 2);
            p[3] = 255;
        }
        RgbaImage { width: w as u32, height: h as u32, data: buf }
    }
}

extern "system" fn main_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_COMMAND => {
                let id = loword(wp.0);
                let code = hiword(wp.0) as u32;
                with_app(|a| {
                    if (ID_CLIP..ID_CLIP + 50).contains(&id) {
                        a.play((id - ID_CLIP) as usize);
                    } else if id == ID_RESET {
                        a.reset();
                    } else if id == ID_SHOT {
                        let dir = PathBuf::from("lab-screenshots");
                        let _ = std::fs::create_dir_all(&dir);
                        let name = format!("lab_{}.png", a.started.elapsed().as_millis());
                        a.screenshot(&dir.join(name));
                    } else if (ID_CHECK..ID_CHECK + CHECKS.len() as i32).contains(&id) {
                        a.apply_checks();
                    } else if id == ID_BONES && code == LBN_SELCHANGE {
                        let idx = send(a.list, LB_GETCURSEL, 0, 0).0;
                        if idx >= 0 {
                            let b = a.bone_by_name(MAJOR_BONES[idx as usize]);
                            a.select_bone(b);
                        }
                    }
                });
                LRESULT(0)
            }
            WM_HSCROLL => {
                with_app(|a| {
                    if HWND(lp.0 as _) == a.slider {
                        let pos = send(a.slider, TBM_GETPOS, 0, 0).0 as f32 / 10.0;
                        a.set_manual(pos);
                    }
                });
                LRESULT(0)
            }
            WM_TIMER => {
                let id = wp.0;
                let _ = KillTimer(Some(hwnd), id);
                with_app(|a| {
                    if id == TIMER_IDLE && a.idle_enabled {
                        if a.player.is_static() {
                            if let Some(c) = a.idle.choose(a.started.elapsed().as_secs_f32()) {
                                a.play(c);
                            } else {
                                a.schedule_idle();
                            }
                        } else {
                            a.schedule_idle();
                        }
                    } else if id == TIMER_SCRIPT {
                        a.script_step();
                    }
                });
                LRESULT(0)
            }
            WM_SIZE => {
                let (w, h) = (loword(lp.0 as usize), hiword(lp.0 as usize));
                with_app(|a| {
                    let _ = MoveWindow(a.canvas, PANEL_W, 0, (w - PANEL_W).max(1), h.max(1), true);
                });
                LRESULT(0)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wp, lp),
        }
    }
}

extern "system" fn canvas_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                BeginPaint(hwnd, &mut ps);
                let _ = EndPaint(hwnd, &ps);
                with_app(|a| a.paint());
                LRESULT(0)
            }
            WM_ERASEBKGND => LRESULT(1),
            WM_SIZE => {
                let (w, h) = (loword(lp.0 as usize) as u32, hiword(lp.0 as usize) as u32);
                with_app(|a| {
                    if let Some(t) = &mut a.target {
                        let _ = t.resize(&a.renderer, [w, h]);
                    }
                    a.invalidate();
                });
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                let (x, y) = (loword(lp.0 as usize) as i16 as i32, hiword(lp.0 as usize) as i16 as i32);
                with_app(|a| a.pick_bone(x, y));
                LRESULT(0)
            }
            WM_MOUSEWHEEL => {
                let delta = (hiword(wp.0) as i16) as f32 / 120.0;
                with_app(|a| {
                    let v = a.manual[a.selected] + delta;
                    a.set_manual(v);
                });
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wp, lp),
        }
    }
}

fn find_rig_dir() -> PathBuf {
    let rel = PathBuf::from("assets/mascot/rig-v0.2");
    let mut roots = vec![std::env::current_dir().unwrap_or_default()];
    if let Ok(exe) = std::env::current_exe() {
        roots.extend(exe.ancestors().map(|p| p.to_path_buf()));
    }
    roots.into_iter().map(|r| r.join(&rel)).find(|p| p.join("rig.json").exists()).unwrap_or(rel)
}

fn fatal(msg: &str) -> ! {
    unsafe {
        MessageBoxW(None, &HSTRING::from(msg), w!("mascot-animation-lab"), MB_ICONERROR);
    }
    std::process::exit(1)
}

fn main() {
    let mut rig_dir = find_rig_dir();
    let mut kind = DeviceKind::Hardware;
    let mut script: Option<(ScriptKind, PathBuf)> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--rig" => rig_dir = args.next().expect("--rig DIR").into(),
            "--warp" => kind = DeviceKind::Warp,
            "--perf" => script = Some((ScriptKind::Perf, args.next().expect("--perf OUT.json").into())),
            "--capture" => script = Some((ScriptKind::Capture, args.next().expect("--capture DIR").into())),
            other => fatal(&format!("unknown argument {other}")),
        }
    }
    if let Some((ScriptKind::Capture, d)) = &script {
        let _ = std::fs::create_dir_all(d);
    }
    let rig = Rig::load(&rig_dir).unwrap_or_else(|e| fatal(&format!("load rig {}: {e}", rig_dir.display())));
    let clips = rig.load_clips().unwrap_or_else(|e| fatal(&format!("load clips: {e}")));
    let mut renderer = Renderer::new(kind).unwrap_or_else(|e| fatal(&format!("Direct2D init: {e}")));
    renderer.load_rig(&rig).unwrap_or_else(|e| fatal(&e));

    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let _ = InitCommonControlsEx(&INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_BAR_CLASSES | ICC_STANDARD_CLASSES,
        });
        let hinst: HINSTANCE = GetModuleHandleW(None).unwrap().into();
        let cursor = LoadCursorW(None, IDC_ARROW).unwrap();
        RegisterClassExW(&WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(main_proc),
            hInstance: hinst,
            hCursor: cursor,
            hbrBackground: GetSysColorBrush(COLOR_BTNFACE),
            lpszClassName: w!("MascotLabMain"),
            ..Default::default()
        });
        RegisterClassExW(&WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(canvas_proc),
            hInstance: hinst,
            hCursor: cursor,
            lpszClassName: w!("MascotLabCanvas"),
            ..Default::default()
        });
        let main = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("MascotLabMain"),
            w!("Mascot rig v0.2 - animation lab"),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            80,
            60,
            1280,
            900,
            None,
            None,
            Some(hinst),
            None,
        )
        .unwrap_or_else(|e| fatal(&format!("CreateWindow: {e}")));
        let font = CreateFontW(-15, 0, 0, 0, 400, 0, 0, 0, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, CLEARTYPE_QUALITY, 0, w!("Segoe UI"));
        let mk = |class: PCWSTR, text: &str, style: WINDOW_STYLE, x: i32, y: i32, cw: i32, ch: i32, id: i32| -> HWND {
            let h = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                class,
                &HSTRING::from(text),
                WS_CHILD | WS_VISIBLE | style,
                x,
                y,
                cw,
                ch,
                Some(main),
                Some(HMENU(id as isize as _)),
                Some(hinst),
                None,
            )
            .unwrap();
            send(h, WM_SETFONT, font.0 as usize, 1);
            h
        };
        let mut y = 10;
        let pad = 10;
        let bw = (PANEL_W - 3 * pad) / 2;
        mk(w!("STATIC"), "Clips", WINDOW_STYLE(0), pad, y, PANEL_W - 2 * pad, 20, -1);
        y += 22;
        for (i, c) in clips.iter().enumerate() {
            let (col, row) = ((i % 2) as i32, (i / 2) as i32);
            mk(w!("BUTTON"), &c.clip.name, WINDOW_STYLE(BS_PUSHBUTTON as u32), pad + col * (bw + pad), y + row * 30, bw, 26, ID_CLIP + i as i32);
        }
        y += clips.len().div_ceil(2) as i32 * 30 + 4;
        mk(w!("BUTTON"), "Reset to rest", WINDOW_STYLE(BS_PUSHBUTTON as u32), pad, y, bw, 26, ID_RESET);
        mk(w!("BUTTON"), "Screenshot", WINDOW_STYLE(BS_PUSHBUTTON as u32), 2 * pad + bw, y, bw, 26, ID_SHOT);
        y += 36;
        let mut checks = Vec::new();
        for (i, c) in CHECKS.iter().enumerate() {
            let (col, row) = ((i % 2) as i32, (i / 2) as i32);
            checks.push(mk(w!("BUTTON"), c, WINDOW_STYLE(BS_AUTOCHECKBOX as u32), pad + col * (bw + pad), y + row * 24, bw, 22, ID_CHECK + i as i32));
        }
        y += CHECKS.len().div_ceil(2) as i32 * 24 + 8;
        mk(w!("STATIC"), "Bone (click a pivot in the view, wheel rotates)", WINDOW_STYLE(0), pad, y, PANEL_W - 2 * pad, 20, -1);
        y += 22;
        let list = mk(w!("LISTBOX"), "", WINDOW_STYLE((LBS_NOTIFY as u32) | WS_BORDER.0 | WS_VSCROLL.0), pad, y, PANEL_W - 2 * pad, 190, ID_BONES);
        for b in MAJOR_BONES {
            send(list, LB_ADDSTRING, 0, HSTRING::from(*b).as_ptr() as isize);
        }
        y += 196;
        let slider = mk(TRACKBAR_CLASSW, "", WINDOW_STYLE(TBS_HORZ as u32 | TBS_AUTOTICKS as u32), pad, y, PANEL_W - 2 * pad, 30, ID_SLIDER);
        send(slider, TBM_SETTICFREQ, 50, 0);
        y += 34;
        let value = mk(w!("STATIC"), "", WINDOW_STYLE(0), pad, y, PANEL_W - 2 * pad, 20, -1);
        y += 30;
        let status = mk(w!("STATIC"), "", WINDOW_STYLE(0), pad, y, PANEL_W - 2 * pad, 80, -1);
        let canvas = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("MascotLabCanvas"),
            w!(""),
            WS_CHILD | WS_VISIBLE,
            PANEL_W,
            0,
            1000,
            860,
            Some(main),
            None,
            Some(hinst),
            None,
        )
        .unwrap();
        let mut rc = RECT::default();
        let _ = GetClientRect(canvas, &mut rc);
        let target = WindowTarget::new(&renderer, canvas, [rc.right as u32, rc.bottom as u32])
            .unwrap_or_else(|e| fatal(&format!("swap chain: {e}")));
        let n = rig.skeleton.bones.len();
        let pose = rig.skeleton.rest_pose();
        let names = ["blink", "double_blink", "look_left", "look_right", "small_head_tilt", "ear_twitch", "tail_flick", "posture_adjust", "stretch"];
        let weights = [5.0, 2.0, 1.5, 1.5, 1.2, 2.0, 2.0, 1.0, 0.6];
        let cooldowns = [0.0, 8.0, 10.0, 10.0, 12.0, 6.0, 6.0, 20.0, 40.0];
        let actions = names
            .iter()
            .zip(weights)
            .zip(cooldowns)
            .filter_map(|((n, w), c)| clips.iter().position(|x| x.clip.name == *n).map(|clip| IdleAction { clip, weight: w, cooldown: c }))
            .collect();
        let app = App {
            main,
            canvas,
            list,
            slider,
            value,
            status,
            checks,
            rig,
            clips,
            renderer,
            target: Some(target),
            player: Player::new(),
            manual: vec![0.0; n],
            pose,
            opts: RenderOptions { background: Some([0.93, 0.95, 0.98, 1.0]), ..Default::default() },
            mirror: false,
            selected: 0,
            last_frame: None,
            frames: 0,
            last_render_ms: 0.0,
            started: Instant::now(),
            idle: IdleDirector::new(actions, 0x5eed),
            idle_enabled: false,
            script: script.map(|(kind, out)| Script {
                kind,
                step: 0,
                out,
                samples: Vec::new(),
                queue: Vec::new(),
                active_from: None,
                frame_times: Vec::new(),
                frames_at: Vec::new(),
            }),
            font,
        };
        APP.with(|a| *a.borrow_mut() = Some(app));
        with_app(|a| {
            a.set_check(1, true);
            let h = a.bone_by_name("head");
            a.select_bone(h);
            a.update_status();
            if a.script.is_some() {
                SetTimer(Some(a.main), TIMER_SCRIPT, 1500, None);
            }
        });
        let _ = ShowWindow(main, SW_SHOW);
        let _ = UpdateWindow(main);
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        with_app(|a| {
            let _ = DeleteObject(a.font.into());
        });
    }
}
