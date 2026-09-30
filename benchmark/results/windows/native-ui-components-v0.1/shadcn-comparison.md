# shadcn computed styles vs native tokens

Ref values are getComputedStyle on the captured element (raw CSS + sRGB hex via canvas normalisation).
Native values are measured from the rect/palette that drew the gallery cell.

## Theme variables — shadcn root vars vs native palette

| shadcn var | light ref | native light | dark ref | native dark |
|---|---|---|---|---|
| `--background` | `#FFFFFF` | `surface` `#FFFFFF` | `#0A0A0A` | `surface` `#171717` |
| `--foreground` | `#000000` | `foreground` `#0A0A0A` | `#FAFAFA` | `foreground` `#FAFAFA` |
| `--card` | `#FFFFFF` | `surface` `#FFFFFF` | `#171717` | `surface` `#171717` |
| `--card-foreground` | `#000000` | `foreground` `#0A0A0A` | `#FAFAFA` | `foreground` `#FAFAFA` |
| `--popover` | `#FFFFFF` | `surface` `#FFFFFF` | `#171717` | `surface` `#171717` |
| `--primary` | `#000000` | `primary` `#171717` | `#E5E5E5` | `primary` `#E5E5E5` |
| `--primary-foreground` | `#FAFAFA` | `primary_fg` `#FAFAFA` | `#171717` | `primary_fg` `#171717` |
| `--secondary` | `#F5F5F5` | `secondary` `#F5F5F5` | `#262626` | `secondary` `#262626` |
| `--secondary-foreground` | `#171717` | `secondary_fg` `#171717` | `#FAFAFA` | `secondary_fg` `#FAFAFA` |
| `--muted` | `#F5F5F5` | `muted` `#F5F5F5` | `#262626` | `muted` `#262626` |
| `--muted-foreground` | `#737373` | `muted_fg` `#737373` | `#A1A1A1` | `muted_fg` `#A1A1A1` |
| `--accent` | `#F5F5F5` | `hover` `#F5F5F5` | `#404040` | `hover` `#262626` |
| `--accent-foreground` | `#171717` | `foreground` `#0A0A0A` | `#FAFAFA` | `foreground` `#FAFAFA` |
| `--border` | `#E5E5E5` | `border` `#E5E5E5` | `#FFFFFF1A` | `border` `#FFFFFF1A` |
| `--input` | `#E5E5E5` | `border` `#E5E5E5` | `#FFFFFF26` | `border` `#FFFFFF1A` |
| `--ring` | `#A1A1A1` | `ring` `#A1A1A1` | `#737373` | `ring` `#737373` |

Radius: `--radius` ref `.625rem` / `.625rem` — native uses radius tokens sm 6 / md 8 / lg 10 / xl 14 DIP.

## button-default-light.png — Button · `button-default` — default (default/default) — light

ref rect: 76 x 36 CSS px (element `[data-slot="button"]`)

| field | reference | native (measured) |
|---|---|---|
| width | `75.7188px` | `75.271484 DIP` |
| height | `36px` | `36 DIP` |
| padding | `8px 16px` | `0 16 DIP` |
| borderRadius | `8px` | `8 DIP` |
| borderWidth | `0px` | `none` |
| borderColor | `lab(90.952 0 -0.0000119209) #E5E5E5` | `—` |
| backgroundColor | `lab(0 0 0) #000000` | `#171717` |
| color | `lab(98.26 0 0) #FAFAFA` | `#FAFAFA` |
| fontFamily | `Geist, "Geist Fallback"` | `Segoe UI Variable Text` |
| fontSize | `14px` | `14 DIP` |
| fontWeight | `500` | `500` |
| lineHeight | `20px` | `—` |
| boxShadow | `none` | `none` |

## button-default-dark.png — Button · `button-default` — default (default/default) — dark

ref rect: 76 x 36 CSS px (element `[data-slot="button"]`)

| field | reference | native (measured) |
|---|---|---|
| width | `75.7188px` | `75.271484 DIP` |
| height | `36px` | `36 DIP` |
| padding | `8px 16px` | `0 16 DIP` |
| borderRadius | `8px` | `8 DIP` |
| borderWidth | `0px` | `none` |
| borderColor | `lab(100 0 0 / 0.1) #FFFFFF1A` | `—` |
| backgroundColor | `lab(90.952 0 -0.0000119209) #E5E5E5` | `#E5E5E5` |
| color | `lab(7.78201 -0.0000149012 0) #171717` | `#171717` |
| fontFamily | `Geist, "Geist Fallback"` | `Segoe UI Variable Text` |
| fontSize | `14px` | `14 DIP` |
| fontWeight | `500` | `500` |
| lineHeight | `20px` | `—` |
| boxShadow | `none` | `none` |

## button-hover-light.png — Button · `button-default` — hover (default/default) — light

ref rect: 76 x 36 CSS px (element `[data-slot="button"]`)

| field | reference | native (measured) |
|---|---|---|
| width | `75.7188px` | `75.271484 DIP` |
| height | `36px` | `36 DIP` |
| padding | `8px 16px` | `0 16 DIP` |
| borderRadius | `8px` | `8 DIP` |
| borderWidth | `0px` | `none` |
| borderColor | `lab(90.952 0 -0.0000119209) #E5E5E5` | `—` |
| backgroundColor | `oklab(0 0 0 / 0.901207) #000000E6` | `#2E2E2E` |
| color | `lab(98.26 0 0) #FAFAFA` | `#FAFAFA` |
| fontFamily | `Geist, "Geist Fallback"` | `Segoe UI Variable Text` |
| fontSize | `14px` | `14 DIP` |
| fontWeight | `500` | `500` |
| lineHeight | `20px` | `—` |
| boxShadow | `none` | `none` |

## button-hover-dark.png — Button · `button-default` — hover (default/default) — dark

ref rect: 76 x 36 CSS px (element `[data-slot="button"]`)

| field | reference | native (measured) |
|---|---|---|
| width | `75.7188px` | `75.271484 DIP` |
| height | `36px` | `36 DIP` |
| padding | `8px 16px` | `0 16 DIP` |
| borderRadius | `8px` | `8 DIP` |
| borderWidth | `0px` | `none` |
| borderColor | `lab(100 0 0 / 0.1) #FFFFFF1A` | `—` |
| backgroundColor | `oklab(0.921998 -0.00000908971 0.0000215769 / 0.9) #E5E5E5E6` | `#D0D0D0` |
| color | `lab(7.78201 -0.0000149012 0) #171717` | `#171717` |
| fontFamily | `Geist, "Geist Fallback"` | `Segoe UI Variable Text` |
| fontSize | `14px` | `14 DIP` |
| fontWeight | `500` | `500` |
| lineHeight | `20px` | `—` |
| boxShadow | `none` | `none` |

