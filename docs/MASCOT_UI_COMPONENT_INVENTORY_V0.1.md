# Mascot UI Component Inventory v0.1

**Status:** authoritative component-scope inventory for the native UI foundation  
**Visual baseline:** shadcn-first  
**Icon baseline:** Lucide  
**Rule:** do not implement the entire shadcn catalog

The purpose of this file is to keep the Mascot UI surface intentionally small while making the components we do own visible, reusable and reviewable.

## 1. Component tiers

### Tier A — implement / maintain now

These are part of the current Mascot product surface or are immediate reusable primitives required by it.

1. **Surface / Bubble**
   - rounded native surface;
   - border;
   - shadow;
   - light/dark;
   - stretchable in width and height.

2. **Typography / Label**
   - body;
   - muted/small;
   - tooltip text;
   - native system typography.

3. **Icon**
   - Lucide geometry;
   - fixed square viewport;
   - current foreground color.

4. **IconButton**
   - primary;
   - ghost;
   - disabled;
   - hover;
   - pressed;
   - focus-visible;
   - fixed control sizes.

5. **Button**
   - default;
   - secondary/neutral;
   - ghost;
   - disabled;
   - focus-visible;
   - compact sizing only.
   - Needed as a reusable non-icon action primitive even if the floating composer mostly uses icon buttons.

6. **Tooltip**
   - compact;
   - single-line first;
   - bounded width;
   - fixed content-driven size, not arbitrary stretching.

7. **Native Text Input / Composer**
   - RichEdit-backed;
   - single-line visual state;
   - multiline bounded state;
   - placeholder;
   - focus;
   - selection;
   - disabled/read-only where relevant;
   - stretchable in width.

8. **Separator**
   - horizontal;
   - one visual weight;
   - stretchable in width.

9. **Response / Content Surface**
   - plain text content region;
   - copy action;
   - stretchable within bounded width;
   - not a generic Markdown/card system.

10. **Badge / Status Pill**
   - compact status text;
   - neutral-first;
   - state must not rely on red-vs-green alone;
   - content-sized, not arbitrarily stretched.

These are the only components that should be implemented or generalized in the current component-gallery task.

### Tier B — approved likely next components, do NOT implement in this task

These are likely useful for M1C/M2/settings/provider selection, but should stay deferred until a concrete product surface needs them.

- ScrollArea
- Popover
- DropdownMenu / MenuItem
- Select
- Switch
- Checkbox
- RadioGroup
- Dialog / lightweight modal
- Settings row
- Progress / activity indicator if mascot animation alone is insufficient (when implemented, it must follow the busy/loading visual rule in `MASCOT_NATIVE_UI_DESIGN_SYSTEM_V0.1.md` §2)

The preview/gallery should list these as **planned / not implemented**, not silently create them.

### Tier C — explicitly deferred

Do not build these merely because shadcn has them:

- Accordion
- Alert
- AlertDialog
- AspectRatio
- Avatar
- Breadcrumb
- Calendar
- Carousel
- Chart
- Collapsible
- Command palette
- ContextMenu
- DataTable / Table
- Drawer
- HoverCard
- InputOTP
- Menubar
- NavigationMenu
- Pagination
- Resizable panels
- Skeleton
- Slider
- Sonner/toast system
- Tabs
- ToggleGroup
- full Form framework
- date/time picker
- generic card library

A future task may promote a Tier B/C component only when a concrete Mascot use case exists.

## 2. Sizing policy

Components fall into three sizing classes.

### Fixed-size controls

Examples:

- Icon
- IconButton
- Checkbox (future)
- Radio (future)
- Switch (future)

Preview them at their supported discrete size(s). Do not create artificial stretched variants.

### Content-sized controls

Examples:

- Button
- Tooltip
- Badge

Preview representative short/medium/long labels and supported compact size variants. Do not stretch them to arbitrary widths unless the product requires full-width behavior.

### Stretchable components

Examples:

- Surface/Bubble
- Text Input/Composer
- Separator
- Response/Content Surface

Preview at minimum:

- minimum practical width;
- current default width;
- wider practical width.

The gallery must make truncation, wrapping, alignment and border/radius behavior visible.

## 3. State coverage

For interactive Tier A components, preview relevant states:

- default;
- hover;
- pressed;
- focus-visible;
- disabled;
- selected/copied/read-only only where meaningful.

Do not manufacture irrelevant states solely for symmetry.

## 4. Theme coverage

Every Tier A component must appear in:

- light;
- dark.

The gallery must make direct visual comparison possible without opening many individual screenshots.

## 5. Reference discipline

shadcn is the visual reference, not a runtime dependency.

For each Tier A component that has a direct shadcn analogue:

- record the official shadcn component/reference URL;
- record the reference capture date/version when available;
- include a small visual reference crop in the developer-only comparison sheet where practical;
- note intentional Mascot deviations.

Lucide remains the canonical icon source.

Do not copy React/Tailwind implementation code into the native runtime.
