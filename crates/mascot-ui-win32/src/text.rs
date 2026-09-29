//! DirectWrite helpers: font-family resolution, cached text formats and
//! measuring for the pure layout (`Measured`) inputs.
//!
//! Font choice per the design doc: "Segoe UI Variable Text" when present,
//! falling back to "Segoe UI". The same family is handed to RichEdit so the
//! editor's glyphs match DWrite text exactly at every scale.

use windows::Win32::Graphics::DirectWrite::*;
use windows::core::*;

/// Resolves the UI font family against the system collection.
pub fn resolve_family(dwrite: &IDWriteFactory) -> Result<String> {
    unsafe {
        let mut fonts: Option<IDWriteFontCollection> = None;
        dwrite.GetSystemFontCollection(&mut fonts, false)?;
        let Some(fonts) = fonts else {
            return Ok("Segoe UI".to_string());
        };
        for name in ["Segoe UI Variable Text", "Segoe UI"] {
            let wname: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
            let mut exists = BOOL::default();
            let mut idx = 0u32;
            if fonts
                .FindFamilyName(PCWSTR(wname.as_ptr()), &mut idx, &mut exists)
                .is_ok()
                && exists.as_bool()
            {
                return Ok(name.to_string());
            }
        }
        Ok("Segoe UI".to_string())
    }
}

/// Cached formats; sizes are in DIP (DWrite font size unit = DIP at the
/// render target's DPI once `SetDpi` is applied).
pub struct Fonts {
    pub dwrite: IDWriteFactory,
    pub family: String,
    pub body: IDWriteTextFormat,
    pub small: IDWriteTextFormat,
    /// BODY_SIZE at medium weight (control labels).
    pub label: IDWriteTextFormat,
    /// SMALL_SIZE at medium weight (badges, captions).
    pub caption: IDWriteTextFormat,
}

impl Fonts {
    pub fn new(dwrite: &IDWriteFactory) -> Result<Fonts> {
        let family = resolve_family(dwrite)?;
        unsafe {
            let fam: Vec<u16> = family.encode_utf16().chain(Some(0)).collect();
            let make = |size_dip: f32, weight: DWRITE_FONT_WEIGHT| -> Result<IDWriteTextFormat> {
                dwrite.CreateTextFormat(
                    PCWSTR(fam.as_ptr()),
                    None,
                    weight,
                    DWRITE_FONT_STYLE_NORMAL,
                    DWRITE_FONT_STRETCH_NORMAL,
                    size_dip,
                    w!("en-us"),
                )
            };
            Ok(Fonts {
                dwrite: dwrite.clone(),
                family,
                body: make(
                    mascot_ui::theme::tokens::BODY_SIZE,
                    DWRITE_FONT_WEIGHT_NORMAL,
                )?,
                small: make(
                    mascot_ui::theme::tokens::SMALL_SIZE,
                    DWRITE_FONT_WEIGHT_NORMAL,
                )?,
                label: make(
                    mascot_ui::theme::tokens::BODY_SIZE,
                    DWRITE_FONT_WEIGHT_MEDIUM,
                )?,
                caption: make(
                    mascot_ui::theme::tokens::SMALL_SIZE,
                    DWRITE_FONT_WEIGHT_MEDIUM,
                )?,
            })
        }
    }

    /// The cached format for a [`TextStyle`].
    pub fn format(&self, style: mascot_ui::component::TextStyle) -> &IDWriteTextFormat {
        use mascot_ui::component::TextStyle;
        match style {
            TextStyle::Body => &self.body,
            TextStyle::Muted => &self.small,
            TextStyle::Label => &self.label,
            TextStyle::Caption => &self.caption,
        }
    }

    /// Measures `text` laid out at `max_w` DIP; returns (width, height) in DIP.
    pub fn measure(&self, text: &str, max_w: f32, fmt: &IDWriteTextFormat) -> Result<(f32, f32)> {
        unsafe {
            let wtext: Vec<u16> = text.encode_utf16().collect();
            let layout = self.dwrite.CreateTextLayout(&wtext, fmt, max_w, f32::MAX)?;
            let mut m = DWRITE_TEXT_METRICS::default();
            layout.GetMetrics(&mut m)?;
            Ok((m.width, m.height))
        }
    }
}
