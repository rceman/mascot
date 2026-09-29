//! The product shell: owns the renderer, painter, RichEdit composer, sprite
//! and [`UiState`]; drives the DComp window's wndproc and the message loop.
//!
//! Event-driven: a frame is presented only when a transition or host event
//! made `dirty` true. No timers run while the UI is idle and the caret has
//! timed out.

type HotkeyFn = Option<Box<dyn FnMut(&mut App, u32)>>;

use std::cell::RefCell;

use mascot_animation::Rig;
use mascot_render_win32::renderer::{DeviceKind, Renderer};
use mascot_ui::geom::Point;
use mascot_ui::layout::{Hit, Layout, MascotMetrics, Measured, hit_test, layout};
use mascot_ui::state::{Activity, ControlId, Surface, UiState};
use mascot_ui::theme::{Theme, tokens};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::HiDpi::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::*;

use crate::edit::{EDIT_TIMER_BASE, Editor, EditorConfig, HostEvent};
use crate::paint::Painter;
use crate::sprite::Sprite;
use crate::text::Fonts;
use crate::window::{CompSurface, register_class};

const TIMER_RESPONSE: usize = 0x5200;
const TIMER_CARET_BLINK: usize = 0x5201;
const TIMER_COPY_REVERT: usize = 0x5202;
const TIMER_TOOLTIP: usize = 0x5203;

/// The whole Mascot UI (mascot + bubble) in one transparent window.
pub struct App {
    pub renderer: Renderer,
    pub painter: Painter,
    pub editor: Editor,
    pub rig: Rig,
    pub metrics: MascotMetrics,
    pub state: UiState,
    pub scale: f32,
    pub layout: Layout,
    pub dirty: bool,
    hwnd: HWND,
    surf: Option<CompSurface>,
    /// Screen-anchored bottom edge for the composition.
    anchor: POINT,
    drag: Option<Point>,
    /// Last input time for the caret blink timeout.
    last_input: std::time::Instant,
    /// Cached `Measured` inputs — `relayout` must not re-query the editor /
    /// DWrite on every frame (EM_REQUESTRESIZE + DWrite measure are costly).
    /// Cached content measurements (editor natural height, response/tooltip
    /// text metrics) in DIP — scale-independent.
    pub measured: Measured,
    /// Set when editor content/scale/width may have changed `editor_content_h`.
    measure_editor: bool,
    /// Cached response fixture height (DIP, scale-independent), measured once.
    response_h: Option<f32>,
    /// Last tooltip width measurement key: (control, copied).
    tooltip_key: Option<(Option<ControlId>, bool)>,
    /// Layout must be recomputed before the next present.
    layout_stale: bool,
    /// Frames actually presented (perf instrumentation).
    pub present_count: u64,
    /// WM_PAINT messages seen (perf instrumentation).
    pub paint_count: u64,
    /// Lab hooks (hotkeys/state presets); `None` in production use.
    #[allow(clippy::type_complexity)]
    pub on_hotkey: HotkeyFn,
    /// Optional response timer: ms until the mock response arrives.
    pending_response: bool,
    /// Text captured at submit time (shown dimmed while submitting).
    submitted: String,
    /// One-shot "response" delay in ms (lab sets; 0 = manual).
    pub response_delay_ms: u32,
    /// Last submitted text (read by selftest).
    pub last_submitted: String,
    /// Diagnostics: every WM_CHAR code seen (selftest evidence).
    pub char_trace: Vec<u32>,
    /// Diagnostics: reason labels of frames that actually presented —
    /// labels accumulate as `Vec<String>` per present (perf instrumentation).
    pub present_reasons: Vec<String>,
    /// Dirty-reason buffer: filled by `mark_dirty`, consumed at present.
    pub dirty_reasons: Vec<&'static str>,
}

impl App {
    /// Marks the frame dirty with a reason (perf instrumentation).
    pub fn mark_dirty(&mut self, reason: &'static str) {
        self.dirty = true;
        if self.dirty_reasons.len() < 16 {
            self.dirty_reasons.push(reason);
        }
    }
}

impl App {
    /// Builds renderer + rig sprite + fonts + RichEdit composer.
    /// `rig` must already be loaded (`Rig::load`).
    pub fn new(device: DeviceKind, rig: Rig, hwnd: HWND, scale: f32) -> Result<App> {
        Self::new_instrumented(device, rig, hwnd, scale, |_| {})
    }

