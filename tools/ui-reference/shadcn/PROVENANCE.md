# shadcn/ui reference captures — provenance

Captured: 2026-09-29 07:39 UTC
Browser: chrome.exe 154.0.8037.58 (headless, CDP, DPR 2)
ui.shadcn.com Next.js build id: unavailable (/_next/static/immutable/ is a cache path); shadcn-ui/ui main commit: `db2db460a26fa84fb65c8d903b213925fbdee9ed`

License: shadcn/ui is MIT-licensed; these PNGs are developer-only
reference evidence — never shipped or read at runtime.

| file | component | example | variant/size | state | theme | used by gallery |
|---|---|---|---|---|---|---|
| button-default-light.png | Button | button-default | default/default | default | light | component-gallery-shadcn-reference*.png |
| button-default-dark.png | Button | button-default | default/default | default | dark | component-gallery-shadcn-reference*.png |
| button-hover-light.png | Button | button-default | default/default | hover | light | component-gallery-shadcn-reference*.png |
| button-hover-dark.png | Button | button-default | default/default | hover | dark | component-gallery-shadcn-reference*.png |
| button-focus-visible-light.png | Button | button-default | default/default | focus-visible | light | component-gallery-shadcn-reference*.png |
| button-focus-visible-dark.png | Button | button-default | default/default | focus-visible | dark | component-gallery-shadcn-reference*.png |
| button-disabled-light.png | Button | button-default | default/default | disabled | light | component-gallery-shadcn-reference*.png |
| button-disabled-dark.png | Button | button-default | default/default | disabled | dark | component-gallery-shadcn-reference*.png |
| button-secondary-light.png | Button | button-secondary | secondary/default | default | light | component-gallery-shadcn-reference*.png |
| button-secondary-dark.png | Button | button-secondary | secondary/default | default | dark | component-gallery-shadcn-reference*.png |
| button-secondary-hover-light.png | Button | button-secondary | secondary/default | hover | light | component-gallery-shadcn-reference*.png |
| button-secondary-hover-dark.png | Button | button-secondary | secondary/default | hover | dark | component-gallery-shadcn-reference*.png |
| button-ghost-light.png | Button | button-ghost | ghost/default | default | light | component-gallery-shadcn-reference*.png |
| button-ghost-dark.png | Button | button-ghost | ghost/default | default | dark | component-gallery-shadcn-reference*.png |
| button-ghost-hover-light.png | Button | button-ghost | ghost/default | hover | light | component-gallery-shadcn-reference*.png |
| button-ghost-hover-dark.png | Button | button-ghost | ghost/default | hover | dark | component-gallery-shadcn-reference*.png |
| button-icon-light.png | IconButton | button-icon | outline/icon | default | light | component-gallery-shadcn-reference*.png |
| button-icon-dark.png | IconButton | button-icon | outline/icon | default | dark | component-gallery-shadcn-reference*.png |
| button-icon-hover-light.png | IconButton | button-icon | outline/icon | hover | light | component-gallery-shadcn-reference*.png |
| button-icon-hover-dark.png | IconButton | button-icon | outline/icon | hover | dark | component-gallery-shadcn-reference*.png |
| button-icon-focus-visible-light.png | IconButton | button-icon | outline/icon | focus-visible | light | component-gallery-shadcn-reference*.png |
| button-icon-focus-visible-dark.png | IconButton | button-icon | outline/icon | focus-visible | dark | component-gallery-shadcn-reference*.png |
| badge-default-light.png | Badge | badge-demo | default/ | default | light | component-gallery-shadcn-reference*.png |
| badge-default-dark.png | Badge | badge-demo | default/ | default | dark | component-gallery-shadcn-reference*.png |
| badge-secondary-light.png | Badge | badge-secondary | secondary/ | default | light | component-gallery-shadcn-reference*.png |
| badge-secondary-dark.png | Badge | badge-secondary | secondary/ | default | dark | component-gallery-shadcn-reference*.png |
| badge-outline-light.png | Badge | badge-outline | outline/ | default | light | component-gallery-shadcn-reference*.png |
| badge-outline-dark.png | Badge | badge-outline | outline/ | default | dark | component-gallery-shadcn-reference*.png |
| tooltip-open-light.png | Tooltip | tooltip-demo | outline/default | open | light | component-gallery-shadcn-reference*.png |
| tooltip-open-dark.png | Tooltip | tooltip-demo | outline/default | open | dark | component-gallery-shadcn-reference*.png |
| separator-light.png | Separator | separator-demo | / | default | light | component-gallery-shadcn-reference*.png |
| separator-dark.png | Separator | separator-demo | / | default | dark | component-gallery-shadcn-reference*.png |
| textarea-default-light.png | Composer | textarea-demo | / | default | light | component-gallery-shadcn-reference*.png |
| textarea-default-dark.png | Composer | textarea-demo | / | default | dark | component-gallery-shadcn-reference*.png |
| textarea-focus-visible-light.png | Composer | textarea-demo | / | focus-visible | light | component-gallery-shadcn-reference*.png |
| textarea-focus-visible-dark.png | Composer | textarea-demo | / | focus-visible | dark | component-gallery-shadcn-reference*.png |
| textarea-disabled-light.png | Composer | textarea-disabled | / | disabled | light | component-gallery-shadcn-reference*.png |
| textarea-disabled-dark.png | Composer | textarea-disabled | / | disabled | dark | component-gallery-shadcn-reference*.png |
| card-light.png | Surface | card-demo | / | default | light | component-gallery-shadcn-reference*.png |
| card-dark.png | Surface | card-demo | / | default | dark | component-gallery-shadcn-reference*.png |
| typography-p-light.png | Typography | typography-p | / | default | light | component-gallery-shadcn-reference*.png |
| typography-p-dark.png | Typography | typography-p | / | default | dark | component-gallery-shadcn-reference*.png |
| typography-muted-light.png | Typography | typography-muted | / | default | light | component-gallery-shadcn-reference*.png |
| typography-muted-dark.png | Typography | typography-muted | / | default | dark | component-gallery-shadcn-reference*.png |
| typography-small-light.png | Typography | typography-small | / | default | light | component-gallery-shadcn-reference*.png |
| typography-small-dark.png | Typography | typography-small | / | default | dark | component-gallery-shadcn-reference*.png |

