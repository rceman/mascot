# Native UI v0.1 — visual review notes and known limitations

Candidate: `b285fd0` (all evidence in this directory was generated from the
clean tree at this HEAD; see `receipt.json` and `perf.json` `env`).

## Review method

- Every state in `contact-sheet-light.png` and `contact-sheet-dark.png`
  inspected at 1:1, and the `zoom/` crops at 3x.
- Every cell of `dpi-contact-sheet.png` (100/125/150/200%) inspected,
  including all 200% cells at 2x. `capture` also validates each DPI cell
  itself (editor line height, text ink span and caret stay inside the
  editor rect). The command fails if any invariant is violated.
- `selftest/selftest-screen.png` is a real screen capture of the live DComp
  window. The desktop shows through outside the bubble and mascot, which
  confirms true per-pixel transparency (no opaque backing).
- `icon-sheet.png`: all 12 Lucide icons at four sizes in both themes.
  The source SVGs are byte-identical to upstream Lucide at tag `0.544.0`
  (commit `ec567e59`). This was verified against
  raw.githubusercontent.com during review.

## Findings (final candidate)

No open visual defects in the declared range: light and dark, left and
right placement, scales 1.0/1.25/1.5/2.0.

- **Perch/overlap:** the mascot overlaps the bubble's top edge by 12 DIP at
  its near edge. The right placement mirrors the mascot position and facing
  only; bubble internals stay LTR (text left, Send/Stop right, Copy right).
- **Borders:** a 1-device-pixel hairline, snapped at every scale. There is
  no blur at 125/150%. When the editor has focus, the bubble border takes
  the ring tone (shadcn input focus treatment).
- **Editor:** editor text and caret are sharp and correctly scaled at all
  four scales. Multiline grows to 6 lines, then scrolls. The selection is
  neutral grey in both themes. The placeholder is muted and aligned to the
  text baseline.
- **Controls:** Send/Stop is a filled 32 DIP primary button and Copy is a
  28 DIP ghost button. Hover, pressed, disabled and focus-visible each read
  distinctly. The focus ring is shown only for keyboard focus and never on
  the editor.
- **Submitting:** the submitted text is dimmed and read-only, and Stop
  replaces Send in the same rect.
- **Response:** the response text wraps at the 348 DIP measure (the text
  width), with a separator line and an empty follow-up composer below it.
  Copy shows a 500 ms hover tooltip, then a "Copied" tooltip and check icon
  that revert after 1.5 s.
- **Icons:** stroke 2/24 scaled to 16 DIP with round caps and joins, and
  optically centred in their buttons at all scales.

## Recorded deviations from the shadcn neutral baseline

| Deviation | Reason |
|---|---|
| Dark bubble surface `#171717` (card) instead of background `#0A0A0A` | The bubble floats over an arbitrary desktop, so the elevated tone keeps luminance separation |
| Pressed tokens (light `#E5E5E5`, dark white @22%) | shadcn has no pressed token; one step stronger than hover |
| Selection colours are pre-blended opaque (`#BDBDBD` / `#4A4A4A`) | The RichEdit syscolor path takes `COLORREF`, which has no alpha (native editor constraint) |
| One shadow layer (sigma 4, +4 DIP, alpha .14 light / .30 dark) instead of shadcn's two layers | Cached single D2D shadow bitmap; visually matched against the two-layer look over light and dark desktops |
| Send/Stop is a filled primary icon button inside the composer | Primary action affordance in a single-row composer without a text label |

## Known limitations

1. **Cold first show ~556–604 ms** (hardware, median 566.9 ms). About
   260 ms of that is D3D11/D2D device creation, and about 60–100 ms is the
   first rig raster (D2D effect shader compilation). The GDI benchmark
   starts in about 94 ms, but it does not create a D3D/DComp/RichEdit
   stack. Warm open is about 3.5 ms p50. Startup was not optimized in M1A;
   a resident process makes this a one-time cost.
2. **Memory ~84–90 MB private at steady state.** This is the shared
   D3D11/D2D driver baseline, in line with the existing
   `animation-rig-v0.2` D2D lab (~97–102 MB). The app's own share is about
   3 MB. `IDXGIDevice3::Trim` + `ID2D1Device::ClearResources` reclaim about
   62 MB after first show and on hide. The +16 threads are driver-owned.
3. **Capture pixels come from WARP**, used for determinism. Perf and
   interactive runs use the hardware device. Antialiasing can differ by a
   sub-pixel between the two.
4. **DPI is validated by explicit scale** (offscreen capture at 4 scales,
   plus the `app_scale_switch` and `dpi_sheet_replay` probes that switch
   scale on a live app). The `WM_DPICHANGED` path is implemented but was
   not exercised by physically moving the window between monitors with
   different DPI.
5. **Accessibility:** UIA exposes the composer editor (Document with
   Value/Text patterns via RichEdit's own windowless provider). Send, Stop
   and Copy are reachable by keyboard (Tab/Enter) and have tooltip names,
   but they are not separate UIA elements. The design system says not to
   add a custom accessibility framework in v0.1.
6. **Response text is DirectWrite-painted and not selectable.** It can be
   copied with the Copy control. The response content is a fixed fixture
   (mock response by scope).
7. **Selftest prerequisites:** an interactive foreground desktop session
   plus the Latvian Standard and Microsoft Japanese IME keyboard layouts.
   It injects real input (SendInput) and saves and restores the clipboard
   and cursor.
8. **Harness-only unsafe aliasing:** the lab and probes drive the `App`
   through a raw pointer shared with the wndproc. All accesses are
   per-statement borrows (no `&mut` held across message pumps), and the
   product `with_app` refuses re-entrant borrows. In practice the guard
   did not fire (`reentry_skips` stayed 0).