## button-focus-visible-light.png — Button · `button-default` — focus-visible (default/default) — light

ref rect: 76 x 36 CSS px (element `[data-slot="button"]`)

| field | reference | native (measured) |
|---|---|---|
| width | `75.7188px` | `75.271484 DIP` |
| height | `36px` | `36 DIP` |
| padding | `8px 16px` | `0 16 DIP` |
| borderRadius | `8px` | `8 DIP` |
| borderWidth | `0px` | `ring 1.5 DIP` |
| borderColor | `lab(90.952 0 -0.0000119209) #E5E5E5` | `—` |
| backgroundColor | `lab(0 0 0) #000000` | `#171717` |
| color | `lab(98.26 0 0) #FAFAFA` | `#FAFAFA` |
| fontFamily | `Geist, "Geist Fallback"` | `Segoe UI Variable Text` |
| fontSize | `14px` | `14 DIP` |
| fontWeight | `500` | `500` |
| lineHeight | `20px` | `—` |
| boxShadow | `none` | `none` |

## button-focus-visible-dark.png — Button · `button-default` — focus-visible (default/default) — dark

ref rect: 76 x 36 CSS px (element `[data-slot="button"]`)

| field | reference | native (measured) |
|---|---|---|
| width | `75.7188px` | `75.271484 DIP` |
| height | `36px` | `36 DIP` |
| padding | `8px 16px` | `0 16 DIP` |
| borderRadius | `8px` | `8 DIP` |
| borderWidth | `0px` | `ring 1.5 DIP` |
| borderColor | `lab(100 0 0 / 0.1) #FFFFFF1A` | `—` |
| backgroundColor | `lab(90.952 0 -0.0000119209) #E5E5E5` | `#E5E5E5` |
| color | `lab(7.78201 -0.0000149012 0) #171717` | `#171717` |
| fontFamily | `Geist, "Geist Fallback"` | `Segoe UI Variable Text` |
| fontSize | `14px` | `14 DIP` |
| fontWeight | `500` | `500` |
| lineHeight | `20px` | `—` |
| boxShadow | `none` | `none` |

## button-disabled-light.png — Button · `button-default` — disabled (default/default) — light

ref rect: 76 x 36 CSS px (element `[data-slot="button"]`)

| field | reference | native (measured) |
|---|---|---|
| width | `75.7188px` | `75.271484 DIP` |
| height | `36px` | `36 DIP` |
| padding | `8px 16px` | `0 16 DIP` |
| borderRadius | `8px` | `8 DIP` |
| borderWidth | `0px` | `none` |
| borderColor | `lab(90.952 0 -0.0000119209) #E5E5E5` | `—` |
| backgroundColor | `lab(0 0 0) #000000` | `#F5F5F5` |
| color | `lab(98.26 0 0) #FAFAFA` | `#737373` |
| fontFamily | `Geist, "Geist Fallback"` | `Segoe UI Variable Text` |
| fontSize | `14px` | `14 DIP` |
| fontWeight | `500` | `500` |
| lineHeight | `20px` | `—` |
| boxShadow | `none` | `none` |

## button-disabled-dark.png — Button · `button-default` — disabled (default/default) — dark

ref rect: 76 x 36 CSS px (element `[data-slot="button"]`)

| field | reference | native (measured) |
|---|---|---|
| width | `75.7188px` | `75.271484 DIP` |
| height | `36px` | `36 DIP` |
| padding | `8px 16px` | `0 16 DIP` |
| borderRadius | `8px` | `8 DIP` |
| borderWidth | `0px` | `none` |
| borderColor | `lab(100 0 0 / 0.1) #FFFFFF1A` | `—` |
| backgroundColor | `lab(90.952 0 -0.0000119209) #E5E5E5` | `#262626` |
| color | `lab(7.78201 -0.0000149012 0) #171717` | `#A1A1A1` |
| fontFamily | `Geist, "Geist Fallback"` | `Segoe UI Variable Text` |
| fontSize | `14px` | `14 DIP` |
| fontWeight | `500` | `500` |
| lineHeight | `20px` | `—` |
| boxShadow | `none` | `none` |

## button-secondary-light.png — Button · `button-secondary` — default (secondary/default) — light

ref rect: 102 x 36 CSS px (element `[data-slot="button"]`)

| field | reference | native (measured) |
|---|---|---|
| width | `102.344px` | `98.21289 DIP` |
| height | `36px` | `36 DIP` |
| padding | `8px 16px` | `0 16 DIP` |
| borderRadius | `8px` | `8 DIP` |
| borderWidth | `0px` | `none` |
| borderColor | `lab(90.952 0 -0.0000119209) #E5E5E5` | `—` |
| backgroundColor | `lab(96.52 -0.0000298023 0.0000119209) #F5F5F5` | `#F5F5F5` |
| color | `lab(7.78201 -0.0000149012 0) #171717` | `#171717` |
| fontFamily | `Geist, "Geist Fallback"` | `Segoe UI Variable Text` |
| fontSize | `14px` | `14 DIP` |
| fontWeight | `500` | `500` |
| lineHeight | `20px` | `—` |
| boxShadow | `none` | `none` |

## button-secondary-dark.png — Button · `button-secondary` — default (secondary/default) — dark

ref rect: 102 x 36 CSS px (element `[data-slot="button"]`)

| field | reference | native (measured) |
|---|---|---|
| width | `102.344px` | `98.21289 DIP` |
| height | `36px` | `36 DIP` |
| padding | `8px 16px` | `0 16 DIP` |
| borderRadius | `8px` | `8 DIP` |
| borderWidth | `0px` | `none` |
| borderColor | `lab(100 0 0 / 0.1) #FFFFFF1A` | `—` |
| backgroundColor | `lab(15.204 0 -0.00000596046) #262626` | `#262626` |
| color | `lab(98.26 0 0) #FAFAFA` | `#FAFAFA` |
| fontFamily | `Geist, "Geist Fallback"` | `Segoe UI Variable Text` |
| fontSize | `14px` | `14 DIP` |
| fontWeight | `500` | `500` |
| lineHeight | `20px` | `—` |
| boxShadow | `none` | `none` |

## button-secondary-hover-light.png — Button · `button-secondary` — hover (secondary/default) — light

ref rect: 102 x 36 CSS px (element `[data-slot="button"]`)

