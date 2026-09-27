# Shared Acceptance Matrix v0.2

This matrix defines the minimum correctness contract for the Rust, Zig, and Go candidates.

A candidate's performance numbers are eligible for comparison only if every required case is PASS.

Allowed result values:

- PASS
- FAIL
- UNTESTED

UNTESTED does not count as passing.

Platform-native behavior may differ only where explicitly permitted by the shared fixtures before candidate implementation starts.

Normative text/action oracle:

- docs/TEXT_FIXTURES.md

Normative provider/lifecycle oracle:

- docs/MOCK_PROVIDER_CONTRACT.md

## A. Mascot/window behavior

| ID | Required | Setup / action | Expected result |
|---|---|---|---|
| W1 | Yes | Launch on a 1x display | Mascot appears borderless with per-pixel transparency and no visible rectangular background |
| W2 | Yes | Launch on a 2x display | Mascot remains sharp and sized by the shared logical dimensions |
| W3 | Yes | Press/drag inside the shared mascot hit region | Mascot moves continuously and releases cleanly |
| W4 | Yes | Click outside the shared mascot hit region over a separate underlying app | Click reaches the underlying app; the transparent exterior does not intercept it |
| W5 | Yes | Open composer using the global hotkey while another app is active | Composer becomes visible and ready for input |
| W6 | Yes | Hide/close composer | Mascot remains available; application stays running |
| W7 | Yes | Move mascot between two Windows display targets with different scale factors | No crash, stuck hit region, incorrect logical size, or gross scale error |
| W8 | Yes | Leave mascot stationary and unfocused | No intentional continuous redraw loop |
| W9 | Yes | Place an ordinary application window beneath the mascot, then activate/use that application | Mascot remains above ordinary windows according to the benchmark always-on-top policy without stealing focus merely by being visible |

### Required Stage A display laboratory for W7

The final Windows correctness/benchmark run must provide two Windows display targets recognized by the OS with different effective scale factors:

- one at 100%
- one at either 150% or 200%

Physical monitors are preferred. A reproducible virtual-display setup is acceptable if it exercises the same Windows DPI/window transition path and is used for all candidates.

If the laboratory cannot provide this setup, W7 is UNTESTED and the candidate is not yet eligible for final comparison. It must not be silently marked PASS.

### Shared mascot hit region

For the benchmark asset, the hit region is the non-transparent mascot silhouette expanded by at most 2 logical pixels for usability.

A candidate may use an equivalent precomputed alpha mask or geometry. Treating the full rectangular image bounds as draggable/click-blocking is a failure.

## B. Composer/text behavior

Data preservation alone is not sufficient for text correctness.

The shared text fixture defines:

- initial text
- caret/selection state
- action sequence
- resulting logical text
- permitted caret/selection outcomes
- required visual behavior
- explicitly approved platform-native variations

A candidate or toolkit documenting a limitation does not make that limitation an accepted variation.

Pixel-identical rasterization is not required.

| ID | Required | Action | Expected result |
|---|---|---|---|
| T1 | Yes | Type ASCII and Latvian text | Exact text appears and is visually readable |
| T2 | Yes | Type Cyrillic text | Exact text appears and is visually readable |
| T3 | Yes | Execute combining-sequence fixture F1 | Matches docs/TEXT_FIXTURES.md, including visual placement and grapheme-safe selection/copy outcome |
| T4 | Yes | Execute emoji-sequence fixture F2 | Matches docs/TEXT_FIXTURES.md, including joined presentation and permitted caret/selection outcome |
| T5 | Yes | Execute mixed-direction/Arabic fixture F3 | Correct bidi ordering and Arabic contextual shaping; logical copy result remains correct |
| T6 | Yes | Select a range using keyboard and mouse | Selection is visible and copy returns the selected text |
| T7 | Yes | Execute multiline clipboard fixture F10 | Exact text and line breaks round-trip correctly |
| T8 | Yes | Execute dead-key fixture F5 | Shared expected composed output is produced exactly once |
| T9 | Yes | Execute Microsoft Japanese IME fixture F6 | Preedit/composition, commit, and cancel match the shared fixture |
| T10 | Yes | Navigate/select representative combining and emoji fixtures | Matches the shared action fixture's expected text and permitted caret/selection outcomes; representative grapheme boundaries are respected except for pre-approved operation-specific native variations |
| T11 | Yes | Submit while no IME composition is active | Prompt submits exactly once |
| T12 | Yes | Execute active-composition submit/hide fixtures F7 and F8 | Matches the shared policy, committed text state, and expected provider-request counts exactly |
| T13 | Yes | Reopen composer after hiding | Input focus/caret state is valid and typing works immediately |
| T14 | Yes | Render response-view fixture F9 | Combining marks, emoji sequences, mixed-direction ordering, Arabic shaping, Latvian and Cyrillic are visually correct in the response path |

For Stage A, the named CJK IME is Microsoft Japanese IME on the documented Windows benchmark build.

A different built-in CJK IME may be substituted only by changing and re-freezing the shared fixture before any candidate implementation starts.

## C. Stream/provider behavior

The canonical wire behavior lives in docs/MOCK_PROVIDER_CONTRACT.md.

| ID | Required | Action | Expected result |
|---|---|---|---|
| P1 | Yes | Submit one request | All response chunks appear in order with no loss |
| P2 | Yes | Provider fragments/coalesces writes | Client reconstructs complete frames correctly |
| P3 | Yes | Execute direct decoder-fragment test plus physical-write fragmentation case | UTF-8/frame decoder handles the defined boundaries correctly; provider write splitting alone is not sufficient evidence |
| P4 | Yes | Provider sends a client-request frame | Client returns the required deterministic response |
| P5 | Yes | Execute canonical cancellation barrier | Client accepts chunks 0..49, sends cancel, receives exactly one cancelled terminal frame with last_seq 49, receives neither chunk 50 nor complete, and the same child remains usable |
| P6 | Yes | Provider writes deterministic stderr load | stderr is drained; no deadlock |
| P7 | Yes | Provider exits unexpectedly | Exit is detected and surfaced; no orphan remains |
| P8 | Yes | Provider sends the maximum valid frame | Frame is accepted |
| P9 | Yes | Provider sends an over-limit frame | Current request fails; provider session is invalidated and fixture is terminated/reaped within the shared timeout; next explicit request starts a fresh session |
| P10 | Yes | Hide composer during an active response | Response continues; reopening shows the current result |
| P11 | Yes | Explicitly cancel while composer is hidden | Cancellation semantics are unchanged |
| P12 | Yes | Application exits | Child is cleanly terminated/reaped within the shared timeout; no orphan remains |

## D. Correctness report

Each implementation must produce a table containing:

- case ID
- PASS / FAIL / UNTESTED
- OS/build
- input method where relevant
- fixture version
- short evidence/notes
- explicitly permitted native variation, if any

Performance results from a candidate with a required FAIL or UNTESTED case are retained as diagnostic data but are not eligible for the final language comparison.
