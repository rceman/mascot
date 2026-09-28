# Mascot Native UI Design System v0.1

**Status:** authoritative design direction for the native UI foundation task  
**Scope:** M1A native visual shell / composer playground  
**Non-scope:** provider integration, final animation integration, history/database, browser UI

## 1. Product character

Mascot is not a conventional chat window. The product should feel like a small living desktop object with a compact native interaction surface.

Default product model:

    mascot only

Open interaction:

                mascot
                  |
          +--------------------+
          | Ask anything...  ↑ |
          +--------------------+

Expanded response:

                mascot
          +------------------------+
          | response text...       |
          |                    copy|
          +------------------------+
          | Ask follow-up...     ↑ |
          +------------------------+

The mascot visually perches on / overlaps the top edge of the bubble. The bubble is subordinate to the mascot rather than a full application window with conventional chrome.

## 2. Visual language

### Shadcn-first rule

For M1A, **shadcn is the primary visual reference system**.

When choosing spacing, radii, border treatment, neutral surfaces, control density, hover/focus treatment, icon-button proportions and overall component composition, start from the shadcn visual language before inventing a Mascot-specific alternative.

This does NOT mean importing shadcn, React, Tailwind or CSS. It means reproducing the useful visual grammar natively in Rust/Win32.

Deviate from the shadcn-first baseline only when there is a concrete product reason, such as:

- mascot overlap/perching;
- transparent desktop composition;
- native editable-text constraints;
- DPI/native-window behavior;
- a Mascot-specific interaction that shadcn does not model.

Do not redesign standard controls merely to make them "more custom".

Direction:

- strongly monochrome;
- shadcn-first restraint, spacing, borders, radii and control density;
- Devin-like black/white/neutral product feel;
- native desktop execution, not a browser imitation;
- minimal chrome;
- compact geometry;
- thin borders;
- restrained shadows;
- strong luminance hierarchy;
- icons instead of labeled toolbar buttons where meaning is obvious;
- no decorative gradients;
- no glassmorphism dependency;
- no large permanent header/sidebar;
- no visual clutter.

This is inspiration, not a pixel copy of any website/library. Do not reproduce external branding, logos or exact proprietary layouts.

The mascot provides personality. The surrounding UI should remain quiet.

## 3. Color system

Use neutral luminance first.

Light mode starting intent:

    app exterior: transparent
    surface:       near-white / white
    elevated:      white
    primary text:  near-black
    muted text:    medium neutral
    border:        light neutral
    hover:         subtle neutral fill
    pressed:       stronger neutral fill
    focus:         high-contrast neutral ring

Dark mode starting intent:

    app exterior: transparent
    surface:       near-black
    elevated:      slightly lighter neutral
    primary text:  near-white
    muted text:    medium neutral
    border:        dark neutral with visible luminance separation
    hover:         subtle lighter neutral
    pressed:       stronger lighter neutral
    focus:         high-contrast neutral ring

Do not rely on red-vs-green alone for any state. Status requires an icon, text/shape, or another redundant cue.

An accent color is optional and should be rare. The default product identity is monochrome.

## 4. Typography

Use native system typography.

Windows:

- Segoe UI Variable where available;
- normal system fallback otherwise.

macOS later:

- SF system family.

Avoid bundled custom fonts for v0.1.

Text hierarchy should come from size, weight, luminance and spacing rather than many typefaces.

Starting hierarchy:

- composer text: normal body size;
- response: normal body size with comfortable line height;
- muted metadata/status: smaller/muted;
- no large display headings in the primary bubble.

## 5. Geometry and spacing

Use a small token set rather than arbitrary values everywhere.

Suggested starting tokens in device-independent pixels:

    spacing: 4, 8, 12, 16, 20
    icon: 16
    compact icon button: 28-32
    border: 1
    small radius: 8
    medium radius: 12
    bubble radius: 14-18

These are starting points, not immutable constants. SWE-2 should tune them using rendered evidence.

Composer target:

- compact single-line state around 48-56 DIP tall;
- multiline growth is bounded;
- width should feel useful without becoming a chat panel;
- starting useful range roughly 320-440 DIP;
- maximum width should remain bounded.

Response surface may expand vertically but should retain the same visual family.

## 6. Component vocabulary

Build only what the product currently needs.

Initial primitives:

- Surface
- RoundedRect
- Border
- Separator
- Text
- Icon
- Image
- Shadow
- Clip
- Row
- Column
- Stack

Initial controls:

- IconButton
- Tooltip
- Composer
- ResponseBubble

Do not build a general-purpose widget framework, CSS system, flexbox clone or component marketplace.

Extract shared primitives only when real repeated use exists.

## 7. Icons

**Lucide is the canonical icon source for M1A.**

Use the actual upstream Lucide icon geometry for required icons rather than drawing approximate "Lucide-style" replacements.

If Lucide does not contain a genuinely required icon, document the exception before introducing another source.

Preferred initial vocabulary:

- ArrowUp / Send
- Square / Stop
- X / Close
- Copy
- Paperclip
- Plus
- Mic
- MoreHorizontal
- Maximize2 / Expand
- History
- Settings

Use Lucide geometry as the default iconography. Verify the upstream Lucide license before importing assets and preserve required attribution/license/provenance files. Do not silently mix several icon families in M1A.

### Runtime rule

Do NOT ship:

- an SVG/XML parser;
- an icon font;
- a browser rendering engine;
- a JS icon package.