| field | reference | native (measured) |
|---|---|---|
| width | `102.344px` | `98.21289 DIP` |
| height | `36px` | `36 DIP` |
| padding | `8px 16px` | `0 16 DIP` |
| borderRadius | `8px` | `8 DIP` |
| borderWidth | `0px` | `none` |
| borderColor | `lab(90.952 0 -0.0000119209) #E5E5E5` | `—` |
| backgroundColor | `oklab(0.969998 -0.00000959635 0.0000227094 / 0.8) #F5F5F5CC` | `#F7F7F7` |
| color | `lab(7.78201 -0.0000149012 0) #171717` | `#171717` |
| fontFamily | `Geist, "Geist Fallback"` | `Segoe UI Variable Text` |
| fontSize | `14px` | `14 DIP` |
| fontWeight | `500` | `500` |
| lineHeight | `20px` | `—` |
| boxShadow | `none` | `none` |

## button-secondary-hover-dark.png — Button · `button-secondary` — hover (secondary/default) — dark

ref rect: 102 x 36 CSS px (element `[data-slot="button"]`)

| field | reference | native (measured) |
|---|---|---|
| width | `102.344px` | `98.21289 DIP` |
| height | `36px` | `36 DIP` |
| padding | `8px 16px` | `0 16 DIP` |
| borderRadius | `8px` | `8 DIP` |
| borderWidth | `0px` | `none` |
| borderColor | `lab(100 0 0 / 0.1) #FFFFFF1A` | `—` |
| backgroundColor | `oklab(0.268999 -0.00000260025 0.00000627339 / 0.802298) #262626CD` | `#232323` |
| color | `lab(98.26 0 0) #FAFAFA` | `#FAFAFA` |
| fontFamily | `Geist, "Geist Fallback"` | `Segoe UI Variable Text` |
| fontSize | `14px` | `14 DIP` |
| fontWeight | `500` | `500` |
| lineHeight | `20px` | `—` |
| boxShadow | `none` | `none` |

## button-ghost-light.png — Button · `button-ghost` — default (ghost/default) — light

ref rect: 71 x 36 CSS px (element `[data-slot="button"]`)

| field | reference | native (measured) |
|---|---|---|
| width | `71.0312px` | `69.365234 DIP` |
| height | `36px` | `36 DIP` |
| padding | `8px 16px` | `0 16 DIP` |
| borderRadius | `8px` | `8 DIP` |
| borderWidth | `0px` | `none` |
| borderColor | `lab(90.952 0 -0.0000119209) #E5E5E5` | `—` |
| backgroundColor | `rgba(0, 0, 0, 0) #00000000` | `#00000000` |
| color | `rgb(0, 0, 0) #000000` | `#0A0A0A` |
| fontFamily | `Geist, "Geist Fallback"` | `Segoe UI Variable Text` |
| fontSize | `14px` | `14 DIP` |
| fontWeight | `500` | `500` |
| lineHeight | `20px` | `—` |
| boxShadow | `none` | `none` |

## button-ghost-dark.png — Button · `button-ghost` — default (ghost/default) — dark

ref rect: 71 x 36 CSS px (element `[data-slot="button"]`)

| field | reference | native (measured) |
|---|---|---|
| width | `71.0312px` | `69.365234 DIP` |
| height | `36px` | `36 DIP` |
| padding | `8px 16px` | `0 16 DIP` |
| borderRadius | `8px` | `8 DIP` |
| borderWidth | `0px` | `none` |
| borderColor | `lab(100 0 0 / 0.1) #FFFFFF1A` | `—` |
| backgroundColor | `rgba(0, 0, 0, 0) #00000000` | `#00000000` |
| color | `rgb(255, 255, 255) #FFFFFF` | `#FAFAFA` |
| fontFamily | `Geist, "Geist Fallback"` | `Segoe UI Variable Text` |
| fontSize | `14px` | `14 DIP` |
| fontWeight | `500` | `500` |
| lineHeight | `20px` | `—` |
| boxShadow | `none` | `none` |

## button-ghost-hover-light.png — Button · `button-ghost` — hover (ghost/default) — light

ref rect: 71 x 36 CSS px (element `[data-slot="button"]`)

| field | reference | native (measured) |
|---|---|---|
| width | `71.0312px` | `69.365234 DIP` |
| height | `36px` | `36 DIP` |
| padding | `8px 16px` | `0 16 DIP` |
| borderRadius | `8px` | `8 DIP` |
| borderWidth | `0px` | `none` |
| borderColor | `lab(90.952 0 -0.0000119209) #E5E5E5` | `—` |
| backgroundColor | `oklab(0.969998 -0.00000959635 0.0000227094 / 0.944511) #F5F5F5F1` | `#F5F5F5` |
| color | `oklab(0.193624 -0.00000196337 0.00000451786) #151515` | `#0A0A0A` |
| fontFamily | `Geist, "Geist Fallback"` | `Segoe UI Variable Text` |
| fontSize | `14px` | `14 DIP` |
| fontWeight | `500` | `500` |
| lineHeight | `20px` | `—` |
| boxShadow | `none` | `none` |

## button-ghost-hover-dark.png — Button · `button-ghost` — hover (ghost/default) — dark

ref rect: 71 x 36 CSS px (element `[data-slot="button"]`)

| field | reference | native (measured) |
|---|---|---|
| width | `71.0312px` | `69.365234 DIP` |
| height | `36px` | `36 DIP` |
| padding | `8px 16px` | `0 16 DIP` |
| borderRadius | `8px` | `8 DIP` |
| borderWidth | `0px` | `none` |
| borderColor | `lab(100 0 0 / 0.1) #FFFFFF1A` | `—` |
| backgroundColor | `oklab(0.370999 -0.00000369549 0.00000870228 / 0.5) #40404080` | `#262626` |
| color | `oklab(0.984998 -0.00000956655 0.0000230074) #FAFAFA` | `#FAFAFA` |
| fontFamily | `Geist, "Geist Fallback"` | `Segoe UI Variable Text` |
| fontSize | `14px` | `14 DIP` |
| fontWeight | `500` | `500` |
| lineHeight | `20px` | `—` |
| boxShadow | `none` | `none` |

## button-icon-light.png — IconButton · `button-icon` — default (outline/icon) — light

ref rect: 36 x 36 CSS px (element `[data-slot="button"]`)

