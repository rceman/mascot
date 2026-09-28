//! Flip-model swap chain for an HWND, exposed as a Direct2D target bitmap.
//!
//! Frames are only presented when the host asks (after an invalidation or an
//! animation step); there is no render thread and no continuous loop.

use crate::renderer::{Renderer, bitmap_props};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::core::{Interface, Result};

pub struct WindowTarget {
    pub swap_chain: IDXGISwapChain1,
    pub bitmap: Option<ID2D1Bitmap1>,
    pub size: [u32; 2],
}

impl WindowTarget {
    pub fn new(r: &Renderer, hwnd: HWND, size: [u32; 2]) -> Result<Self> {
        unsafe {
            let dxgi_dev: IDXGIDevice = r.d3d.cast()?;
            let adapter = dxgi_dev.GetAdapter()?;
            let factory: IDXGIFactory2 = adapter.GetParent()?;
            let desc = DXGI_SWAP_CHAIN_DESC1 {
                Width: size[0].max(1),
                Height: size[1].max(1),
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                Stereo: false.into(),
                SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                BufferCount: 2,
                Scaling: DXGI_SCALING_NONE,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
                AlphaMode: DXGI_ALPHA_MODE_IGNORE,
                Flags: 0,
            };
            let swap_chain = factory.CreateSwapChainForHwnd(&r.d3d, hwnd, &desc, None, None)?;
            let mut t = WindowTarget { swap_chain, bitmap: None, size: [desc.Width, desc.Height] };
            t.bind(r)?;
            Ok(t)
        }
    }

    fn bind(&mut self, r: &Renderer) -> Result<()> {
        unsafe {
            let surface: IDXGISurface = self.swap_chain.GetBuffer(0)?;
            let bmp = r.ctx.CreateBitmapFromDxgiSurface(
                &surface,
                Some(&bitmap_props(D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW)),
            )?;
            self.bitmap = Some(bmp);
        }
        Ok(())
    }

    pub fn resize(&mut self, r: &Renderer, size: [u32; 2]) -> Result<()> {
        let size = [size[0].max(1), size[1].max(1)];
        if size == self.size {
            return Ok(());
        }
        unsafe {
            r.ctx.SetTarget(None);
            self.bitmap = None;
            self.swap_chain.ResizeBuffers(0, size[0], size[1], DXGI_FORMAT_UNKNOWN, DXGI_SWAP_CHAIN_FLAG(0))?;
        }
        self.size = size;
        self.bind(r)
    }

    pub fn present(&self) -> Result<()> {
        unsafe { self.swap_chain.Present(1, DXGI_PRESENT(0)).ok() }
    }
}
