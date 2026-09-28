# Visual sanity-check notes — Windows Stage A

Package produced by `benchmark/harness/capture_visuals.py` on the final
correctness-ready binaries (`out/{rust,zig,go}/mascot.exe`, subsystem
`windowsgui`). All captures are external screen crops of the virtual
display (plain Windows desktop background visible behind transparent
pixels). Videos are raw `ffmpeg -f gdigrab` captures at 15 fps, h264,
no narration or edits; each is ~24.3 s.

Setup identical for all three: cursor launch point (3840+512, 300) on the
1024x768@100% virtual display; typed text `Ārā līst, mēs turpinām. 世界 👨‍💻`;
scenario text `visual check`; fixture `windows-v1.0.2`. DPI shots at the
virtual display raised to 3840x2160 with scale set to 125% then 150%
(restored to 1024x768@100% afterward).

## Transparency quality

- All three render the mascot through a layered per-pixel-alpha window;
  the desktop shows cleanly around the silhouette on the same wallpaper.
- No background fill, matte, or rectangular artifact in any candidate.

## Edge quality / alpha halo

- Silhouette edges show the source asset's own antialiased rim; no
  white/black halo observed against the dark wallpaper in any candidate.
- Rust and Go captures show the mascot at the same size/position; Zig
  identical.

## Mascot scaling

- 64-dip logical size honored: mascot is visibly larger in dpi-125 and
  dpi-150 captures (WM_DPICHANGED handled live without relaunch).
- No resolution-dependent raster stepping visible at 150%; source is a
  1254x1254 asset resampled at presentation.

## Text rendering

- Input: `Ārā līst, mēs turpinām.` and `世界` render correctly in all
  three; combining marks attach correctly (`Ā`, `ī`).
- Emoji (`👨‍💻`, fixture 🔒/🐧-line glyphs) render as monochrome outline
  glyphs in all three — the RichEdit/control stack does not emit color
  emoji (expected limitation of the native edit control without
  DirectWrite); identical across candidates.
- Response text is the frozen fixture script in all three (numbered
  `NNN|Ā ...` chunks including Arabic shaping line).

## Font differences

- All three use Segoe UI ~16 dip per the fixture. Rust and Go input text
  sits at identical metrics. Zig input text renders at the same family
  but the input area is an unbordered region of the window body rather
  than a framed edit row.

## Composer geometry

- Rust and Go composers are visually equivalent: caption `mascot`,
  mascot thumbnail top-center, bordered input row on top, response area
  middle, status strip (`idle`/`streaming`/`complete`) + `Send`/`Cancel`
  buttons at the bottom.
- Zig composer differs: caption `Mascot Composer`, no mascot thumbnail,
  no status strip, no Send/Cancel buttons — the window is a single
  bordered region with the unbordered input at top and response below.
- Composer size on screen is the same logical size in all three at the
  same DPI; positions differ only because they anchor to the mascot.

## Caret/selection appearance

- Caret visible as standard native I-beam/blinking caret in all three.
- `CTRL+HOME` + `SHIFT+END` produces the system highlight color over the
  full typed line in all three (visible in the demo videos).

## Response rendering

- Chunks append live with the response autoscrolled to the tail; the
  fixture transcript (`User:/Assistant:` seed plus numbered chunks)
  renders identically byte-wise in all three.
- Rust and Go show `streaming`→`complete` in the status strip; Zig has
  no visible status indicator (behavior verified via events; visual
  absence noted).

## DPI behavior

- All three rescale composer and mascot on the live 100%→125%→150%
  transition (shots taken without restart); window chrome, fonts and
  mascot pixels scale proportionally. No clipping or layout break
  observed at 150%.

## Focus/activation behavior

- Hotkey `CTRL+ALT+SPACE` opens the composer and focuses its input in
  all three (typing lands immediately — verified in videos).
- Rust and Go composer shows standard focused caption. Zig same.

## Drag behavior

- Mascot drags smoothly via the system HTCAPTION path in all three
  (visible in videos); click-through is honored — a right-click on a
  transparent corner pixel reaches the desktop (context menu appears).

## Flicker

- None observed during launch, show/hide, drag, streaming append, or DPI
  change in any candidate. No repaint loop artifacts (R6 paints delta
  was 0 in the benchmark).

## Layout differences between implementations

| Aspect | Rust | Zig | Go |
|---|---|---|---|
| Composer caption | `mascot` | `Mascot Composer` | `mascot` |
| Mascot thumbnail in composer | yes | no | yes |
| Status strip (idle/streaming) | yes | no | yes |
| Send/Cancel buttons | yes | no | yes |
| Input row | bordered edit | unbordered region | bordered edit |

These are permitted implementation variations within the frozen
acceptance matrix (all T1-T14/W1-W9 cases pass on all three); they are
recorded as evidence, not scored.

## Defects discovered

- None blocking. Notes only: color emoji are not rendered by any
  candidate (monochrome fallback — same platform limitation in all
  three); the response view contains the fixture's pre-seeded
  `User:/Assistant:` transcript on first open in all three, which is
  fixture-defined behavior.