| field | reference | native (measured) |
|---|---|---|
| width | `36px` | `28 / 32 DIP` |
| height | `36px` | `28 / 32 DIP` |
| padding | `0px` | `—` |
| borderRadius | `8px` | `8 DIP` |
| borderWidth | `1px` | `none (1 DIP border only on focus-visible)` |
| borderColor | `lab(90.952 0 -0.0000119209) #E5E5E5` | `—` |
| backgroundColor | `lab(100 0 0) #FFFFFF` | `ghost #00000000 / primary #171717` |
| color | `rgb(0, 0, 0) #000000` | `ghost #0A0A0A / primary #FAFAFA` |
| fontFamily | `Geist, "Geist Fallback"` | `icon geometry (Lucide paths)` |
| fontSize | `14px` | `—` |
| fontWeight | `500` | `—` |
| lineHeight | `20px` | `—` |
| boxShadow | `rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0.05) 0px 1px 2px 0px` | `—` |
| native variant | — | `ghost 28 + primary 32 (no outline icon button)` |
| ring | — | `none` |

## button-icon-dark.png — IconButton · `button-icon` — default (outline/icon) — dark

ref rect: 36 x 36 CSS px (element `[data-slot="button"]`)

| field | reference | native (measured) |
|---|---|---|
| width | `36px` | `28 / 32 DIP` |
| height | `36px` | `28 / 32 DIP` |
| padding | `0px` | `—` |
| borderRadius | `8px` | `8 DIP` |
| borderWidth | `1px` | `none (1 DIP border only on focus-visible)` |
| borderColor | `lab(100 0 0 / 0.15) #FFFFFF26` | `—` |
| backgroundColor | `oklab(0.999998 -0.00000980496 0.0000234246 / 0.045) #FFFFFF0B` | `ghost #00000000 / primary #E5E5E5` |
| color | `rgb(255, 255, 255) #FFFFFF` | `ghost #FAFAFA / primary #171717` |
| fontFamily | `Geist, "Geist Fallback"` | `icon geometry (Lucide paths)` |
| fontSize | `14px` | `—` |
| fontWeight | `500` | `—` |
| lineHeight | `20px` | `—` |
| boxShadow | `rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0.05) 0px 1px 2px 0px` | `—` |
| native variant | — | `ghost 28 + primary 32 (no outline icon button)` |
| ring | — | `none` |

## button-icon-hover-light.png — IconButton · `button-icon` — hover (outline/icon) — light

ref rect: 36 x 36 CSS px (element `[data-slot="button"]`)

| field | reference | native (measured) |
|---|---|---|
| width | `36px` | `28 / 32 DIP` |
| height | `36px` | `28 / 32 DIP` |
| padding | `0px` | `—` |
| borderRadius | `8px` | `8 DIP` |
| borderWidth | `1px` | `none (1 DIP border only on focus-visible)` |
| borderColor | `lab(90.952 0 -0.0000119209) #E5E5E5` | `—` |
| backgroundColor | `oklab(0.970352 -0.00000959881 0.0000227178) #F5F5F5` | `ghost #F5F5F5 / primary #2E2E2E` |
| color | `oklab(0.202577 -0.00000205415 0.00000472675) #171717` | `ghost #0A0A0A / primary #FAFAFA` |
| fontFamily | `Geist, "Geist Fallback"` | `icon geometry (Lucide paths)` |
| fontSize | `14px` | `—` |
| fontWeight | `500` | `—` |
| lineHeight | `20px` | `—` |
| boxShadow | `rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0.05) 0px 1px 2px 0px` | `—` |
| native variant | — | `ghost 28 + primary 32 (no outline icon button)` |
| ring | — | `none` |

## button-icon-hover-dark.png — IconButton · `button-icon` — hover (outline/icon) — dark

ref rect: 36 x 36 CSS px (element `[data-slot="button"]`)

| field | reference | native (measured) |
|---|---|---|
| width | `36px` | `28 / 32 DIP` |
| height | `36px` | `28 / 32 DIP` |
| padding | `0px` | `—` |
| borderRadius | `8px` | `8 DIP` |
| borderWidth | `1px` | `none (1 DIP border only on focus-visible)` |
| borderColor | `lab(100 0 0 / 0.15) #FFFFFF26` | `—` |
| backgroundColor | `oklab(0.999998 -0.00000980496 0.0000234246 / 0.074663) #FFFFFF13` | `ghost #262626 / primary #D0D0D0` |
| color | `oklab(0.985166 -0.00000894722 0.0000229746) #FAFAFA` | `ghost #FAFAFA / primary #171717` |
| fontFamily | `Geist, "Geist Fallback"` | `icon geometry (Lucide paths)` |
| fontSize | `14px` | `—` |
| fontWeight | `500` | `—` |
| lineHeight | `20px` | `—` |
| boxShadow | `rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0.05) 0px 1px 2px 0px` | `—` |
| native variant | — | `ghost 28 + primary 32 (no outline icon button)` |
| ring | — | `none` |

## button-icon-focus-visible-light.png — IconButton · `button-icon` — focus-visible (outline/icon) — light

ref rect: 36 x 36 CSS px (element `[data-slot="button"]`)

| field | reference | native (measured) |
|---|---|---|
| width | `36px` | `28 / 32 DIP` |
| height | `36px` | `28 / 32 DIP` |
| padding | `0px` | `—` |
| borderRadius | `8px` | `8 DIP` |
| borderWidth | `1px` | `none (1 DIP border only on focus-visible)` |
| borderColor | `lab(90.952 0 -0.0000119209) #E5E5E5` | `—` |
| backgroundColor | `lab(100 0 0) #FFFFFF` | `ghost #00000000 / primary #171717` |
| color | `rgb(0, 0, 0) #000000` | `ghost #0A0A0A / primary #FAFAFA` |
| fontFamily | `Geist, "Geist Fallback"` | `icon geometry (Lucide paths)` |
| fontSize | `14px` | `—` |
| fontWeight | `500` | `—` |
| lineHeight | `20px` | `—` |
| boxShadow | `rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0.05) 0px 1px 2px 0px` | `—` |
| native variant | — | `ghost 28 + primary 32 (no outline icon button)` |
| ring | — | `3 DIP at ring 50 % + 1 DIP #A1A1A1` |

## button-icon-focus-visible-dark.png — IconButton · `button-icon` — focus-visible (outline/icon) — dark

ref rect: 36 x 36 CSS px (element `[data-slot="button"]`)

| field | reference | native (measured) |
|---|---|---|
| width | `36px` | `28 / 32 DIP` |
| height | `36px` | `28 / 32 DIP` |
| padding | `0px` | `—` |
| borderRadius | `8px` | `8 DIP` |
| borderWidth | `1px` | `none (1 DIP border only on focus-visible)` |
| borderColor | `lab(100 0 0 / 0.15) #FFFFFF26` | `—` |
| backgroundColor | `oklab(0.999998 -0.00000980496 0.0000234246 / 0.045) #FFFFFF0B` | `ghost #00000000 / primary #E5E5E5` |
| color | `rgb(255, 255, 255) #FFFFFF` | `ghost #FAFAFA / primary #171717` |
| fontFamily | `Geist, "Geist Fallback"` | `icon geometry (Lucide paths)` |
| fontSize | `14px` | `—` |
| fontWeight | `500` | `—` |
| lineHeight | `20px` | `—` |
| boxShadow | `rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0.05) 0px 1px 2px 0px` | `—` |
| native variant | — | `ghost 28 + primary 32 (no outline icon button)` |
| ring | — | `3 DIP at ring 50 % + 1 DIP #737373` |

