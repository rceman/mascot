# Visual review — native UI component gallery v0.1

Candidate: `03eae71` (receipt `head=03eae71`, `dirty=false`). All sheets were inspected at full
resolution (region crops, not downsampled views). The final sheets are byte-identical to the
last reviewed dev capture (r4), and the r4→r5 cleanup did not change a single sheet pixel.

## Review rounds

The first pass and three rework rounds found and fixed the following. Each fix now has a self-check
in `validate`, so the run fails if it regresses.

| # | Finding | Severity | Fix |
|---|---|---|---|
| 1 | Captions/headings clipped by one glyph (`defaul`, `Typograph`, widths `32/38/44`) | ERROR | NO_WRAP text layouts at measured width; self-check ink ≥ 0.9 × measured |
| 2 | Mascot sprite leaking into every Composer/Response cell | ERROR | sprite detached for component cells; 0 mascot-orange px self-check |
| 3 | Ghost/unfilled controls ink-cropped and top-aligned vs filled siblings | ERROR | fixed cell boxes (rect + pad), equal-height rows |
| 4 | Dark cells invisible on the white reference page | ERROR | every cell flattened over its theme's surface; light/dark bands |
| 5 | Overflow composer showed 1 line → then lines 6–9 at the top with blank space | ERROR | editor re-measured after set_text; exact `EM_LINESCROLL` bottom anchoring; self-check `client_h − line_h ≤ last_bottom ≤ client_h` |
| 6 | Reference "default" Button was actually the outline variant (`button-demo`) | ERROR | `button-default` example; `data-variant`/`data-size` asserted and recorded |
| 7 | Reference computed styles read from the wrong theme / from the tooltip trigger | ERROR | styles read after theme + state settle; tooltip reads `[data-slot=tooltip-content]` |
| 8 | Textarea measured at 16 px (viewport below Tailwind `md`) | ERROR | viewport 1200, element width set inline to 380 px (recorded) |
| 9 | Reference crops flush top/left, neighbour badge intruding | ERROR | ±16 CSS px on all sides asserted; same-slot neighbours hidden (recorded) |
| 10 | Translucent colour hex wrong (`#F5FFFF1A`) | ERROR | exact RGB from opaque form; script self-test `#FFFFFF1A` / `#FFFFFF26` |
| 11 | Caption bands overlapping cell bottoms (tooltip/badge corners cut) | ERROR | captions strictly below cells; intersection self-check |
| 12 | Long tooltip text touching the pill edges (production painter) | ERROR | ellipsis layout inset by `TOOLTIP_PAD_X`; ink-inset self-check |
| 13 | Native Surface cells without the production shadow | ERROR | `Painter::prepare` + `Painter::shadow` in the cell; shadow-pixel self-check |
| 14 | Native badge 18 DIP / 8 DIP radius vs measured shadcn pill 22 DIP | ERROR | badge aligned: 22 DIP pill, 8 DIP pad-x, 12/500, 1 DIP border |
| 15 | Native comparison values approximated (`~18 DIP`) and colours re-derived in the lab | ERROR | measured from the drawn cell; colours from the shared `mascot_ui::component::*_colors` |
| 16 | Receipt reported `dirty=true` for in-repo captures | ERROR | dirty sampled before the staging dir exists |

## Final sheet review

- **component-gallery-light / -dark** (100 %, 1 px = 1 DIP). All 10 Tier A components appear in
  inventory order, with every state in the inventory and correct light/dark tokens. Hover, pressed and
  focus-visible are distinguishable for IconButton and Button. The copied state shows the Check icon.
  Composer: placeholder, focused, text, selection, multiline (4), overflow (9→6, caret line at the bottom),
  and submitting (read-only + Stop). Response: default, copied, and follow-up text. No ERROR findings.
- **component-gallery-sizes**. Fixed controls appear only at their supported sizes: IconButton 28/32,
  Button sm 32 / default 36, Icon 16. Content-sized controls show short/medium/long content: Button,
  Tooltip (long clamps to 240 DIP with a padded ellipsis), and Badge. Stretchable components are shown
  at 320/380/440 DIP: Surface, Separator, Composer, and Response. The self-check confirms that the
  requested width equals the layout width. No ERROR findings.
- **component-gallery-dpi**. 100/125/150/200 % in light and dark bands. Hairlines stay one device
  pixel, and editor text/caret stay inside the editor at every scale. No ERROR findings.
- **component-gallery-shadcn-reference (1–5)** use refs at DPR 2 and native at 200 %, with
  `shadcn | native` in light and dark bands. Each native cell uses the same content as its ref, and
  sizes are asserted equal where meant (Surface = card box, Separator 380, Composer 380, and
  Typography wrap width). Every deviation listed under each block is lead-authored from
  `shadcn-comparison.md`.

## Accepted WARN findings (reviewed, not defects)

- Native label text sits about 1 DIP lower than the ref in badge/button pills. DirectWrite centres
  the natural line box, whereas CSS centres the `line-height` box. This is within glyph-metric
  differences (Segoe UI Variable vs Geist).
- In dark mode the ref crops keep shadcn's page background (#0A0A0A), while native cells sit on the
  Mascot surface (#171717). This is intentional, so both sources are shown as-is.
- The live `/view` example site renders `--primary` as #000000 (light) and `--accent` as #404040 (dark).
  Mascot tokens follow the documented neutral theme (ui.shadcn.com/docs/theming: `oklch(0.205 0 0)` =
  #171717, `oklch(0.269 0 0)` = #262626). Recorded as a deviation.
- The Tooltip uses primary/primary-foreground instead of shadcn's foreground/background. It is an
  unjustified deviation and a follow-up candidate. It was left unchanged so the foundation pixels
  stay identical (100/100).

## Limitations

- Reference PNGs are developer evidence from the live site (Chrome 154, upstream commit in
  `reference/provenance.json`). The product never reads them.
- The in-repo `perf.json` run hit a noisy cold-start window (first show 753–1108 ms). The controlled,
  strictly interleaved A/B in `logs/perf-ab/` (10 runs each) gives a first-show median of 626 ms at
  `4ab080d` vs 606 ms for this branch. Warm open is 3.78 vs 3.75 ms and submit→response 6.15 vs 6.16 ms,
  so there is no regression. Static idle is still 0 presents.
- Gallery pixels come from WARP for determinism. Composer/Response cells render through the real
  `App::render_offscreen` and windowless RichEdit, and primitives through the production `Painter`
  methods. Sheet captions/headings are gallery framing only.
