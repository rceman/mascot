# macOS Stage B — Rust vs Go critical slice

Machine: Apple M2, 16 GiB, macOS 26.7 (25G229), arm64.
Base: `7858b54a1f5ec07f370e8e767dc5d751eebbaed6`.
Fixture: native darwin/arm64 build, manifest `macos-v1.0.0`, identical payloads/semantics to Windows.

## Verdict

Both candidates pass the full critical-slice gate on native macOS. No
structural blocker for either stack; differences are in footprint/threads
and engineering glue, detailed in `docs/MACOS_RUST_GO_REPORT.md`.

## Correctness

| Gate | Rust | Go |
|---|---|---|
| Provider scenarios (9) | 9/9 | 9/9 |
| Interactive checks (19) | 19/19 | 19/19 |

Interactive coverage: hotkey show/hide + cancel, composer key focus on
show, ASCII typing, Unicode roundtrip (Latvian/Cyrillic/combining/emoji/
Arabic-bidi), marked-text composition, IME-style commit and cancel,
submit-blocked-while-composing, hide-during-composition, Cmd+Return
submit, plain-Return newline, streamed Unicode response, anchored
mascot/composer drag (perch), transparent-pixel click-through,
own-window screenshots, clean shutdown.

## Resources (single measurement pass each; same power/display)

| Metric | Rust | Go |
|---|---|---|
| Launch → mascot visible | 400 ms (repeats 267–327 ms) | 350 ms (repeats 264–335 ms) |
| First composer activation | 427 ms | 422 ms |
| Warm composer activation | ~325–333 ms | ~159–330 ms |
| Idle mascot RSS / footprint | 64.9 / 35.7 MB | 71.3 / 38.9 MB |
| Composer-open RSS / footprint | 113.0 / 45.9 MB | 100.5 / 48.5 MB |
| Streaming peak RSS / footprint | 120.1 / 73.2 MB | 122.4 / 81.5 MB |
| Idle threads | 9 | 13 |
| Streaming threads | 12 | 17 |
| Idle CPU | 0.0% | 0.0% |
| Stability loops failures | 0 | 0 |
| Clean shutdown | 25.9 ms | 17.4 ms |

## macOS-specific source

| | Rust | Go |
|---|---|---|
| macOS-specific files | 4 | 13 |
| macOS-specific LOC | 1,778 | 1,671 |
| macOS-specific o200k tokens | 15,314 | 14,721 |
| Shared (Windows-carried) LOC | 2,993 | 1,839 |
| Shared tokens | 23,100 | 14,508 |

## ACP compatibility gate (Devin)

`devin acp` spawns; `initialize` and `session/new` both answered for both
candidates. `session/prompt` is rejected because the CLI is not logged in
("Please log in to use Devin"). Status: **UNAVAILABLE** — authentication
sub-gate only; transport/protocol verified. Evidence:
`raw/acp-gate/{rust,go}.ndjson`.

## Notable macOS workarounds

- Accessory apps cannot be activated by synthetic events on macOS 26;
  `MASCOT_REGULAR_APP=1` proves the activation path; real users activate
  via genuine clicks/hotkey presses.
- Input-method sources cannot be selected programmatically
  (TISSelectInputSource paramErr, input-context setter ignored); the
  composition pipeline is verified through the same NSTextInputClient
  calls the IME makes.
- Builds pin `SDKROOT=…/MacOSX26.sdk` — SDK 27 `.tbd` files break the
  linker on this machine.
- TIS functions resolved via `dlsym` (Rust) — HIToolbox symbols are not
  exported by the `Carbon` umbrella at link time; `TISCopyInputSourceWithID`
  no longer exists on macOS 26.

Raw data: `raw/*-scenarios`, `raw/*-interactive`, `raw/*-measurements`,
`raw/acp-gate`. Screenshots/recordings: `visual/`.
