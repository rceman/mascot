# Icon provenance

Source: https://github.com/lucide-icons/lucide

- Release tag: `0.544.0`
- Tag commit: `ec567e59fc3be92f82bbb4724d6db4a7992a9f4d`
- Fetched: 2026-09-28
- License: ISC (see `LICENSE` in this directory; upstream Lucide is ISC and
  includes the Feather MIT notice inside the license file)

Files fetched verbatim from `icons/<name>.svg` at that tag:

    arrow-up.svg  square.svg  x.svg  copy.svg  check.svg  paperclip.svg
    plus.svg      mic.svg     ellipsis.svg  maximize-2.svg  history.svg
    settings.svg  LICENSE

`ellipsis` is the current upstream name of the icon formerly called
`more-horizontal`.

## Conversion

Dev-time only, never at runtime:

    cargo run -p mascot-icons --bin mascot-icons-gen

parses the SVG subset Lucide uses (`<path>` `d` with all absolute/relative
commands including H/V/S/Q/T/A, plus `<circle>`, `<ellipse>`, `<rect>`
incl. rx/ry, `<line>`, `<polyline>`, `<polygon>`), converts everything to
absolute `Seg::{M,L,C,Z}` commands in Lucide's 24x24 coordinate space (arcs,
circles, rects and polylines are expanded to cubic/line segments), and rewrites
`src/generated.rs`. The library itself contains no SVG/XML parser; the parser
(`src/svgparse.rs`) is compiled only into the generator binary and into tests.

A unit test regenerates the file in memory from `source/` and asserts
`generated.rs` is byte-identical, so the checked-in data cannot drift from the
upstream SVGs.

Lucide stroke metadata is uniform across the set: stroke-width 2 on a 24x24
grid, round line caps and joins, fill none. The Rust API exposes this as
`Icon::STROKE_WIDTH` and the Win32 layer applies round caps/joins at draw time.
