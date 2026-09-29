//! Universal character pipeline: rig profiles, deterministic reference guides,
//! combined reference boards, character source manifests and validation.

pub mod board;
pub mod draw;
pub mod font;
pub mod guide;
pub mod hash;
pub mod png_io;
pub mod profile;
pub mod report;
pub mod source;
pub mod synth;
pub mod validate;

pub use board::*;
pub use guide::*;
pub use profile::*;
pub use report::*;
pub use source::*;
pub use validate::*;