## badge-default-light.png — Badge · `badge-demo` — default (default/) — light

ref rect: 54 x 22 CSS px (element `[data-slot="badge"]`)

| field | reference | native (measured) |
|---|---|---|
| width | `54.1094px` | `52.347656 DIP` |
| height | `22px` | `22 DIP` |
| padding | `2px 8px` | `2 x 8 DIP` |
| borderRadius | `3.35544e+07px` | `11 DIP (pill)` |
| borderWidth | `1px` | `1 DIP (transparent)` |
| borderColor | `rgba(0, 0, 0, 0) #00000000` | `transparent` |
| backgroundColor | `lab(0 0 0) #000000` | `#171717` |
| color | `lab(98.26 0 0) #FAFAFA` | `#FAFAFA` |
| fontFamily | `Geist, "Geist Fallback"` | `Segoe UI Variable Text` |
| fontSize | `12px` | `12 DIP` |
| fontWeight | `500` | `500` |
| lineHeight | `16px` | `—` |
| boxShadow | `none` | `none` |

## badge-default-dark.png — Badge · `badge-demo` — default (default/) — dark

ref rect: 54 x 22 CSS px (element `[data-slot="badge"]`)

| field | reference | native (measured) |
|---|---|---|
| width | `54.1094px` | `52.347656 DIP` |
| height | `22px` | `22 DIP` |
| padding | `2px 8px` | `2 x 8 DIP` |
| borderRadius | `3.35544e+07px` | `11 DIP (pill)` |
| borderWidth | `1px` | `1 DIP (transparent)` |
| borderColor | `rgba(0, 0, 0, 0) #00000000` | `transparent` |
| backgroundColor | `lab(90.952 0 -0.0000119209) #E5E5E5` | `#E5E5E5` |
| color | `lab(7.78201 -0.0000149012 0) #171717` | `#171717` |
| fontFamily | `Geist, "Geist Fallback"` | `Segoe UI Variable Text` |
| fontSize | `12px` | `12 DIP` |
| fontWeight | `500` | `500` |
| lineHeight | `16px` | `—` |
| boxShadow | `none` | `none` |

## badge-secondary-light.png — Badge · `badge-secondary` — default (secondary/) — light

ref rect: 78 x 22 CSS px (element `[data-slot="badge"]`)

| field | reference | native (measured) |
|---|---|---|
| width | `78.2969px` | `74.75391 DIP` |
| height | `22px` | `22 DIP` |
| padding | `2px 8px` | `2 x 8 DIP` |
| borderRadius | `3.35544e+07px` | `11 DIP (pill)` |
| borderWidth | `1px` | `1 DIP (transparent)` |
| borderColor | `rgba(0, 0, 0, 0) #00000000` | `transparent` |
| backgroundColor | `lab(96.52 -0.0000298023 0.0000119209) #F5F5F5` | `#F5F5F5` |
| color | `lab(7.78201 -0.0000149012 0) #171717` | `#171717` |
| fontFamily | `Geist, "Geist Fallback"` | `Segoe UI Variable Text` |
| fontSize | `12px` | `12 DIP` |
| fontWeight | `500` | `500` |
| lineHeight | `16px` | `—` |
| boxShadow | `none` | `none` |

## badge-secondary-dark.png — Badge · `badge-secondary` — default (secondary/) — dark

ref rect: 78 x 22 CSS px (element `[data-slot="badge"]`)

| field | reference | native (measured) |
|---|---|---|
| width | `78.2969px` | `74.75391 DIP` |
| height | `22px` | `22 DIP` |
| padding | `2px 8px` | `2 x 8 DIP` |
| borderRadius | `3.35544e+07px` | `11 DIP (pill)` |
| borderWidth | `1px` | `1 DIP (transparent)` |
| borderColor | `rgba(0, 0, 0, 0) #00000000` | `transparent` |
| backgroundColor | `lab(15.204 0 -0.00000596046) #262626` | `#262626` |
| color | `lab(98.26 0 0) #FAFAFA` | `#FAFAFA` |
| fontFamily | `Geist, "Geist Fallback"` | `Segoe UI Variable Text` |
| fontSize | `12px` | `12 DIP` |
| fontWeight | `500` | `500` |
| lineHeight | `16px` | `—` |
| boxShadow | `none` | `none` |

## badge-outline-light.png — Badge · `badge-outline` — default (outline/) — light

ref rect: 59 x 22 CSS px (element `[data-slot="badge"]`)

| field | reference | native (measured) |
|---|---|---|
| width | `58.9531px` | `58.054688 DIP` |
| height | `22px` | `22 DIP` |
| padding | `2px 8px` | `2 x 8 DIP` |
| borderRadius | `3.35544e+07px` | `11 DIP (pill)` |
| borderWidth | `1px` | `1 DIP (border)` |
| borderColor | `lab(90.952 0 -0.0000119209) #E5E5E5` | `#E5E5E5` |
| backgroundColor | `rgba(0, 0, 0, 0) #00000000` | `#00000000` |
| color | `lab(0 0 0) #000000` | `#0A0A0A` |
| fontFamily | `Geist, "Geist Fallback"` | `Segoe UI Variable Text` |
| fontSize | `12px` | `12 DIP` |
| fontWeight | `500` | `500` |
| lineHeight | `16px` | `—` |
| boxShadow | `none` | `none` |

## badge-outline-dark.png — Badge · `badge-outline` — default (outline/) — dark

ref rect: 59 x 22 CSS px (element `[data-slot="badge"]`)

| field | reference | native (measured) |
|---|---|---|
| width | `58.9531px` | `58.054688 DIP` |
| height | `22px` | `22 DIP` |
| padding | `2px 8px` | `2 x 8 DIP` |
| borderRadius | `3.35544e+07px` | `11 DIP (pill)` |
| borderWidth | `1px` | `1 DIP (border)` |
| borderColor | `lab(100 0 0 / 0.1) #FFFFFF1A` | `#FFFFFF1A` |
| backgroundColor | `rgba(0, 0, 0, 0) #00000000` | `#00000000` |
| color | `lab(98.26 0 0) #FAFAFA` | `#FAFAFA` |
| fontFamily | `Geist, "Geist Fallback"` | `Segoe UI Variable Text` |
| fontSize | `12px` | `12 DIP` |
| fontWeight | `500` | `500` |
| lineHeight | `16px` | `—` |
| boxShadow | `none` | `none` |