Preferred pipeline:

    upstream Lucide SVG/path data
        -> build/dev conversion
        -> compact native path data
        -> Rust Icon enum
        -> Direct2D geometry on Windows

Example API intent:

    ui.icon(Icon::ArrowUp, 16.0)

The icon renderer uses the current foreground color.

Keep source icon assets or provenance in a clear dev-only location if needed.

## 8. Native editable text

The composer MUST use a native editable text control/stack.

Windows preferred:

- RichEdit or another native Windows editor integrated into our visual surface.

Do not custom-build text editing.

Required behavior:

- caret;
- selection;
- copy/paste;
- multiline;
- dead keys;
- IME;
- Unicode;
- keyboard navigation;
- submit shortcut;
- focus;
- accessibility-compatible native behavior.

The surrounding bubble, icons and decoration remain custom native-rendered.

## 9. Rendering architecture

Windows v0.1 preferred stack:

- Win32;
- Direct2D;
- DirectWrite for non-editable custom text where appropriate;
- DirectComposition where it materially improves atomic composition/movement;
- native RichEdit/editor for editable input.

Do NOT introduce:

- Electron;
- Chromium;
- WebView;
- Tauri frontend;
- Node.js;
- egui;
- iced;
- Slint;
- Qt;
- Skia solely for this UI;
- a game engine;
- wgpu.

### wgpu policy

wgpu is not needed for M1A.

Do not add it speculatively.

The UI/render boundary should remain clean enough that a specialized GPU renderer could be introduced later if weighted mesh deformation, shaders or substantial GPU effects justify it. Do not create abstractions solely for a hypothetical wgpu backend.

## 10. Window/composition model

Preferred product direction is one user-visible atomic composition where practical:

    transparent top-level composition
      - mascot
      - bubble
      - response
      - controls
      - embedded native text editor

Transparent exterior pixels should remain click-through where appropriate.

For the UI lab, correctness and visual evidence matter more than final window architecture, but do not build a design that inherently requires visible lagging separate windows.

## 11. Mascot use in this task

This is a UI task, not a rig task.

Use the existing approved mascot/rest rendering as a visual anchor.

Do not redesign mascot art.

Do not modify rig decomposition, animation clips, pivots or hidden geometry unless the Planner explicitly moves that scope into this task.

Static whole-mascot mirroring is allowed for composition tests.

## 12. Required UI states

At minimum the UI lab must present:

1. mascot-only reference;
2. empty composer;
3. focused composer;
4. composer with text;
5. multiline composer;
6. disabled/empty-send state if relevant;
7. submitting/stop state;
8. expanded response;
9. response + follow-up composer;
10. hover/focus states for icon controls;
11. light appearance;
12. dark appearance;
13. left-side composition;
14. right-side mirrored composition.

The states may be developer-controlled in the lab. No real provider is required.

## 13. Interaction details

Keep controls conditional.

Examples:

- Send appears/enables when input is actionable.
- Stop replaces Send only while a simulated request is active.
- Copy appears for response content when useful.
- Secondary controls may appear on hover/focus.
- Tooltips explain non-obvious icons.
- Escape closes/hides the bubble in the lab where appropriate.
- Enter/Shift+Enter behavior must be explicit and native-feeling.

Do not fill the bubble with controls merely because the icons exist.

## 14. DPI and display behavior

Use device-independent layout semantics.

Validate at minimum:

- 100%;
- 125%;
- 150%;
- 200%.

Text, icons, border thickness, hit targets and mascot/bubble alignment must remain visually coherent.

Avoid integer-pixel assumptions that break mixed scaling.

## 15. Accessibility and input

At minimum:

- visible keyboard focus;
- adequate contrast;
- status not encoded by color alone;
- native text semantics;
- reasonable minimum hit targets;
- keyboard navigation for actionable controls;
- meaningful tooltip/accessibility names for icons.

Do not add a parallel custom accessibility framework in v0.1.

## 16. Performance invariant

Static UI must be event-driven.

Required:

    static -> no continuous redraw loop

Do not use a permanent 60 Hz timer merely to keep the UI alive.

Redraw only on:

- window invalidation;
- input;
- state change;
- actual animation;
- compositor-required presentation.

Report:

- idle CPU;
- working set/private working set where available;
- thread count;
- handles/GDI/USER where useful;
- open/close latency;
- first-show and warm-show behavior;
- frame activity while static.

The UI lab should not materially regress the established Rust foundation without explanation.

## 17. Design evidence

The task must commit deterministic visual evidence.

At minimum:

    benchmark/results/windows/native-ui-v0.1/
      README.md
      contact-sheet-light.png
      contact-sheet-dark.png
      dpi-contact-sheet.png
      states/
        ...
      perf.json
      visual-review.md

Useful optional evidence:

- short interaction MP4;
- crop sheet for icons/text alignment;
- comparison variants used to choose spacing/radius.

Do not commit hundreds of redundant exploratory screenshots. Keep final evidence compact and reproducible.

## 18. Design acceptance

The v0.1 UI is accepted when:

- it reads visually as a compact native black/white/neutral shell;
- mascot remains the primary visual identity;
- bubble geometry is balanced and compact;
- typography and icons look intentionally aligned;
- light and dark modes are coherent;
- native text input works correctly;
- controls are not overexposed;
- no browser-style chrome appears;
- static state has no continuous redraw;
- DPI variants remain coherent;
- the implementation remains small enough to understand and iterate on.