## Motion strips (`motion/`)

Captured with `capture-shadcn.ps1 -Motion` (Chrome over CDP, DPR 2). The script pauses page animations
(`Animation.setPlaybackRate 0`), triggers the state change, then seeks the started animations to
t = 0/25/50/75/100/125/150 ms and screenshots the same padded clip at each step. The tooltip close is
triggered with Escape, because Radix keeps `data-state=delayed-open` while the pointer is in the
hover grace area. Each strip must differ between t=0 and t=150, and t=75 must differ from both, or
the capture fails.

| Strip | Example | Trigger | Animation(s) | Used by |
|---|---|---|---|---|
| `button-default-{light,dark}-t*.png` | button-default | hover | background-color transition 150 ms `cubic-bezier(0.4,0,0.2,1)` | `component-gallery-motion.png` |
| `button-ghost-{light,dark}-t*.png` | button-ghost | hover | background-color transition | `component-gallery-motion.png` |
| `button-focus-{light,dark}-t*.png` | button-default | Tab (keyboard focus) | box-shadow/border transition | `component-gallery-motion.png` |
| `tooltip-open-{light,dark}-t*.png` | tooltip-demo | hover | tw-animate-css `enter` 150 ms `ease` (fade-in-0, zoom-in-95, slide-in-from-bottom-2) | `component-gallery-motion-2.png` |
| `tooltip-close-{light,dark}-t*.png` | tooltip-demo | pointer off + Escape (recorded as `hover-off`) | tw-animate-css `exit` 150 ms `ease` (fade-out-0, zoom-out-95) | `component-gallery-motion-2.png` |

Per-strip URL, browser, animation names and computed styles are in `motion-provenance.json`.
