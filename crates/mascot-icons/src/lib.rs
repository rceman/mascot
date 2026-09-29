//! Typed icon vocabulary for the Mascot native UI.
//!
//! Geometry is upstream **Lucide** path data (ISC licensed, see
//! `source/PROVENANCE.md` and `source/LICENSE`) converted at dev time into
//! compact absolute [`Seg`] commands in Lucide's 24x24 coordinate space.
//! There is no runtime SVG/XML parser and no icon font; renderers turn each
//! [`Seg`] list into native path geometry (Direct2D on Windows) stroked with
//! the icon stroke width and round caps/joins.

mod generated;

#[cfg(test)]
mod svgparse;

/// One absolute path command in Lucide's 24x24 space.
///
/// Arcs, circles, rects, lines and polylines from the source SVGs are expanded
/// to line/cubic segments by the dev-time generator, so renderers only need
/// move/line/cubic/close.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Seg {
    /// Move to (x, y).
    M(f32, f32),
    /// Line to (x, y).
    L(f32, f32),
    /// Cubic Bézier to (x, y) with control points (x1, y1) and (x2, y2).
    C(f32, f32, f32, f32, f32, f32),
    /// Close the current subpath.
    Z,
}

/// The M1A icon vocabulary (Lucide geometry, see [`Icon::name`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Icon {
    ArrowUp,
    Square,
    X,
    Copy,
    Check,
    Paperclip,
    Plus,
    Mic,
    Ellipsis,
    Maximize2,
    History,
    Settings,
}

impl Icon {
    /// Every icon, in a stable order (drives `icon-sheet.png` and the registry
    /// tests).
    pub const ALL: &'static [Icon] = &[
        Icon::ArrowUp,
        Icon::Square,
        Icon::X,
        Icon::Copy,
        Icon::Check,
        Icon::Paperclip,
        Icon::Plus,
        Icon::Mic,
        Icon::Ellipsis,
        Icon::Maximize2,
        Icon::History,
        Icon::Settings,
    ];

    /// Lucide stroke width in 24x24 space (same for every icon upstream).
    pub const STROKE_WIDTH: f32 = 2.0;

    /// Upstream Lucide icon id (`source/<name>.svg`).
    pub fn name(self) -> &'static str {
        match self {
            Icon::ArrowUp => "arrow-up",
            Icon::Square => "square",
            Icon::X => "x",
            Icon::Copy => "copy",
            Icon::Check => "check",
            Icon::Paperclip => "paperclip",
            Icon::Plus => "plus",
            Icon::Mic => "mic",
            Icon::Ellipsis => "ellipsis",
            Icon::Maximize2 => "maximize-2",
            Icon::History => "history",
            Icon::Settings => "settings",
        }
    }

    /// Human-facing label used for tooltips and accessibility names in the
    /// contexts where M1A uses the icon.
    pub fn label(self) -> &'static str {
        match self {
            Icon::ArrowUp => "Send",
            Icon::Square => "Stop",
            Icon::X => "Close",
            Icon::Copy => "Copy",
            Icon::Check => "Copied",
            Icon::Paperclip => "Attach",
            Icon::Plus => "New",
            Icon::Mic => "Microphone",
            Icon::Ellipsis => "More",
            Icon::Maximize2 => "Expand",
            Icon::History => "History",
            Icon::Settings => "Settings",
        }
    }

    /// Absolute path commands in Lucide's 24x24 space.
    pub fn path(self) -> &'static [Seg] {
        match self {
            Icon::ArrowUp => generated::ARROW_UP,
            Icon::Square => generated::SQUARE,
            Icon::X => generated::X,
            Icon::Copy => generated::COPY,
            Icon::Check => generated::CHECK,
            Icon::Paperclip => generated::PAPERCLIP,
            Icon::Plus => generated::PLUS,
            Icon::Mic => generated::MIC,
            Icon::Ellipsis => generated::ELLIPSIS,
            Icon::Maximize2 => generated::MAXIMIZE_2,
            Icon::History => generated::HISTORY,
            Icon::Settings => generated::SETTINGS,
        }
    }
}

/// `(generated constant name, lucide file stem)` in [`Icon::ALL`] order —
/// shared by the generator binary and the in-sync test. Not part of the UI
/// contract; exposed only so `mascot-icons-gen` uses one canonical table.
#[doc(hidden)]
pub const ICON_TABLE: &[(&str, &str)] = &[
    ("ARROW_UP", "arrow-up"),
    ("SQUARE", "square"),
    ("X", "x"),
    ("COPY", "copy"),
    ("CHECK", "check"),
    ("PAPERCLIP", "paperclip"),
    ("PLUS", "plus"),
    ("MIC", "mic"),
    ("ELLIPSIS", "ellipsis"),
    ("MAXIMIZE_2", "maximize-2"),
    ("HISTORY", "history"),
    ("SETTINGS", "settings"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_covers_every_variant() {
        assert_eq!(Icon::ALL.len(), 12);
        let mut seen = std::collections::HashSet::new();
        for i in Icon::ALL {
            assert!(seen.insert(*i), "duplicate {i:?}");
        }
    }

    #[test]
    fn names_are_unique_and_nonempty() {
        let mut seen = std::collections::HashSet::new();
        for i in Icon::ALL {
            assert!(!i.name().is_empty());
            assert!(seen.insert(i.name()), "duplicate name {}", i.name());
        }
    }

    #[test]
    fn paths_are_nonempty_and_in_bounds() {
        for i in Icon::ALL {
            let p = i.path();
            assert!(!p.is_empty(), "{}: empty path", i.name());
            assert!(
                matches!(p[0], Seg::M(..)),
                "{}: path does not start with M",
                i.name()
            );
            for s in p {
                let pts: &[f32] = match s {
                    Seg::M(x, y) | Seg::L(x, y) => &[*x, *y],
                    Seg::C(a, b, c, d, x, y) => &[*a, *b, *c, *d, *x, *y],
                    Seg::Z => &[],
                };
                for v in pts {
                    assert!(
                        (-0.001..=24.001).contains(v),
                        "{}: coord {v} out of 24x24",
                        i.name()
                    );
                }
            }
        }
    }

    /// In-sync guard: `src/generated.rs` must be a byte-identical regen of
    /// `source/*.svg` so checked-in geometry cannot drift from upstream Lucide.
    #[test]
    fn generated_is_in_sync_with_sources() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let regen = svgparse::generate(ICON_TABLE, &dir.join("source")).expect("regenerate");
        let on_disk =
            std::fs::read_to_string(dir.join("src/generated.rs")).expect("read generated.rs");
        assert_eq!(
            regen, on_disk,
            "generated.rs is stale; run `cargo run -p mascot-icons --bin mascot-icons-gen`"
        );
    }
}
