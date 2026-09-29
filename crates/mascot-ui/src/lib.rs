//! Platform-independent semantics for the Mascot native UI (M1A).
//!
//! This crate owns everything about the product UI that does not require an
//! OS: geometry semantics, theme tokens, the state model, layout, composer
//! sizing and hit testing. The Windows layer (`mascot-ui-win32`) turns these
//! results into Direct2D/DirectWrite/RichEdit calls and feeds measured text
//! sizes back in.
//!
//! # Coordinates (DIP)
//!
//! All layout and hit testing is in **device-independent pixels** (DIP): 1 DIP
//! = 1/96 inch. The origin is the top-left corner of the window's client area,
//! y grows downward. Conversion to device pixels is `px = dip * scale` where
//! `scale = dpi / 96` (1.0, 1.25, 1.5, 2.0 for the validated DPI steps).
//! [`snap`] snaps a DIP coordinate to the device pixel grid; the 1-DIP border
//! is always painted on a snapped edge so it stays crisp at every scale.
//!
//! # Tokens
//!
//! Visual constants live in [`tokens`]: shadcn-neutral palette per
//! [`Theme`], radii (sm 6 / md 8 / lg 10 / xl 14, bubble 14), spacing steps
//! (4/8/12/16/20), text sizes (body 14/20, small 12/16), icon size (16 DIP on
//! a 24 grid, stroke 2), control sizes (32 primary icon button, 28 ghost),
//! composer bounds (width 380, single-line 52, max 6 text lines) and the
//! bubble shadow margin. Deviations from raw shadcn values are documented on
//! the token that carries them.
//!
//! # Controls and state
//!
//! [`UiState`] is a pure value. Interaction state uses the standard meanings:
//!
//! - **hover**: pointer is over the control's hit rect (accent fill);
//! - **pressed**: pointer is held down over it (one step stronger than hover);
//! - **focus**: the control owns keyboard focus;
//! - **focus-visible**: focus was reached by keyboard, so a 3 DIP focus ring
//!   (ring colour at 50 %) plus ring-coloured border is drawn — never for
//!   pointer focus;
//! - **disabled**: send with an empty editor — muted background and icon, no
//!   hover/pressed response, and it is not keyboard-focusable.
//!
//! [`ControlId`] enumerates the only controls: `Editor`, `Send`, `Stop`,
//! `Copy`. Send and Stop share one rect and are never visible together; Stop
//! replaces Send while [`Activity::Submitting`].
//!
//! # Composer sizing rule
//!
//! [`composer_height`] maps measured editor content height to composer height:
//! `clamp(content_h + 2 * EDITOR_PAD_Y, COMPOSER_MIN_H, COMPOSER_MAX_H)`.
//! Single-line text is vertically centred; beyond the 6-line maximum the text
//! scrolls inside the editor while the composer stays at the maximum.
//!
//! # Enter / Shift+Enter rule
//!
//! Implemented by the Win32 layer but fixed here: plain **Enter** submits when
//! the editor is non-empty and no IME composition is active (Enter while
//! composing commits the IME instead); **Shift+Enter** inserts a newline;
//! Enter on an empty editor is a no-op; Ctrl+Enter behaves like Enter.
//!
//! # Icons
//!
//! Icons are [`mascot_icons::Icon`] values (upstream Lucide geometry). Usage:
//! `Send` -> `Icon::ArrowUp`, `Stop` -> `Icon::Square`, `Copy` -> `Icon::Copy`,
//! copied state -> `Icon::Check` (label `Icon::label`).

pub mod geom;
pub mod layout;
pub mod state;
pub mod theme;

pub use geom::{Point, Rect, snap};
pub use layout::{Hit, Layout, MascotMetrics, Measured, composer_height, hit_test, layout};
pub use state::{Activity, ControlId, Interaction, Placement, Surface, UiState};
pub use theme::{Palette, Theme};

/// Fixture response text for the deterministic `Response` surface (plain text,
/// bounded, no Markdown, a little Unicode on purpose).
pub const RESPONSE_FIXTURE: &str = "On it — the fixture harness is green and the diff is under review.\n\nSummary: 3 files changed, 42 assertions, no regressions. Über alles deterministic ✓ (日本語でも読める).";

/// Placeholder shown in an empty, unfocused composer.
pub const PLACEHOLDER_ASK: &str = "Ask anything…";
/// Placeholder for the follow-up composer under a response.
pub const PLACEHOLDER_FOLLOWUP: &str = "Ask follow-up…";