    /// Same as [`App::new`], but `stage` is invoked after each startup stage
    /// (`"renderer"`, `"rig"`, `"fonts+painter"`, `"richedit"`, `"sprite"`)
    /// — used by the perf harness for the memory breakdown.
    pub fn new_instrumented(
        device: DeviceKind,
        rig: Rig,
        hwnd: HWND,
        scale: f32,
        mut stage: impl FnMut(&'static str),
    ) -> Result<App> {
        let mut renderer = Renderer::new(device)?;
        stage("renderer");
        renderer
            .load_rig(&rig)
            .map_err(|e| Error::new(HRESULT(0x8000_4005u32 as i32), e))?;
        stage("rig");
        let fonts = Fonts::new(renderer.dwrite())?;
        let painter = Painter::new(&renderer.ctx, fonts)?;
        stage("fonts+painter");
        let cfg = EditorConfig {
            face: painter.fonts.family.clone(),
            size_twips: (tokens::BODY_SIZE * 15.0) as i32, // 14 DIP -> 210 twips
            fg: Theme::Light.palette().foreground,
            sel_bg: Theme::Light.palette().selection_bg,
            sel_fg: Theme::Light.palette().selection_fg,
            read_only: false,
        };
        let editor = Editor::new(hwnd, RECT::default(), scale, &cfg)?;
        stage("richedit");
        let mut app = App {
            renderer,
            painter,
            editor,
            rig,
            metrics: MascotMetrics {
                content_w: tokens::MASCOT_H,
                content_h: tokens::MASCOT_H,
            },
            state: UiState::default(),
            scale,
            layout: Layout::default(),
            dirty: true,
            hwnd,
            surf: None,
            anchor: POINT { x: 0, y: 0 },
            drag: None,
            last_input: std::time::Instant::now(),
            measured: Measured::default(),
            measure_editor: true,
            response_h: None,
            tooltip_key: None,
            layout_stale: true,
            present_count: 0,
            paint_count: 0,
            on_hotkey: None,
            pending_response: false,
            submitted: String::new(),
            response_delay_ms: 1200,
            last_submitted: String::new(),
            char_trace: Vec::new(),
            present_reasons: Vec::new(),
            dirty_reasons: Vec::new(),
        };
        app.rebuild_sprite()?;
        stage("sprite");
        Ok(app)
    }

    /// Re-renders the mascot sprite for the current placement/scale.
    pub fn rebuild_sprite(&mut self) -> Result<()> {
        let mirror = self.state.placement == mascot_ui::state::Placement::Right;
        let dip = [tokens::MASCOT_H, tokens::MASCOT_H];
        self.painter.sprite = Some(Sprite::render(
            &mut self.renderer,
            &self.rig,
            mirror,
            dip,
            self.scale,
        )?);
        self.metrics = MascotMetrics {
            content_w: dip[0],
            content_h: dip[1],
        };
        Ok(())
    }

    /// Marks layout + frame stale (state affecting geometry changed).
    fn mark_layout(&mut self) {
        self.layout_stale = true;
        self.mark_dirty("layout");
    }

    /// Recomputes [`Layout`] from the current state. Expensive measurements
    /// (editor natural size via EM_REQUESTRESIZE, DWrite text metrics) are
    /// cached and only re-run when the input that produced them changed:
    /// editor content/scale, Response surface (once), tooltip identity.
    pub fn relayout(&mut self) -> Result<()> {
        if self.measure_editor {
            let editor_w = tokens::BUBBLE_W
                - tokens::BUBBLE_PAD_X
                - tokens::COMPOSER_EDGE
                - tokens::PRIMARY_BUTTON
                - tokens::COMPOSER_GAP;
            self.measured.editor_content_h = self
                .editor
                .natural_size(editor_w)
                .map(|(_, h)| h)
                .unwrap_or(tokens::BODY_LINE);
            self.measure_editor = false;
        }
        let rh = if self.state.surface == Surface::Response {
            *self.response_h.get_or_insert_with(|| {
                self.painter
                    .fonts
                    .measure(
                        mascot_ui::RESPONSE_FIXTURE,
                        tokens::RESPONSE_TEXT_W,
                        &self.painter.fonts.body,
                    )
                    .map(|(_, h)| h)
                    .unwrap_or(0.0)
            })
        } else {
            0.0
        };
        self.measured.response_text_h = rh;
        let tw_key = Some((self.state.tooltip, self.state.copied));
        if self.tooltip_key != tw_key {
            self.measured.tooltip_text_w = match self.state.tooltip {
                Some(id) => {
                    let label = match id {
                        ControlId::Copy if self.state.copied => "Copied",
                        c => c.icon().label(),
                    };
                    self.painter
                        .fonts
                        .measure(label, 400.0, &self.painter.fonts.small)
                        .map(|(w, _)| w)
                        .unwrap_or(0.0)
                }
                None => 0.0,
            };
            self.tooltip_key = tw_key;
        }
        self.layout = layout(&self.state, &self.metrics, &self.measured, self.scale);
        // keep the editor's client rect in sync — the editor's coordinate
        // space is DIP (scale-independent)
        if let Some(e) = self.layout.editor {
            let before = self.editor.client_rect();
            let want = RECT {
                left: 0,
                top: 0,
                right: e.w.round() as i32,
                bottom: e.h.round() as i32,
            };
            if before != want {
                self.editor.set_client_rect(want);
                if before.right - before.left != want.right - want.left {
                    // wrap width changed: next relayout must re-measure
                    self.measure_editor = true;
                }
            }
            self.editor.set_scale(self.scale)?;
        }
        self.layout_stale = false;
        self.mark_dirty("relayout");
        Ok(())
    }

    /// Paints + presents when dirty. No-op when idle-clean. Only relayouts
    /// when geometry-affecting state changed (`layout_stale`).
    pub fn present_if_dirty(&mut self) -> Result<()> {
        if self.layout_stale {
            self.relayout()?;
        }
        if !self.dirty {
            return Ok(());
        }
        self.dirty = false;
        let pal = self.state.theme.palette();
        let px = self.layout.window_px();
        if self.surf.is_none() {
            return Ok(());
        }
        let needs_resize = self.surf.as_ref().unwrap().size != px;
        if needs_resize {
            self.surf
                .as_mut()
                .unwrap()
                .resize(&self.renderer, px, self.scale)?;
            self.move_window();
        }
        self.painter
            .prepare(&self.renderer.ctx, &pal, &self.layout, px)?;
        let lay = self.layout;
        // split borrows: take the fields we need
        let (state, ed) = (&self.state, Some(&self.editor));
        let painter = &self.painter;
        self.surf.as_mut().unwrap().present(&self.renderer, |ctx| {
            painter.paint_frame(ctx, state, &lay, ed)
        })?;
        self.present_count += 1;
        // perf diagnostics: which dirty source produced this frame
        if self.dirty_reasons.is_empty() {
            self.present_reasons.push("unknown".to_string());
        } else {
            self.present_reasons.push(self.dirty_reasons.join("+"));
        }
        self.dirty_reasons.clear();
        Ok(())
    }

    /// Renders the current state to an offscreen bitmap (same painter path).
    pub fn render_offscreen(&mut self) -> Result<mascot_render_win32::image::RgbaImage> {
        if self.layout_stale {
            self.relayout()?;
        }
        let pal = self.state.theme.palette();
        let px = self.layout.window_px();
        let ctx = &self.renderer.ctx;
        self.painter.prepare(ctx, &pal, &self.layout, px)?;
        let bmp = self.renderer.create_target_bitmap(px)?;
        let scale = self.scale;
        unsafe {
            let mut dx = 0.0f32;
            let mut dy = 0.0f32;
            ctx.GetDpi(&mut dx, &mut dy);
            ctx.SetTarget(&bmp.cast::<windows::Win32::Graphics::Direct2D::ID2D1Image>()?);
            ctx.SetDpi(96.0 * scale, 96.0 * scale);
            ctx.BeginDraw();
            let r = self
                .painter
                .paint_frame(ctx, &self.state, &self.layout, Some(&self.editor));
            let e = ctx.EndDraw(None, None);
            ctx.SetTarget(None);
            ctx.SetDpi(dx, dy); // renderer APIs assume a 96-DPI ctx
            e?;
            r?;
        }
        self.renderer.read_back(&bmp, px)
    }

    // -- input ------------------------------------------------------------

    fn dip(&self, x: i32, y: i32) -> Point {
        Point::new(x as f32 / self.scale, y as f32 / self.scale)
    }

    fn mark_input(&mut self) {
        self.last_input = std::time::Instant::now();
        // fresh input restarts the blink window: richedit timers may have been
        // killed by the timeout; WM_SETFOCUS re-arms them on next focus.
    }

    /// Sync editor colors/face with the theme. Existing text recolors live:
    /// normal runs carry `CFE_AUTOCOLOR` (resolved via `TxGetSysColor`) and
    /// the submitting-dim run is re-applied with the new muted tone.
    pub fn apply_theme(&mut self) {
        let p = self.state.theme.palette();
        self.editor
            .set_colors(p.foreground, p.selection_bg, p.selection_fg);
        if self.state.activity == Activity::Submitting {
            self.editor.dim_text(Some(p.muted_fg));
        }
        self.mark_dirty("theme");
    }

    /// Opens the composer (mascot click while hidden).
    pub fn open_composer(&mut self) {
        self.state.open();
        self.focus_editor();
        self.relayout().ok();
        self.move_window();
        self.present_if_dirty().ok();
    }

    /// Gives the editor keyboard focus (used by perf/selftest harnesses).
    pub fn focus_editor(&mut self) {
        self.state.set_focus(Some(ControlId::Editor), false);
        // focus gain is activity: the caret blink countdown restarts
        self.last_input = std::time::Instant::now();
        self.editor.send(WM_SETFOCUS, 0, 0);
        self.arm_caret_timer();
    }

    /// Drops keyboard focus without hiding the surface (perf harness uses it
    /// for the unfocused-idle measurement).
    pub fn unfocus_editor(&mut self) {
        self.state.set_focus(None, false);
        self.blur_editor();
        self.mark_dirty("unfocus");
    }

    fn blur_editor(&mut self) {
        self.editor.send(WM_KILLFOCUS, 0, 0);
        self.editor.kill_timers();
        unsafe {
            let _ = KillTimer(Some(self.hwnd), TIMER_CARET_BLINK);
        }
    }

    fn arm_caret_timer(&mut self) {
        unsafe {
            let blink = GetCaretBlinkTime();
            if blink != u32::MAX && blink > 0 {
                let _ = SetTimer(Some(self.hwnd), TIMER_CARET_BLINK, blink, None);
            }
        }
    }

    /// Moves/resizes the window to the current layout keeping the bottom
    /// corner anchored (`self.anchor`): `anchor.x` is the window's LEFT edge
    /// for `Placement::Left` and its RIGHT edge for `Placement::Right` (the
    /// near-screen-edge anchor mirrors with placement).
    pub fn move_window(&mut self) {
        if self.hwnd.is_invalid() {
            return;
        }
        let px = self.layout.window_px();
        let (w, h) = (px[0] as i32, px[1] as i32);
        let left = match self.state.placement {
            mascot_ui::state::Placement::Right => self.anchor.x - w,
            mascot_ui::state::Placement::Left => self.anchor.x,
        };
        unsafe {
            let _ = SetWindowPos(
                self.hwnd,
                Some(HWND_TOPMOST),
                left,
                self.anchor.y - h,
                w,
                h,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
    }

    /// Sets the bottom anchor in screen px (left edge for Left placement,
    /// right edge for Right).
    pub fn set_anchor(&mut self, right: i32, bottom: i32) {
        self.anchor = POINT {
            x: right,
            y: bottom,
        };
        self.move_window();
    }

    pub fn attach_window(&mut self, surf: CompSurface) {
        self.hwnd = surf.hwnd;
        self.editor.set_hwnd(surf.hwnd);
        self.surf = Some(surf);
    }

    /// Esc: hidden surface. Returns true if it closed something.
    pub fn escape(&mut self) -> bool {
        if self.state.surface != Surface::Hidden {
            self.blur_editor();
            self.state.escape();
            self.relayout().ok();
            self.move_window();
            self.present_if_dirty().ok();
            // the hidden window holds no content — release driver-side heaps
            self.trim_gpu();
            return true;
        }
        false
    }

    /// Releases idle GPU-side allocations: `IDXGIDevice3::Trim` (D3D heap
    /// pages no longer referenced) + `ID2D1Device::ClearResources` (cached
    /// bitmaps/effects). Measured to cut ~40-60 MB of private bytes after
    /// startup without costing frame time (the swapchain buffers survive).
    pub fn trim_gpu(&self) {
        use windows::Win32::Graphics::Dxgi::IDXGIDevice3;
        unsafe {
            if let Ok(dxgi) = self.renderer.d3d.cast::<IDXGIDevice3>() {
                dxgi.Trim();
            }
            if let Ok(dev) = self.renderer.ctx.GetDevice() {
                dev.ClearResources(0);
            }
        }
    }

    /// Submit (Enter). Starts the mock response timer. The submitted text
    /// stays visible read-only and dimmed to muted-fg.
    pub fn submit(&mut self) -> bool {
        if !self.state.submit() {
            return false;
        }
        self.submitted = self.editor.text();
        self.last_submitted = self.submitted.clone();
        self.editor.set_read_only(true);
        let pal = self.state.theme.palette();
        self.editor.dim_text(Some(pal.muted_fg));
        // read-only text: hide the caret, move keyboard focus to Stop
        self.editor.send(WM_KILLFOCUS, 0, 0);
        self.editor.kill_timers();
        self.state.set_focus(Some(ControlId::Stop), false);
        if self.response_delay_ms > 0 {
            self.pending_response = true;
            unsafe {
                let _ = SetTimer(
                    Some(self.hwnd),
                    TIMER_RESPONSE,
                    self.response_delay_ms,
                    None,
                );
            }
        }
        self.relayout().ok();
        self.present_if_dirty().ok();
        true
    }

    /// Mock response arrival (the response timer drives it in interactive
    /// mode; harnesses call it directly).
    pub fn response_arrived(&mut self) {
        if self.state.activity != Activity::Submitting {
            return;
        }
        self.pending_response = false;
        self.editor.set_read_only(false);
        self.editor.dim_text(None);
        let _ = self.editor.set_text("");
        self.state.response_arrived();
        self.relayout().ok();
        self.move_window();
        self.present_if_dirty().ok();
    }

    fn copy_clicked(&mut self) {
        if self.state.surface != Surface::Response {
            return;
        }
        let _ = clipboard_write(mascot_ui::RESPONSE_FIXTURE);
        self.state.copy();
        unsafe {
            let _ = SetTimer(Some(self.hwnd), TIMER_COPY_REVERT, 1500, None);
        }
        self.mark_layout(); // copied tooltip appeared
    }

    /// Drains editor host events → repaint/relayout as needed.
    pub fn process_editor_events(&mut self) {
        let mut changed = false;
        for e in self.editor.drain_events() {
            match e {
                HostEvent::Invalidate | HostEvent::Caret => self.mark_dirty("editor"),
                HostEvent::Change => {
                    self.state.editor_empty = self.editor.is_empty();
                    self.measure_editor = true;
                    changed = true;
                }
                HostEvent::SetTimer(..) | HostEvent::KillTimer(_) | HostEvent::Capture(_) => {}
            }
        }
        if changed {
            self.relayout().ok();
            self.move_window();
        }
    }

    // -- message handlers ---------------------------------------------------

    /// Shift state at the instant this message is being processed.
    /// `GetKeyState` is synchronized with the message stream — safe to read
    /// here (the harness drains injected input before asserting).
    fn shift_down(&self) -> bool {
        unsafe { GetKeyState(VK_SHIFT.0 as i32) < 0 }
    }

    fn wm_char(&mut self, w: usize, l: isize) -> isize {
        if self.char_trace.len() < 512 {
            self.char_trace.push(w as u32);
        }
        if self.state.activity == Activity::Submitting {
            return 0; // submitted text is read-only
        }
        let ch = w as u32;
        // Enter (CR) submits; Ctrl+Enter (LF) behaves like Enter. Shift+Enter
        // and composition chars are forwarded to the editor natively.
        if ch == 0x0D || ch == 0x0A {
            if self.state.composing || self.shift_down() {
                let r = self.editor.send(WM_CHAR, w, l);
                self.mark_input();
                self.process_editor_events();
                self.present_if_dirty().ok();
                return r;
            }
            self.submit();
            return 0;
        }
        if ch == 27 {
            return 0; // Esc char — handled at keydown
        }
        // chars only reach the editor while it owns keyboard input (IME
        // composition chars are already handled above)
        if self.state.interaction.focus != Some(ControlId::Editor) {
            return 0;
        }
        let r = self.editor.send(WM_CHAR, w, l);
        self.mark_input();
        self.process_editor_events();
        self.present_if_dirty().ok();
        r
    }

    fn wm_keydown(&mut self, w: usize, l: isize) -> isize {
        let vk = w as u32;
        self.mark_input();
        // lab hotkeys first (function keys)
        if (VK_F1.0 as u32..=VK_F5.0 as u32).contains(&vk) {
            if let Some(h) = &mut self.on_hotkey {
                // borrow dance: take, call, put back
                let mut h = std::mem::replace(h, Box::new(|_, _| {}));
                h(self, vk);
                self.on_hotkey = Some(h);
            }
            self.present_if_dirty().ok();
            return 0;
        }
        match vk {
            v if v == VK_ESCAPE.0 as u32 => {
                if !self.escape() {
                    // nothing open: also pass to editor (IME cancel etc.)
                    let _ = self.editor.send(WM_KEYDOWN, w, l);
                }
                0
            }
            v if v == VK_TAB.0 as u32 => {
                let shift = self.shift_down();
                self.blur_editor();
                self.state.cycle_focus(shift);
                if self.state.interaction.focus == Some(ControlId::Editor) {
                    self.editor.send(WM_SETFOCUS, 0, 0);
                }
                self.present_if_dirty().ok();
                0
            }
            v if v == VK_RETURN.0 as u32 => {
                if self.state.composing {
                    // Enter commits the composition inside the editor.
                    return self.editor.send(WM_KEYDOWN, w, l);
                }
                if self.state.interaction.focus != Some(ControlId::Editor) {
                    // Enter on a focused button activates it; the translated
                    // CR is dropped by wm_char (focus isn't the editor).
                    self.activate_focus();
                    return 0;
                }
                if self.shift_down() {
                    // Shift+Enter: native newline — forward the keydown so the
                    // editor sees the full VK+char pair.
                    let r = self.editor.send(WM_KEYDOWN, w, l);
                    self.process_editor_events();
                    self.present_if_dirty().ok();
                    return r;
                }
                // plain Enter: submission happens when the translated WM_CHAR
                // arrives (richtext ordering); nothing to forward.
                0
            }
            v if v == VK_SPACE.0 as u32 => {
                if self.state.interaction.focus != Some(ControlId::Editor) {
                    self.activate_focus();
                    return 0;
                }
                let r = self.editor.send(WM_KEYDOWN, w, l);
                self.process_editor_events();
                self.present_if_dirty().ok();
                r
            }
            _ => {
                // key events only reach the editor while it owns input —
                // Ctrl accelerators (A/C/X/V/Z, word nav) are handled
                // natively by msftedit.
                if self.state.interaction.focus != Some(ControlId::Editor) && !self.state.composing
                {
                    return 0;
                }
                let r = self.editor.send(WM_KEYDOWN, w, l);
                self.process_editor_events();
                self.present_if_dirty().ok();
                r
            }
        }
    }

    fn activate_focus(&mut self) {
        match self.state.interaction.focus {
            Some(ControlId::Send) => {
                self.submit();
            }
            Some(ControlId::Stop) => {
                self.state.stop();
                self.editor.set_read_only(false);
                self.editor.dim_text(None);
                self.focus_editor();
                self.pending_response = false;
                unsafe {
                    let _ = KillTimer(Some(self.hwnd), TIMER_RESPONSE);
                }
                self.present_if_dirty().ok();
            }
            Some(ControlId::Copy) => {
                self.copy_clicked();
                self.present_if_dirty().ok();
            }
            _ => {}
        }
    }

    fn wm_mousemove(&mut self, x: i32, y: i32) {
        let p = self.dip(x, y);
        if let Some(origin_px) = self.drag {
            // mascot drag: move the whole window; origin is the client-px
            // offset where the press landed
            unsafe {
                let mut pt = POINT { x, y };
                let _ = ClientToScreen(self.hwnd, &mut pt);
                let w = self.layout.window_px();
                let left = pt.x - origin_px.x as i32;
                let top = pt.y - origin_px.y as i32;
                let _ = SetWindowPos(
                    self.hwnd,
                    Some(HWND_TOPMOST),
                    left,
                    top,
                    w[0] as i32,
                    w[1] as i32,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
                self.anchor = POINT {
                    x: left + w[0] as i32,
                    y: top + w[1] as i32,
                };
            }
            return;
        }
        let h = hit_test(&self.state, &self.layout, p);
        let c = match h {
            Hit::Control(id) => Some(id),
            Hit::Editor => Some(ControlId::Editor),
            _ => None,
        };
        let had_tooltip = self.state.tooltip.is_some();
        let before = self.state.interaction;
        self.state.set_hover(c);
        if self.state.interaction != before {
            self.mark_dirty("hover");
        }
        // a cleared tooltip changes geometry
        if had_tooltip && self.state.tooltip.is_none() {
            self.mark_layout();
        }
        // forward to the editor when hovering its rect (caret cursor);
        // richedit coords are editor-local device px
        if h == Hit::Editor
            && let Some(e) = self.layout.editor
        {
            self.editor.send(
                WM_MOUSEMOVE,
                0,
                make_lparam((p.x - e.x).round() as i32, (p.y - e.y).round() as i32),
            );
            self.process_editor_events();
        }
        self.present_if_dirty().ok();
    }

    fn wm_lbuttondown(&mut self, x: i32, y: i32) {
        let p = self.dip(x, y);
        let h = hit_test(&self.state, &self.layout, p);
        match h {
            Hit::Mascot => {
                unsafe {
                    SetCapture(self.hwnd);
                }
                self.drag = Some(Point::new(x as f32, y as f32)); // client px
            }
            Hit::Control(id) => {
                self.state.interaction.pressed = Some(id);
                self.state.set_focus(Some(id), false);
                self.mark_dirty("press");
            }
            Hit::Editor => {
                self.state.set_focus(Some(ControlId::Editor), false);
                self.editor.send(WM_SETFOCUS, 0, 0);
                if let Some(e) = self.layout.editor {
                    self.editor.send(
                        WM_LBUTTONDOWN,
                        windows::Win32::System::SystemServices::MK_LBUTTON.0 as usize,
                        make_lparam((p.x - e.x).round() as i32, (p.y - e.y).round() as i32),
                    );
                    self.process_editor_events();
                }
                unsafe {
                    SetCapture(self.hwnd);
                }
            }
            _ => {}
        }
        self.present_if_dirty().ok();
    }

    fn wm_lbuttonup(&mut self, x: i32, y: i32) {
        let p = self.dip(x, y);
        if let Some(origin) = self.drag.take() {
            unsafe {
                let _ = ReleaseCapture();
            }
            // click without drag (< SM_CXDRAG) toggles the bubble
            let dx = (x as f32 - origin.x).abs();
            let dy = (y as f32 - origin.y).abs();
            let thresh = unsafe { GetSystemMetrics(SM_CXDRAG) } as f32;
            if dx <= thresh && dy <= thresh {
                if self.state.surface == Surface::Hidden {
                    self.open_composer();
                } else {
                    self.escape();
                }
            }
            return;
        }
        let h = hit_test(&self.state, &self.layout, p);
        // the editor needs the button-up to finish drag-select (it may hold
        // richedit-requested mouse capture)
        if let Some(e) = self.layout.editor {
            self.editor.send(
                WM_LBUTTONUP,
                0,
                make_lparam((p.x - e.x).round() as i32, (p.y - e.y).round() as i32),
            );
            self.process_editor_events();
        }
        let pressed = self.state.interaction.pressed.take();
        if let (Some(id), Hit::Control(hit_id)) = (pressed, h)
            && id == hit_id
        {
            match id {
                ControlId::Send => {
                    self.submit();
                }
                ControlId::Stop => {
                    self.state.stop();
                    self.editor.set_read_only(false);
                    self.editor.dim_text(None);
                    self.focus_editor();
                    self.pending_response = false;
                    unsafe {
                        let _ = KillTimer(Some(self.hwnd), TIMER_RESPONSE);
                    }
                }
                ControlId::Copy => {
                    self.copy_clicked();
                }
                _ => {}
            }
        }
        self.mark_dirty("lbuttonup");
        self.present_if_dirty().ok();
    }

    fn wm_timer(&mut self, id: usize) {
        match id {
            TIMER_RESPONSE => {
                unsafe {
                    let _ = KillTimer(Some(self.hwnd), TIMER_RESPONSE);
                }
                self.response_arrived();
            }
            TIMER_COPY_REVERT => {
                unsafe {
                    let _ = KillTimer(Some(self.hwnd), TIMER_COPY_REVERT);
                }
                self.state.copy_revert();
                self.mark_layout(); // "Copied" tooltip label shrinks
                self.present_if_dirty().ok();
            }
            TIMER_CARET_BLINK => {
                // windowless richedit does not blink the caret itself — the
                // host toggles caret_shown at GetCaretBlinkTime, and stops
                // once SPI_GETCARETTIMEOUT has elapsed since the last input
                // so the caret freezes solid and the process emits zero
                // frames.
                let mut timeout_ms = 5000u32;
                unsafe {
                    let _ = SystemParametersInfoW(
                        SPI_GETCARETTIMEOUT,
                        0,
                        Some(&mut timeout_ms as *mut u32 as *mut std::ffi::c_void),
                        SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
                    );
                }
                if self.last_input.elapsed().as_millis() as u64 >= timeout_ms.max(1) as u64 {
                    self.editor.kill_timers();
                    self.editor.force_caret_shown();
                    unsafe {
                        let _ = KillTimer(Some(self.hwnd), TIMER_CARET_BLINK);
                    }
                    self.mark_dirty("caret-timeout");
                    self.present_if_dirty().ok();
                } else {
                    self.editor.toggle_caret();
                    self.mark_dirty("caret-blink");
                    self.present_if_dirty().ok();
                }
            }
            TIMER_TOOLTIP => unsafe {
                let _ = KillTimer(Some(self.hwnd), TIMER_TOOLTIP);
            },
            id if id >= EDIT_TIMER_BASE => {
                // richedit-requested timer: forward WM_TIMER
                let _ = self.editor.send(WM_TIMER, id - EDIT_TIMER_BASE, 0);
                self.process_editor_events();
                self.present_if_dirty().ok();
            }
            _ => {}
        }
    }
}

fn make_lparam(x: i32, y: i32) -> isize {
    ((y as u32) << 16 | (x as u32 & 0xFFFF)) as isize
}

fn clipboard_write(text: &str) -> Result<()> {
    use windows::Win32::System::DataExchange::*;
    use windows::Win32::System::Memory::*;
    use windows::Win32::System::Ole::CF_UNICODETEXT;
    unsafe {
        OpenClipboard(None)?;
        EmptyClipboard()?;
        let w: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
        let h = GlobalAlloc(GMEM_MOVEABLE, w.len() * 2)?;
        let p = GlobalLock(h);
        std::ptr::copy_nonoverlapping(w.as_ptr(), p as *mut u16, w.len());
        let _ = GlobalUnlock(h);
        SetClipboardData(CF_UNICODETEXT.0 as u32, Some(HANDLE(h.0)))?;
        CloseClipboard()?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// wndproc
// ---------------------------------------------------------------------------

thread_local! {
    static APP_PTR: RefCell<*mut App> = const { RefCell::new(std::ptr::null_mut()) };
}

fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP_PTR.with(|p| {
        let p = *p.borrow();
        if p.is_null() {
            None
        } else {
            Some(unsafe { f(&mut *p) })
        }
    })
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_NCHITTEST => with_app(|app| {
                let mut pt = POINT {
                    x: ((l.0 as i32) as i16) as i32,
                    y: (((l.0 as i32) >> 16) as i16) as i32,
                };
                let _ = ScreenToClient(hwnd, &mut pt);
                let p = app.dip(pt.x, pt.y);
                match hit_test(&app.state, &app.layout, p) {
                    Hit::Outside => LRESULT(HTTRANSPARENT as isize),
                    _ => LRESULT(HTCLIENT as isize),
                }
            })
            .unwrap_or(LRESULT(HTTRANSPARENT as isize)),
            WM_PAINT => {
                with_app(|a| {
                    a.paint_count += 1;
                    a.present_if_dirty().ok();
                });
                let _ = ValidateRect(Some(hwnd), None);
                LRESULT(0)
            }
            WM_ERASEBKGND => LRESULT(1),
            WM_MOUSEMOVE => LRESULT(
                with_app(|a| {
                    a.wm_mousemove(
                        ((l.0 as i32) as i16) as i32,
                        (((l.0 as i32) >> 16) as i16) as i32,
                    )
                })
                .map(|_| 0)
                .unwrap_or(0) as isize,
            ),
            WM_LBUTTONDOWN => {
                with_app(|a| {
                    a.wm_lbuttondown(
                        ((l.0 as i32) as i16) as i32,
                        (((l.0 as i32) >> 16) as i16) as i32,
                    )
                });
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                with_app(|a| {
                    a.wm_lbuttonup(
                        ((l.0 as i32) as i16) as i32,
                        (((l.0 as i32) >> 16) as i16) as i32,
                    )
                });
                LRESULT(0)
            }
            WM_MOUSEWHEEL => {
                let _ = with_app(|a| a.editor.send(WM_MOUSEWHEEL, w.0, l.0));
                LRESULT(0)
            }
            WM_CHAR | WM_DEADCHAR | WM_SYSCHAR => {
                LRESULT(with_app(|a| a.wm_char(w.0, l.0)).unwrap_or(0))
            }
            WM_KEYDOWN | WM_SYSKEYDOWN => {
                LRESULT(with_app(|a| a.wm_keydown(w.0, l.0)).unwrap_or(0))
            }
            WM_KEYUP | WM_SYSKEYUP => {
                // keyups only matter to the editor (it tracks modifiers) —
                // never run keydown logic on them.
                let _ = with_app(|a| a.editor.send(msg, w.0, l.0));
                LRESULT(0)
            }
            WM_IME_STARTCOMPOSITION
            | WM_IME_ENDCOMPOSITION
            | WM_IME_COMPOSITION
            | WM_IME_NOTIFY
            | WM_IME_SETCONTEXT
            | WM_IME_CHAR
            | WM_IME_REQUEST => LRESULT(
                with_app(|a| {
                    if a.char_trace.len() < 512 {
                        a.char_trace.push(0x300_0000 | msg);
                    }
                    if msg == WM_IME_STARTCOMPOSITION {
                        a.state.composing = true;
                    } else if msg == WM_IME_ENDCOMPOSITION {
                        a.state.composing = false;
                    }
                    let r = a.editor.send(msg, w.0, l.0);
                    a.process_editor_events();
                    if a.dirty {
                        a.present_if_dirty().ok();
                    }
                    r
                })
                .unwrap_or(0),
            ),
            WM_INPUTLANGCHANGE | WM_SETCURSOR => {
                LRESULT(with_app(|a| a.editor.send(msg, w.0, l.0)).unwrap_or(0))
            }
            WM_GETOBJECT => {
                // UIA root request: answer with the fragment root wrapping
                // the windowless RichEdit provider (see uia.rs).
                use windows::Win32::UI::Accessibility::*;
                if l.0 as i32 == UiaRootObjectId {
                    let r = with_app(|a| {
                        a.editor
                            .uia_root()
                            .and_then(|root| root.cast::<IRawElementProviderSimple>().ok())
                            .map(|el| UiaReturnRawElementProvider(hwnd, w, l, &el))
                    });
                    r.flatten().unwrap_or(LRESULT(0))
                } else {
                    LRESULT(0)
                }
            }
            WM_SETFOCUS => {
                with_app(|a| a.focus_editor());
                LRESULT(0)
            }
            WM_KILLFOCUS => {
                with_app(|a| a.blur_editor());
                LRESULT(0)
            }
            WM_TIMER => {
                with_app(|a| a.wm_timer(w.0));
                LRESULT(0)
            }
            WM_DPICHANGED => {
                with_app(|a| {
                    let dpi = (w.0 >> 16) as u32;
                    a.scale = dpi as f32 / 96.0;
                    let _ = a.editor.set_scale(a.scale);
                    a.measure_editor = true;
                    let _ = a.rebuild_sprite();
                    a.relayout().ok();
                    let rc = &*(l.0 as *const RECT);
                    let _ = SetWindowPos(
                        hwnd,
                        Some(HWND_TOPMOST),
                        rc.left,
                        rc.top,
                        rc.right - rc.left,
                        rc.bottom - rc.top,
                        SWP_NOACTIVATE,
                    );
                    a.present_if_dirty().ok();
                });
                LRESULT(0)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, w, l),
        }
    }
}

/// Creates the transparent UI window and wires `app` into the wndproc.
/// Returns the HWND; the caller drives the message loop (`pump`/`pump_once`).
/// `app` must outlive the window (leak via `Box::leak` in `run` below).
pub fn create(app: &'static mut App, anchor_right: i32, anchor_bottom: i32) -> Result<HWND> {
    create_opts(app, anchor_right, anchor_bottom, false)
}

/// `create` with activation control: `no_activate` adds WS_EX_NOACTIVATE and
/// shows without focusing (used by the perf harness for "unfocused" idle).
pub fn create_opts(
    app: &'static mut App,
    anchor_right: i32,
    anchor_bottom: i32,
    no_activate: bool,
) -> Result<HWND> {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let hinst = register_class(w!("MascotUiV01"), Some(wndproc))?;
        app.relayout()?;
        let px = app.layout.window_px();
        let x0 = match app.state.placement {
            mascot_ui::state::Placement::Right => anchor_right - px[0] as i32,
            mascot_ui::state::Placement::Left => anchor_right,
        };
        let ex = if no_activate {
            WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE
        } else {
            WS_EX_NOREDIRECTIONBITMAP | WS_EX_TOPMOST | WS_EX_TOOLWINDOW
        };
        let hwnd = CreateWindowExW(
            ex,
            w!("MascotUiV01"),
            w!("mascot"),
            WS_POPUP
                | if no_activate {
                    WINDOW_STYLE(0)
                } else {
                    WS_VISIBLE
                },
            x0,
            anchor_bottom - px[1] as i32,
            px[0] as i32,
            px[1] as i32,
            None,
            None,
            Some(hinst),
            None,
        )?;
        if no_activate {
            let _ = ShowWindow(hwnd, SW_SHOWNA);
        }
        let surf = CompSurface::new(&app.renderer, hwnd, px, app.scale)?;
        app.attach_window(surf);
        app.anchor = POINT {
            x: anchor_right,
            y: anchor_bottom,
        };
        app.relayout()?;
        APP_PTR.with(|p| *p.borrow_mut() = app);
        let _ = app.present_if_dirty();
        Ok(hwnd)
    }
}

/// Pumps the thread queue once; returns false on WM_QUIT.
pub fn pump_once() -> bool {
    unsafe {
        let mut msg = MSG::default();
        while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
            if msg.message == WM_QUIT {
                return false;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        true
    }
}

/// Blocking message pump until WM_QUIT/destroy.
pub fn pump() {
    unsafe {
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

/// Detaches the app pointer (call after the window is destroyed).
pub fn unbind() {
    APP_PTR.with(|p| *p.borrow_mut() = std::ptr::null_mut());
}

/// Creates the transparent UI window + message loop; returns after the
/// window closes.
pub fn run(app: App, anchor_right: i32, anchor_bottom: i32) -> Result<()> {
    let app = Box::leak(Box::new(app));
    let r = create(app, anchor_right, anchor_bottom).map(|_| ());
    if r.is_ok() {
        pump();
    }
    unbind();
    r
}
