# Shared Acceptance Matrix v0.1

This matrix defines the minimum correctness contract for both Rust and Zig candidates.

A candidate's performance numbers are eligible for comparison only if every required case is PASS.

Allowed result values:

- PASS
- FAIL
- UNTESTED

UNTESTED does not count as passing.

Platform-native behavior may differ only where explicitly permitted below.

## A. Mascot/window behavior

| ID | Required | Setup / action | Expected result |
|---|---|---|---|
| W1 | Yes | Launch on a 1x display | Mascot appears borderless with per-pixel transparency and no visible rectangular background |
| W2 | Yes | Launch on a 2x display | Mascot remains sharp and sized by the shared logical dimensions |
| W3 | Yes | Press/drag inside the shared mascot hit region | Mascot moves continuously and releases cleanly |
| W4 | Yes | Click outside the shared mascot hit region over a separate underlying app | Click reaches the underlying app; the transparent exterior does not intercept it |
| W5 | Yes | Open composer using the global hotkey while another app is active | Composer becomes visible and ready for input |
| W6 | Yes | Hide/close composer | Mascot remains available; application stays running |
| W7 | Yes | Move between monitors/scales where available | No crash, stuck input region, or gross scale error |
| W8 | Yes | Leave mascot stationary and unfocused | No intentional continuous redraw loop |

### Shared mascot hit region

For the benchmark asset, the hit region is the non-transparent mascot silhouette expanded by at most 2 logical pixels for usability.

A candidate may use an equivalent precomputed alpha mask or geometry. Treating the full rectangular image bounds as draggable/click-blocking is a failure.

## B. Composer/text behavior

Use the platform's normal editing conventions where they do not contradict the required observable outcomes.

| ID | Required | Action | Expected result |
|---|---|---|---|
| T1 | Yes | Type ASCII and Latvian text | Exact text appears |
| T2 | Yes | Type Cyrillic text | Exact text appears |
| T3 | Yes | Insert combining-character sample | Content survives insertion, selection, copy/paste, and submit without corruption |
| T4 | Yes | Insert multi-codepoint emoji samples | Content survives editing and copy/paste without corruption |
| T5 | Yes | Insert mixed LTR/RTL sample | Content is displayed and remains editable without data corruption |
| T6 | Yes | Select a range using keyboard and mouse | Selection is visible and copy returns the selected text |
| T7 | Yes | Copy/paste multiline text | Round trip preserves content and line breaks |
| T8 | Yes | Use platform dead-key composition | Composed character is committed correctly |
| T9 | Yes | Use one named CJK IME | Preedit/composition is visible; commit works; cancel works |
| T10 | Yes | Move caret and delete around representative Unicode samples | Behavior follows the platform/toolkit's documented editing policy and never corrupts text state |
| T11 | Yes | Press submit while no IME composition is active | Prompt submits once |
| T12 | Yes | Press submit/hide while IME composition is active | Behavior is deterministic, documented, and does not silently lose or duplicate composed text |
| T13 | Yes | Reopen composer after hiding | Input focus/caret state is valid and typing works immediately |

For the initial Windows benchmark, the named IME is Microsoft Japanese IME on a documented Windows build. A different built-in CJK IME may be substituted only if both candidates use the same one and the substitution is recorded.

## C. Stream/provider behavior

The canonical wire behavior lives in docs/MOCK_PROVIDER_CONTRACT.md.

| ID | Required | Action | Expected result |
|---|---|---|---|
| P1 | Yes | Submit one request | All response chunks appear in order with no loss |
| P2 | Yes | Provider fragments/coalesces writes | Client reconstructs complete frames correctly |
| P3 | Yes | UTF-8 sequence crosses a pipe-read boundary | Decoded text remains correct |
| P4 | Yes | Provider sends a client-request frame | Client returns the required deterministic response |
| P5 | Yes | Cancel at the fixed cancellation point | Cooperative cancel completes; UI remains responsive |
| P6 | Yes | Provider writes deterministic stderr load | stderr is drained; no deadlock |
| P7 | Yes | Provider exits unexpectedly | Exit is detected and surfaced; no orphan remains |
| P8 | Yes | Provider sends the maximum valid frame | Frame is accepted |
| P9 | Yes | Provider sends an over-limit frame | Frame is rejected according to contract without unbounded allocation |
| P10 | Yes | Hide composer during an active response | Response continues; reopening shows the current result |
| P11 | Yes | Explicitly cancel while composer is hidden | Cancellation semantics are unchanged |
| P12 | Yes | Application exits | Child is cleanly terminated/reaped; no orphan remains |

## D. Correctness report

Each implementation must produce a table containing:

- case ID
- PASS / FAIL / UNTESTED
- OS/build
- input method where relevant
- short evidence/notes
- explicitly permitted native variation, if any

Performance results from a candidate with a required FAIL or UNTESTED case are retained as diagnostic data but are not eligible for the final language comparison.
