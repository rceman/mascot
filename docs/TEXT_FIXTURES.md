# Shared Text and Visual Fixtures v0.1

This document is a normative correctness oracle shared by the Rust, Zig, and Go candidates.

The purpose is not pixel-identical rendering. The purpose is to prevent a candidate from winning by preserving UTF-8 bytes while presenting or editing representative text incorrectly.

All cases apply to the composer unless marked response-only. Visual-only cases also apply to the plain-text response view.

## General rules

- Candidate-selected toolkit limitations are not automatically accepted behavior.
- Permitted native variations must be listed here before candidate implementation starts.
- Pixel-identical glyph rasterization, font hinting, antialiasing, and caret appearance are not required.
- The same benchmark font family, size, and fallback policy must be used where the platform makes that practical.
- The resulting Unicode scalar sequence must match the expected text exactly unless the case explicitly permits a native variation.
- Visual inspection must confirm that no required text is missing, reordered incorrectly, clipped, or replaced by tofu when the platform has a suitable system font.
- Composer editing behavior and response rendering are checked separately.

## F1 — Combining sequence navigation

Initial text:

    AéB

The visible é is U+0065 LATIN SMALL LETTER E followed by U+0301 COMBINING ACUTE ACCENT.

Initial caret:

    after B

Actions:

1. Move caret left once.
2. Move caret left once.
3. Select the visible é using keyboard selection.
4. Copy.
5. Paste into an empty temporary composer.

Expected:

- The accent remains visually attached to the e.
- Selection must not leave the combining mark visually orphaned.
- The copied/pasted result is exactly U+0065 U+0301.
- Caret/navigation must respect the shared grapheme boundary around é.

Permitted variation:

- Native caret may expose an intermediate code-point position only if the selected/copy result and visual behavior remain grapheme-safe and the variation is pre-approved for all candidates on that platform.

Deletion refinement:

1. Recreate the initial text.
2. Select the complete visible é sequence using the frozen keyboard procedure.
3. Delete the selection.

Expected result:

    AB

There must be no orphaned combining mark, replacement character, or hidden residual code point from the selected sequence.

## F2 — Emoji sequence navigation

Initial text:

    A👨‍💻B

The emoji is the sequence MAN + ZWJ + LAPTOP.

Actions:

1. Place caret after B.
2. Navigate left across B and then across the emoji.
3. Select the emoji using keyboard selection.
4. Copy/paste it.

Expected:

- The emoji is presented as the intended joined sequence when supported by the system font stack.
- Selection/copy does not split the ZWJ sequence into visibly broken pieces.
- Pasted text is byte-for-byte equivalent UTF-8 for the same Unicode sequence.
- No replacement characters appear.

Permitted variation:

- Platform-native caret stepping may differ only if the shared visual and copy/selection outcomes remain correct and the variation is approved before candidate implementation.

Deletion refinement:

1. Recreate the initial text.
2. Select the complete 👨‍💻 sequence using the frozen keyboard procedure.
3. Delete the selection.

Expected result:

    AB

There must be no orphaned ZWJ, broken sequence fragment, replacement character, or hidden residual code point from the selected emoji sequence.

## F3 — Mixed-direction and Arabic shaping

Text:

    English العربية English

Apply to:

- composer visual path
- response visual path

Expected:

- Arabic letters use contextual shaping.
- The Arabic run has correct RTL ordering.
- The surrounding English runs remain readable in their expected LTR order.
- No candidate may treat logical storage order as the required visual order.
- Copying the entire line from the composer reproduces the original logical Unicode text.

Pixel-identical line breaking is not required.

## F4 — Latvian, Cyrillic, and emoji response rendering

Response fixture must include all of:

    Ārā līst, bet mēs turpinām darbu.
    Проверка отображения текста.
    👨‍💻 🧑🏽‍🚀 ❤️‍🔥

Expected in both composer and response view where applicable:

- no dropped code points
- no replacement characters
- expected combining/emoji sequences remain visually intact
- system fallback fonts may be used

## F5 — Dead-key composition

Windows Stage A input:

- use the same configured keyboard layout for all candidates
- enter one Latin character through a dead-key sequence

Expected:

- precomposition does not submit a request
- the composed character appears once
- copy/paste preserves the composed result

The exact keyboard layout and key sequence are frozen in the fixture manifest before candidate implementation.

## F6 — Microsoft Japanese IME commit and cancel

Windows Stage A named IME:

    Microsoft Japanese IME

Cases:

### F6a commit

1. Start with an empty composer.
2. Enter the fixture-defined romaji sequence.
3. Observe IME preedit/composition.
4. Commit the composition.

Expected:

- preedit is visible
- committed text equals the fixture manifest value
- exactly one committed text sequence appears
- no provider request is sent during composition

### F6b cancel

1. Start composition with the same fixture.
2. Cancel composition using the fixture-defined native action.

Expected:

- no duplicate/lost committed text
- composer returns to the fixture-defined prior state
- no provider request is sent

## F7 — Submit while IME composition is active

Shared policy:

- Submit is disabled as a provider action while an IME composition is active.
- The first submit key action follows the platform-native composition behavior and must not create a provider request.
- Only after composition has reached a committed/non-composing state may a subsequent submit create exactly one provider request.

Expected provider-request count before composition ends:

    0

Expected provider-request count after the explicit post-commit submit:

    1

## F8 — Hide while IME composition is active

Shared policy:

- Hiding the composer must not submit a provider request.
- Active composition is cancelled before the composer is hidden.
- Reopening the composer restores the last committed text state from before the active composition began.

Expected provider-request count:

    0

This policy is intentionally fixed for benchmark equality even if a toolkit has a different default.

## F9 — Response-view visual parity

The response view is not editable, but it must pass the visual portions of:

- F1 combining-mark presentation
- F2 emoji-sequence presentation
- F3 mixed-direction ordering and Arabic contextual shaping
- F4 Latvian/Cyrillic/emoji rendering

A candidate cannot use a correct native composer to pass text tests while using an incorrect cheaper response renderer.

## F10 — Multiline clipboard round trip

Initial text:

    first line
    Ārā līst
    Проверка
    👨‍💻

Actions:

1. Select all.
2. Copy.
3. Clear composer.
4. Paste.

Expected:

- exact logical text and line-break structure are preserved
- visual rendering remains correct for each line

## Fixture freeze fields

Before candidate implementation begins, the common fixture manifest must freeze:

- benchmark font family/size or platform-default rule
- Windows keyboard layout for F5
- exact dead-key sequence and expected output
- F6 romaji sequence and expected committed Japanese text
- native cancel action for F6b
- exact logical strings for F1-F4 and F10
- any pre-approved platform-native caret/selection variation
- visual-check procedure and evidence format
- concrete keyboard selection procedure and expected caret/selection endpoints for F1
- concrete keyboard selection procedure and expected caret/selection endpoints for F2

Any change after freeze increments the fixture version and invalidates affected measurements/correctness evidence.