## tooltip-open-light.png — Tooltip · `tooltip-demo` — open (outline/default) — light

ref rect: 98 x 28 CSS px (element `[data-slot="tooltip-content"]`)

| field | reference | native (measured) |
|---|---|---|
| width | `97.5px` | `96.66797 DIP` |
| height | `28px` | `28 DIP` |
| padding | `6px 12px` | `6 x 12 DIP` |
| borderRadius | `8px` | `8 DIP` |
| borderWidth | `0px` | `—` |
| borderColor | `lab(90.952 0 -0.0000119209) #E5E5E5` | `—` |
| backgroundColor | `lab(0 0 0) #000000` | `#0A0A0A` |
| color | `lab(100 0 0) #FFFFFF` | `#FFFFFF` |
| fontFamily | `Geist, "Geist Fallback"` | `Segoe UI Variable Text` |
| fontSize | `12px` | `12 DIP` |
| fontWeight | `400` | `400` |
| lineHeight | `16px` | `—` |
| boxShadow | `none` | `none` |

## tooltip-open-dark.png — Tooltip · `tooltip-demo` — open (outline/default) — dark

ref rect: 98 x 28 CSS px (element `[data-slot="tooltip-content"]`)

| field | reference | native (measured) |
|---|---|---|
| width | `97.5px` | `96.66797 DIP` |
| height | `28px` | `28 DIP` |
| padding | `6px 12px` | `6 x 12 DIP` |
| borderRadius | `8px` | `8 DIP` |
| borderWidth | `0px` | `—` |
| borderColor | `lab(100 0 0 / 0.1) #FFFFFF1A` | `—` |
| backgroundColor | `lab(98.26 0 0) #FAFAFA` | `#FAFAFA` |
| color | `lab(2.75381 0 0) #0A0A0A` | `#0A0A0A` |
| fontFamily | `Geist, "Geist Fallback"` | `Segoe UI Variable Text` |
| fontSize | `12px` | `12 DIP` |
| fontWeight | `400` | `400` |
| lineHeight | `16px` | `—` |
| boxShadow | `none` | `none` |

## separator-light.png — Separator · `separator-demo` — default (/) — light

ref rect: 380 x 1 CSS px (element `[data-slot="separator"]`)

| field | reference | native (measured) |
|---|---|---|
| width | `380px` | `380 DIP` |
| height | `1px` | `1 DIP` |
| padding | `0px` | `—` |
| borderRadius | `0px` | `—` |
| borderWidth | `0px` | `—` |
| borderColor | `lab(90.952 0 -0.0000119209) #E5E5E5` | `—` |
| backgroundColor | `lab(90.952 0 -0.0000119209) #E5E5E5` | `#E5E5E5` |
| color | `rgb(0, 0, 0) #000000` | `—` |
| fontFamily | `Geist, "Geist Fallback"` | `—` |
| fontSize | `16px` | `—` |
| fontWeight | `400` | `—` |
| lineHeight | `24px` | `—` |
| boxShadow | `none` | `none` |

## separator-dark.png — Separator · `separator-demo` — default (/) — dark

ref rect: 380 x 1 CSS px (element `[data-slot="separator"]`)

| field | reference | native (measured) |
|---|---|---|
| width | `380px` | `380 DIP` |
| height | `1px` | `1 DIP` |
| padding | `0px` | `—` |
| borderRadius | `0px` | `—` |
| borderWidth | `0px` | `—` |
| borderColor | `lab(100 0 0 / 0.1) #FFFFFF1A` | `—` |
| backgroundColor | `lab(100 0 0 / 0.1) #FFFFFF1A` | `#FFFFFF1A` |
| color | `rgb(255, 255, 255) #FFFFFF` | `—` |
| fontFamily | `Geist, "Geist Fallback"` | `—` |
| fontSize | `16px` | `—` |
| fontWeight | `400` | `—` |
| lineHeight | `24px` | `—` |
| boxShadow | `none` | `none` |

## textarea-default-light.png — Composer · `textarea-demo` — default (/) — light

ref rect: 380 x 64 CSS px (element `[data-slot="textarea"]`)

| field | reference | native (measured) |
|---|---|---|
| width | `380px` | `380 DIP` |
| height | `64px` | `52 DIP (composer, measured)` |
| padding | `8px 12px` | `editor insets 16 DIP x / 16 DIP y; send edge 10 DIP` |
| borderRadius | `8px` | `14 DIP (bubble)` |
| borderWidth | `1px` | `1 DIP` |
| borderColor | `lab(90.952 0 -0.0000119209) #E5E5E5` | `#E5E5E5` |
| backgroundColor | `rgba(0, 0, 0, 0) #00000000` | `#FFFFFF` |
| color | `rgb(0, 0, 0) #000000` | `#0A0A0A` |
| fontFamily | `Geist, "Geist Fallback"` | `Segoe UI Variable Text` |
| fontSize | `14px` | `14 DIP` |
| fontWeight | `400` | `400` |
| lineHeight | `20px` | `20 DIP` |
| boxShadow | `rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0.05) 0px 1px 2px 0px` | `—` |
| composer | — | `380x52 DIP` |

## textarea-default-dark.png — Composer · `textarea-demo` — default (/) — dark

ref rect: 380 x 64 CSS px (element `[data-slot="textarea"]`)

| field | reference | native (measured) |
|---|---|---|
| width | `380px` | `380 DIP` |
| height | `64px` | `52 DIP (composer, measured)` |
| padding | `8px 12px` | `editor insets 16 DIP x / 16 DIP y; send edge 10 DIP` |
| borderRadius | `8px` | `14 DIP (bubble)` |
| borderWidth | `1px` | `1 DIP` |
| borderColor | `lab(100 0 0 / 0.15) #FFFFFF26` | `#FFFFFF1A` |
| backgroundColor | `oklab(0.999998 -0.00000980496 0.0000234246 / 0.045) #FFFFFF0B` | `#171717` |
| color | `rgb(255, 255, 255) #FFFFFF` | `#FAFAFA` |
| fontFamily | `Geist, "Geist Fallback"` | `Segoe UI Variable Text` |
| fontSize | `14px` | `14 DIP` |
| fontWeight | `400` | `400` |
| lineHeight | `20px` | `20 DIP` |
| boxShadow | `rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0.05) 0px 1px 2px 0px` | `—` |
| composer | — | `380x52 DIP` |

