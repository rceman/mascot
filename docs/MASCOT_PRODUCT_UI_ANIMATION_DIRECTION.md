# Mascot product UI + animation direction

Status: design direction for post-benchmark product work.

This document records the intended product character and rendering constraints so the rig/runtime work does not optimize for the temporary benchmark composer UI.

## 1. Product interaction model

The product should feel like a small living desktop object, not a conventional chat application.

Default:

    mascot only

Open by click or global shortcut:

                    mascot
                      |
             small floating bubble
          +-------------------------+
          | Ask anything...      up |
          +-------------------------+

After submit, the same lightweight surface expands for the response:

                    mascot
             +-------------------------+
             | streamed response...    |
             |                    ...  |
             +-------------------------+
             | Ask follow-up...      up |
             +-------------------------+

The mascot should visually perch/sit on the top edge of the bubble/composer.

The benchmark composer is functional evidence only. It is not the product visual design and should not be incrementally polished into the product UI.

## 2. Visual language

Target: minimalist, restrained, native-feeling, but distinctive because of the living mascot.

Principles:

- very little chrome
- no permanent large header
- no browser-like or IDE-like frame in the primary interaction
- compact bubble
- rounded surface, subtle border, restrained shadow
- native system typography: Segoe UI Variable on Windows, SF family on macOS
- actions use small icon buttons rather than large text buttons
- controls appear only when relevant
- light and dark appearance should both remain high-contrast
- status must never rely on red-vs-green color alone; pair color with icon/text/state

Possible icon vocabulary: Lucide-style icons such as send/arrow-up, stop/square, copy, paperclip, mic, plus, more, expand, history, settings.

Do not ship an SVG/XML runtime solely for icons. Prefer build-time conversion of the small chosen icon set to compact vector paths consumed directly by the native renderer.

## 3. Surface/window direction

The current benchmark uses separately positioned mascot and composer windows, which can visibly lag during window moves.

The intended product direction is one atomic visual composition where practical:

    one transparent top-level composition
      - mascot
      - bubble
      - response
      - controls

Transparent exterior pixels remain click-through.

If platform constraints ultimately require more than one native window, the user-visible composition must still move atomically enough that the mascot never visibly trails the bubble.

## 4. Mascot orientation

Default artwork faces left-to-right.

The character should face inward / toward the active UI.

Rules:

- on the left side of the current monitor, normally face right
- on the right side, mirror horizontally and face left
- if the bubble is open, prefer facing toward the bubble
- evaluate position relative to the current monitor work area, not the entire virtual desktop
- use hysteresis near the center to avoid rapid flip-flopping
- mirror hit geometry and the complete rig transform with the visual
- do not maintain duplicate left/right raster art

Whole-rig horizontal mirror should be a root transform.

## 5. Animation character

Idle does not mean permanently frozen.

The mascot can occasionally:

- blink
- double-blink
- glance left/right/up
- tilt the head
- twitch an ear
- flick the tail
- adjust posture
- adjust the laptop
- stretch

Reactive animation can later respond to:

- pointer nearby
- hover
- click
- drag
- opening/closing bubble
- notifications

Task animation can later represent:

- thinking
- reading
- typing/generating
- waiting
- success
- error

Idle actions must not run in a predictable fixed loop. Use weighted choice, randomized cooldowns, repetition guards and context filters.

Long inactivity should reduce activity frequency rather than increasing it.

## 6. Performance invariant

The mascot is intended to remain present for long periods.

Rendering must therefore be event-driven:

    static state
      -> no animation frames
      -> no redraw loop
      -> no 60 Hz timer

When an animation starts:

    request frames
      -> animate at display cadence
      -> finish
      -> return to static/no-frame state

Target behavior:

- effectively 0 FPS while static
- 60 FPS only while an animation or interaction is active
- no browser engine
- no DOM
- no JS runtime
- no permanent polling/redraw loop
- prefer native compositor transforms where that reduces CPU wakeups

Windows runtime target remains roughly consistent with the benchmark motivation: very low idle CPU and a small resident footprint.

## 7. Rendering direction

The primary shell should remain native/lightweight.

Current preferred direction:

Windows:
- Win32
- Direct2D
- DirectComposition
- DirectWrite where appropriate
- native Windows text control for editable text

macOS later:
- AppKit
- Core Animation / CoreGraphics / CoreText
- NSTextView for editable text

Do not introduce Electron, Chromium, WebView, Tauri, a game engine, or a large cross-platform UI framework just to render this small shell.

A small project-owned scene/layout layer is acceptable and preferred if it remains narrow.

Potential primitives:

- Node
- Transform
- Rect / RoundedRect
- Path
- Image
- Text
- Clip
- Shadow
- NativeTextEditor

The product does not need CSS or a general-purpose widget framework.

## 8. Native text exception

Do not custom-render the editable composer from scratch.

Use the native text stack behind our visual surface so we retain:

- IME
- dead keys
- caret
- selection
- clipboard
- accessibility
- OS keyboard conventions
- Unicode correctness

The native editor can be styled/embedded so it visually belongs to our bubble.

## 9. Rig/art direction

The mascot must be a layered skeletal asset rather than a single flattened PNG.

At minimum, independent transforms are wanted for:

- body
- head
- both ears
- both eyes / blink support
- both arms/paws
- both legs/feet
- tail
- laptop
- laptop screen

Leg architecture must anticipate walking.

The first runtime may use rigid sprite attachments, but the skeleton/data format must permit later replacement with weighted mesh attachments without replacing the animation model.

## 10. Shadows and outlines

Shadows are composition effects, not art layers.

Do not bake:

- floor/contact shadow
- bubble shadow
- generic mascot drop shadow
- hover glow

into the mascot PNG layers.

Render them at runtime from the final composition/geometry.

Likewise, complete black outer outlines should not be baked independently around every moving body part if that produces seams and double borders.

The rig/runtime spec defines the intended outline pipeline in detail.

## 11. Development sequence

Current intended order:

M1A - static visual playground
- mascot
- bubble geometry
- theme
- icons
- native composer
- edge placement
- mirror behavior

M1B - animation runtime
- layered mascot
- bones
- idle clips
- reactive clips
- runtime outline/shadow
- no-frame idle

M1C - interaction
- click/hotkey
- submit
- mocked streaming response
- cancel
- expand/collapse
- native keyboard behavior

M2 - real provider/agent daemon integration

Provider integration must not dictate the visual design during the animation-lab task.
