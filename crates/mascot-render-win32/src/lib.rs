//! Windows renderer for mascot rig v0.2 (Direct2D 1.1 on a D3D11 device).
//!
//! - fills are composited first; the external outline is generated from the
//!   composited fill silhouette every frame (never baked per part)
//! - internal line art is drawn in z order but excluded from the outline mask
//! - shadows are Direct2D effects derived from the current silhouette
//! - offscreen rendering with WARP gives deterministic evidence renders

pub mod image;
#[cfg(windows)]
pub mod renderer;
#[cfg(windows)]
pub mod swapchain;

#[cfg(windows)]
pub use renderer::{DeviceKind, Layers, RenderOptions, Renderer, ShadowStyle, View};