## textarea-focus-visible-light.png — Composer · `textarea-demo` — focus-visible (/) — light

ref rect: 380 x 64 CSS px (element `[data-slot="textarea"]`)

| field | reference | native (measured) |
|---|---|---|
| width | `380px` | `380 DIP` |
| height | `64px` | `52 DIP (composer, measured)` |
| padding | `8px 12px` | `editor insets 16 DIP x / 16 DIP y; send edge 10 DIP` |
| borderRadius | `8px` | `14 DIP (bubble)` |
| borderWidth | `1px` | `1 DIP` |
| borderColor | `lab(90.952 0 -0.0000119209) #E5E5E5` | `#E5E5E5` |
| backgroundColor | `rgba(0, 0, 0, 0) #00000000` | `#FFFFFF` |
| color | `rgb(0, 0, 0) #000000` | `#0A0A0A` |
| fontFamily | `Geist, "Geist Fallback"` | `Segoe UI Variable Text` |
| fontSize | `14px` | `14 DIP` |
| fontWeight | `400` | `400` |
| lineHeight | `20px` | `20 DIP` |
| boxShadow | `rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0.05) 0px 1px 2px 0px` | `—` |
| composer | — | `380x52 DIP` |

## textarea-focus-visible-dark.png — Composer · `textarea-demo` — focus-visible (/) — dark

ref rect: 380 x 64 CSS px (element `[data-slot="textarea"]`)

| field | reference | native (measured) |
|---|---|---|
| width | `380px` | `380 DIP` |
| height | `64px` | `52 DIP (composer, measured)` |
| padding | `8px 12px` | `editor insets 16 DIP x / 16 DIP y; send edge 10 DIP` |
| borderRadius | `8px` | `14 DIP (bubble)` |
| borderWidth | `1px` | `1 DIP` |
| borderColor | `lab(100 0 0 / 0.15) #FFFFFF26` | `#FFFFFF1A` |
| backgroundColor | `oklab(0.999998 -0.00000980496 0.0000234246 / 0.045) #FFFFFF0B` | `#171717` |
| color | `rgb(255, 255, 255) #FFFFFF` | `#FAFAFA` |
| fontFamily | `Geist, "Geist Fallback"` | `Segoe UI Variable Text` |
| fontSize | `14px` | `14 DIP` |
| fontWeight | `400` | `400` |
| lineHeight | `20px` | `20 DIP` |
| boxShadow | `rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0.05) 0px 1px 2px 0px` | `—` |
| composer | — | `380x52 DIP` |

## textarea-disabled-light.png — Composer · `textarea-disabled` — disabled (/) — light

ref rect: 380 x 64 CSS px (element `[data-slot="textarea"]`)

| field | reference | native (measured) |
|---|---|---|
| width | `380px` | `380 DIP` |
| height | `64px` | `52 DIP (composer, measured)` |
| padding | `8px 12px` | `editor insets 16 DIP x / 16 DIP y; send edge 10 DIP` |
| borderRadius | `8px` | `14 DIP (bubble)` |
| borderWidth | `1px` | `1 DIP` |
| borderColor | `lab(90.952 0 -0.0000119209) #E5E5E5` | `#E5E5E5` |
| backgroundColor | `rgba(0, 0, 0, 0) #00000000` | `#FFFFFF` |
| color | `rgb(0, 0, 0) #000000` | `#0A0A0A` |
| fontFamily | `Geist, "Geist Fallback"` | `Segoe UI Variable Text` |
| fontSize | `14px` | `14 DIP` |
| fontWeight | `400` | `400` |
| lineHeight | `20px` | `20 DIP` |
| boxShadow | `rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0.05) 0px 1px 2px 0px` | `—` |
| composer | — | `380x52 DIP` |

## textarea-disabled-dark.png — Composer · `textarea-disabled` — disabled (/) — dark

ref rect: 380 x 64 CSS px (element `[data-slot="textarea"]`)

| field | reference | native (measured) |
|---|---|---|
| width | `380px` | `380 DIP` |
| height | `64px` | `52 DIP (composer, measured)` |
| padding | `8px 12px` | `editor insets 16 DIP x / 16 DIP y; send edge 10 DIP` |
| borderRadius | `8px` | `14 DIP (bubble)` |
| borderWidth | `1px` | `1 DIP` |
| borderColor | `lab(100 0 0 / 0.15) #FFFFFF26` | `#FFFFFF1A` |
| backgroundColor | `oklab(0.999998 -0.00000980496 0.0000234246 / 0.045) #FFFFFF0B` | `#171717` |
| color | `rgb(255, 255, 255) #FFFFFF` | `#FAFAFA` |
| fontFamily | `Geist, "Geist Fallback"` | `Segoe UI Variable Text` |
| fontSize | `14px` | `14 DIP` |
| fontWeight | `400` | `400` |
| lineHeight | `20px` | `20 DIP` |
| boxShadow | `rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0.05) 0px 1px 2px 0px` | `—` |
| composer | — | `380x52 DIP` |

## card-light.png — Surface · `card-demo` — default (/) — light

ref rect: 384 x 388 CSS px (element `[data-slot="card"]`)

| field | reference | native (measured) |
|---|---|---|
| width | `384px` | `384 DIP` |
| height | `388px` | `388 DIP` |
| padding | `24px 0px` | `—` |
| borderRadius | `14px` | `14 DIP` |
| borderWidth | `1px` | `1 DIP` |
| borderColor | `lab(90.952 0 -0.0000119209) #E5E5E5` | `#E5E5E5` |
| backgroundColor | `lab(100 0 0) #FFFFFF` | `#FFFFFF` |
| color | `lab(0 0 0) #000000` | `—` |
| fontFamily | `Geist, "Geist Fallback"` | `—` |
| fontSize | `16px` | `—` |
| fontWeight | `400` | `—` |
| lineHeight | `24px` | `—` |
| boxShadow | `rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0.1) 0px 1px 3px 0px, rgba(0, 0, 0, 0.1) 0px 1px 2px -1px` | `composited drop shadow` |

## card-dark.png — Surface · `card-demo` — default (/) — dark

ref rect: 384 x 388 CSS px (element `[data-slot="card"]`)

