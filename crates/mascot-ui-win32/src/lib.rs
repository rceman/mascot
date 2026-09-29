//! Windows host for `mascot-ui`: DirectComposition transparent window,
//! Direct2D/DirectWrite painting, windowless RichEdit composer, mascot anchor.
//!
//! Only compiled on Windows.

#[cfg(windows)]
pub mod app;
#[cfg(windows)]
pub mod components;
#[cfg(windows)]
pub mod edit;
#[cfg(windows)]
pub mod icons;
#[cfg(windows)]
pub mod paint;
#[cfg(windows)]
pub mod sprite;
#[cfg(windows)]
pub mod text;
#[cfg(windows)]
pub mod uia;
#[cfg(windows)]
pub mod window;
