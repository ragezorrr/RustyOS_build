---
name: aqua-visual-guidelines
description: >-
  Visual design guidelines and color palette for the Aqua compositor macOS
  Sequoia theme. Use when adding new UI elements, overlays, icons, or adjusting
  the appearance to match macOS Sequoia styling.
---

# Aqua Visual Design Guidelines (macOS Sequoia Style)

## Core Principles

1. **Transparency first** — UI elements float over the wallpaper; avoid opaque fills
2. **Soft glass** — translucent panels with subtle border highlights and drop shadows
3. **Motion** — smooth fade-in/out (200ms), gentle slide animations
4. **Typography** — SF Pro Text, 11–13pt, tight letter-spacing
5. **Restraint** — use color sparingly; most UI is near-black or white with alpha

---

## Color Palette (Dark Mode)

### Menu Bar
```
Background:  transparent (alpha = 0)
Text bright: [242, 242, 247, 255]   — white-ish SF Pro labels
Text dim:    [180, 180, 190, 255]   — menu items, secondary
Separator:   [255, 255, 255,  12]   — 1px hairline
```

### Dock
```
Glass fill:  [28,  28,  36,  170]   — dark navy glass
Border:      [255, 255, 255,  45]   — 1px bright rim
Shadow:      [0,   0,   0,   8-12] — 4-layer grow shadow
Corner r:    16px outer, 15px inner
```

### Windows
```
Titlebar:    [32,  32,  38,  245]   — almost opaque dark
Client bg:   drawn by the app itself
Border:      [255, 255, 255,  28]   — hairline top
Shadow:      12-layer, grow 1.2px each, [0,0,0, 8] per layer
Corner r:    12px
```

### Wallpaper (Dark)
```
Gradient top:    [18,  14,  42,  255]   — deep indigo
Gradient bottom: [6,   4,   14,  255]   — near-black
Aurora glow 1:   [68,  48,  148, 255]  — purple-violet (alpha 38 blobs)
Aurora glow 2:   [38,  28,  100, 255]  — deep blue-violet (alpha 30 blobs)
Blob placement: 5 overlapping radial blobs forming "S-curve"
```

### Overlays (Spotlight, Control Center, Settings)
```
Panel bg dark:   [30,  30,  38,  230]   — frosted glass dark
Panel bg light:  [245, 245, 248, 220]
Panel border:    [255, 255, 255,  35]   — rim glow
Panel shadow:    [0,   0,   0,  120]   — deeper shadow than windows
Corner r:        14px
```

---

## Color Palette (Light Mode)

```
Menu bar bg:  [255, 255, 255, 110]   — translucent white
Menu text:    [28,  28,  32,  255]   — near-black
Dock glass:   [255, 255, 255, 150]
Dock border:  [0,   0,   0,   30]
Wallpaper top: [255, 214, 196, 255]  — peach
Wallpaper bot: [168, 196, 236, 255]  — sky blue
```

---

## Typography

| Usage | Font | Size | Weight |
|---|---|---|---|
| App name in menu bar | SF Pro Text Bold | 13pt | Bold |
| Menu items | SF Pro Text Regular | 13pt | Regular |
| Dock tooltip | SF Pro Text Regular | 11pt | Regular |
| Window title | SF Pro Text Bold | 13pt | Bold |
| Settings labels | SF Pro Text Regular | 13pt | Regular |
| Settings section headers | SF Pro Text Bold | 11pt | Bold |
| Clock | SF Pro Text Regular | 13pt | Regular |
| Battery % | SF Pro Text Regular | 12pt | Regular |

**Baseline**: typically `panel_h * 0.65` or `icon_bottom - 4px`

---

## Spacing & Geometry

```
Dock icon size (default): 48px
Dock icon gap:            14px  (or icon_size * 0.28 when configurable)
Dock padding:             14px left/right
Dock margin from bottom:  4px   (almost flush, macOS style)
Dock headroom above:      34px  (tooltip + magnification space)

Menu bar height:          30px
Titlebar height:          32px
Window min size:          160 × 80px (without titlebar)

Settings window:          680 × 460px
Control Center:           320 × 300px (approx)
Spotlight:                620px wide, variable height
```

---

## Icons & Symbols

Dock icons are drawn procedurally in `ui.rs::draw_app_icon()` using tiny-skia paths.

Each `AppDef` has:
- `kind: IconKind` → selects the drawing routine
- `color_a`, `color_b` → gradient stops for the icon background

Available `IconKind` variants:
- `Files` — Finder smiling face (two-tone)
- `Browser` — Safari compass rose
- `Terminal` — `>_` prompt
- `Editor` — lined notepad
- `Music` — double eighth note on gradient
- `Settings` — gear wheel

---

## Animation Timings

| Animation | Duration | Easing |
|---|---|---|
| Window open | 200ms | Ease-out cubic |
| Window close | 160ms | Linear fade |
| Window move | 200ms | Linear interpolation |
| Dock icon bounce | 500ms | Decaying sine (2 bounces) |
| Settings drag | Immediate | No easing |

---

## Rendering Order (back to front)

1. Wallpaper (full-screen)
2. App windows (back to front by stacking order)
3. Per-window drop shadows
4. Per-window decorations (titlebar, traffic lights, border)
5. Menu bar panel (top)
6. Dock (bottom center)
7. Dropdown menus (if open)
8. Spotlight (if open)
9. Control Center (if open)
10. App Switcher (if open)
11. Settings window (if open, draggable)

---

## Adding New Colors to Theme

Always add both dark and light variants in `src/theme.rs`:

```rust
pub fn my_new_color(&self) -> [u8; 4] {
    if self.dark() {
        [R, G, B, A]  // dark mode value
    } else {
        [R, G, B, A]  // light mode value
    }
}
```

Then use `theme.my_new_color()` in `ui.rs` or `render.rs`. Never hardcode
colors outside `theme.rs`.