| field | reference | native (measured) |
|---|---|---|
| width | `384px` | `384 DIP` |
| height | `388px` | `388 DIP` |
| padding | `24px 0px` | `—` |
| borderRadius | `14px` | `14 DIP` |
| borderWidth | `1px` | `1 DIP` |
| borderColor | `lab(100 0 0 / 0.1) #FFFFFF1A` | `#FFFFFF1A` |
| backgroundColor | `lab(7.78201 -0.0000149012 0) #171717` | `#171717` |
| color | `lab(98.26 0 0) #FAFAFA` | `—` |
| fontFamily | `Geist, "Geist Fallback"` | `—` |
| fontSize | `16px` | `—` |
| fontWeight | `400` | `—` |
| lineHeight | `24px` | `—` |
| boxShadow | `rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0) 0px 0px 0px 0px, rgba(0, 0, 0, 0.1) 0px 1px 3px 0px, rgba(0, 0, 0, 0.1) 0px 1px 2px -1px` | `composited drop shadow` |

## typography-p-light.png — Typography · `typography-p` — default (/) — light

ref rect: 380 x 84 CSS px (element `p:first-of-type`)

| field | reference | native (measured) |
|---|---|---|
| width | `380px` | `380 DIP (ref wrap width)` |
| height | `84px` | `37 DIP` |
| padding | `0px` | `—` |
| borderRadius | `0px` | `—` |
| borderWidth | `0px` | `—` |
| borderColor | `lab(90.952 0 -0.0000119209) #E5E5E5` | `—` |
| backgroundColor | `rgba(0, 0, 0, 0) #00000000` | `—` |
| color | `rgb(0, 0, 0) #000000` | `#0A0A0A` |
| fontFamily | `Geist, "Geist Fallback"` | `Segoe UI Variable Text` |
| fontSize | `16px` | `14 DIP` |
| fontWeight | `400` | `400` |
| lineHeight | `28px` | `—` |
| boxShadow | `none` | `—` |
| native style | — | `body 14/400` |

## typography-p-dark.png — Typography · `typography-p` — default (/) — dark

ref rect: 380 x 84 CSS px (element `p:first-of-type`)

| field | reference | native (measured) |
|---|---|---|
| width | `380px` | `380 DIP (ref wrap width)` |
| height | `84px` | `37 DIP` |
| padding | `0px` | `—` |
| borderRadius | `0px` | `—` |
| borderWidth | `0px` | `—` |
| borderColor | `lab(100 0 0 / 0.1) #FFFFFF1A` | `—` |
| backgroundColor | `rgba(0, 0, 0, 0) #00000000` | `—` |
| color | `rgb(255, 255, 255) #FFFFFF` | `#FAFAFA` |
| fontFamily | `Geist, "Geist Fallback"` | `Segoe UI Variable Text` |
| fontSize | `16px` | `14 DIP` |
| fontWeight | `400` | `400` |
| lineHeight | `28px` | `—` |
| boxShadow | `none` | `—` |
| native style | — | `body 14/400` |

## typography-muted-light.png — Typography · `typography-muted` — default (/) — light

ref rect: 380 x 20 CSS px (element `p:first-of-type`)

| field | reference | native (measured) |
|---|---|---|
| width | `380px` | `380 DIP (ref wrap width)` |
| height | `20px` | `16 DIP` |
| padding | `0px` | `—` |
| borderRadius | `0px` | `—` |
| borderWidth | `0px` | `—` |
| borderColor | `lab(90.952 0 -0.0000119209) #E5E5E5` | `—` |
| backgroundColor | `rgba(0, 0, 0, 0) #00000000` | `—` |
| color | `lab(48.496 0 0) #737373` | `#737373` |
| fontFamily | `Geist, "Geist Fallback"` | `Segoe UI Variable Text` |
| fontSize | `14px` | `12 DIP` |
| fontWeight | `400` | `400` |
| lineHeight | `20px` | `—` |
| boxShadow | `none` | `—` |
| native style | — | `muted 12/400` |

## typography-muted-dark.png — Typography · `typography-muted` — default (/) — dark

ref rect: 380 x 20 CSS px (element `p:first-of-type`)

| field | reference | native (measured) |
|---|---|---|
| width | `380px` | `380 DIP (ref wrap width)` |
| height | `20px` | `16 DIP` |
| padding | `0px` | `—` |
| borderRadius | `0px` | `—` |
| borderWidth | `0px` | `—` |
| borderColor | `lab(100 0 0 / 0.1) #FFFFFF1A` | `—` |
| backgroundColor | `rgba(0, 0, 0, 0) #00000000` | `—` |
| color | `lab(66.128 -0.0000298023 0.0000119209) #A1A1A1` | `#A1A1A1` |
| fontFamily | `Geist, "Geist Fallback"` | `Segoe UI Variable Text` |
| fontSize | `14px` | `12 DIP` |
| fontWeight | `400` | `400` |
| lineHeight | `20px` | `—` |
| boxShadow | `none` | `—` |
| native style | — | `muted 12/400` |

## typography-small-light.png — Typography · `typography-small` — default (/) — light

ref rect: 92 x 18 CSS px (element `small:first-of-type`)

| field | reference | native (measured) |
|---|---|---|
| width | `auto` | `91.9375 DIP (ref wrap width)` |
| height | `auto` | `19 DIP` |
| padding | `0px` | `—` |
| borderRadius | `0px` | `—` |
| borderWidth | `0px` | `—` |
| borderColor | `lab(90.952 0 -0.0000119209) #E5E5E5` | `—` |
| backgroundColor | `rgba(0, 0, 0, 0) #00000000` | `—` |
| color | `rgb(0, 0, 0) #000000` | `#0A0A0A` |
| fontFamily | `Geist, "Geist Fallback"` | `Segoe UI Variable Text` |
| fontSize | `14px` | `14 DIP` |
| fontWeight | `500` | `500` |
| lineHeight | `14px` | `—` |
| boxShadow | `none` | `—` |
| native style | — | `label 14/500` |

## typography-small-dark.png — Typography · `typography-small` — default (/) — dark

ref rect: 92 x 18 CSS px (element `small:first-of-type`)

| field | reference | native (measured) |
|---|---|---|
| width | `auto` | `91.9375 DIP (ref wrap width)` |
| height | `auto` | `19 DIP` |
| padding | `0px` | `—` |
| borderRadius | `0px` | `—` |
| borderWidth | `0px` | `—` |
| borderColor | `lab(100 0 0 / 0.1) #FFFFFF1A` | `—` |
| backgroundColor | `rgba(0, 0, 0, 0) #00000000` | `—` |
| color | `rgb(255, 255, 255) #FFFFFF` | `#FAFAFA` |
| fontFamily | `Geist, "Geist Fallback"` | `Segoe UI Variable Text` |
| fontSize | `14px` | `14 DIP` |
| fontWeight | `500` | `500` |
| lineHeight | `14px` | `—` |
| boxShadow | `none` | `—` |
| native style | — | `label 14/500` |
