//! The DirectComposition transparent window: `WS_POPUP` +
//! `WS_EX_NOREDIRECTIONBITMAP` so there is never an opaque redirection
//! surface; a flip-model premultiplied-alpha swap chain bound through
//! DirectComposition. Verified transparent during the Phase-A spike (magenta
//! backdrop reads through the corners).
//!
//! Frames are event-driven: `present()` is called only when state changed.

use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectComposition::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::core::*;

use mascot_render_win32::renderer::{Renderer, bitmap_props};

/// One DComp-backed window surface. Size is in device pixels; drawing happens
/// in DIP (ctx.SetDpi(96*scale)).
pub struct CompSurface {
    pub hwnd: HWND,
    pub swap: IDXGISwapChain1,
    pub dcomp: IDCompositionDevice,
    pub target: IDCompositionTarget,
    pub visual: IDCompositionVisual,
    pub size: [u32; 2],
    pub dscale: f32,
    bmp: Option<ID2D1Bitmap1>,
}

impl CompSurface {
    /// Creates the composition swap chain + visual rooted at `hwnd`.
    pub fn new(r: &Renderer, hwnd: HWND, size_px: [u32; 2], dscale: f32) -> Result<CompSurface> {
        unsafe {
            let dxgi_dev: IDXGIDevice = r.d3d.cast()?;
            let factory: IDXGIFactory2 = dxgi_dev.GetAdapter()?.GetParent()?;
            let desc = DXGI_SWAP_CHAIN_DESC1 {
                Width: size_px[0].max(1),
                Height: size_px[1].max(1),
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                Stereo: false.into(),
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                BufferCount: 2,
                // STRETCH is required for composition swap chains (NONE fails
                // with DXGI_ERROR_INVALID_CALL on this build).
                Scaling: DXGI_SCALING_STRETCH,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
                AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
                Flags: 0,
            };
            let swap = factory.CreateSwapChainForComposition(&r.d3d, &desc, None)?;
            let dcomp: IDCompositionDevice = DCompositionCreateDevice(&dxgi_dev)?;
            let target = dcomp.CreateTargetForHwnd(hwnd, true)?;
            let visual = dcomp.CreateVisual()?;
            visual.SetContent(&swap)?;
            target.SetRoot(&visual)?;
            dcomp.Commit()?;
            let mut s = CompSurface {
                hwnd,
                swap,
                dcomp,
                target,
                visual,
                size: size_px,
                dscale,
                bmp: None,
            };
            s.bind(r)?;
            Ok(s)
        }
    }

    fn bind(&mut self, r: &Renderer) -> Result<()> {
        unsafe {
            let surface: IDXGISurface = self.swap.GetBuffer(0)?;
            self.bmp = Some(r.ctx.CreateBitmapFromDxgiSurface(
                &surface,
                Some(&bitmap_props(
                    D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
                )),
            )?);
        }
        Ok(())
    }

    /// Resizes the swap chain (call after the window was resized).
    pub fn resize(&mut self, r: &Renderer, size_px: [u32; 2], dscale: f32) -> Result<()> {
        if size_px == self.size && dscale == self.dscale && self.bmp.is_some() {
            return Ok(());
        }
        unsafe {
            r.ctx.SetTarget(None);
            self.bmp = None;
            if size_px != self.size {
                self.swap.ResizeBuffers(
                    0,
                    size_px[0].max(1),
                    size_px[1].max(1),
                    DXGI_FORMAT_UNKNOWN,
                    DXGI_SWAP_CHAIN_FLAG(0),
                )?;
                self.size = size_px;
            }
            self.dscale = dscale;
            self.bind(r)
        }
    }

    /// Runs `draw` against the swap chain target at the current DPI, then
    /// presents + commits. `draw` sees the context between BeginDraw/EndDraw.
    pub fn present(
        &mut self,
        r: &Renderer,
        draw: impl FnOnce(&ID2D1DeviceContext) -> Result<()>,
    ) -> Result<()> {
        unsafe {
            let Some(bmp) = &self.bmp else { return Ok(()) };
            let ctx = &r.ctx;
            ctx.SetTarget(&bmp.cast::<ID2D1Image>()?);
            ctx.SetDpi(96.0 * self.dscale, 96.0 * self.dscale);
            ctx.BeginDraw();
            let res = draw(ctx);
            ctx.EndDraw(None, None)?;
            ctx.SetTarget(None);
            // restore 96 DPI: the shared Renderer assumes it outside frames
            ctx.SetDpi(96.0, 96.0);
            res?;
            self.swap.Present(0, DXGI_PRESENT(0)).ok()?;
            self.dcomp.Commit()?;
        }
        Ok(())
    }
}

/// Window-class registration for the mascot UI window.
pub fn register_class(
    name: PCWSTR,
    wndproc: windows::Win32::UI::WindowsAndMessaging::WNDPROC,
) -> Result<windows::Win32::Foundation::HINSTANCE> {
    use windows::Win32::UI::WindowsAndMessaging::*;
    unsafe {
        let hinst: HINSTANCE =
            windows::Win32::System::LibraryLoader::GetModuleHandleW(None)?.into();
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: wndproc,
            hInstance: hinst,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            lpszClassName: name,
            ..Default::default()
        };
        RegisterClassExW(&wc);
        Ok(hinst)
    }
}
