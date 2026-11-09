---
name: aqua-compositor
description: >-
  Complete reference and runbook for the Aqua macOS-style Wayland compositor
  written in Rust. Use when working on the aqua project: building, running,
  modifying UI, adding features, debugging rendering issues, or understanding
  the codebase architecture.
---

# Aqua — macOS Wayland Compositor (Rust + Smithay)

## Project Overview

**Aqua** is a custom Wayland compositor that replicates the macOS Sequoia
visual experience on Linux. It is written 100% in Rust using:

- **Smithay** — Wayland compositor framework
- **tiny-skia** — CPU-side GPU-free 2D rasterizer (all UI is drawn on CPU)
- **fontdue** — Font rasterizer (SF Pro Text Regular + Bold embedded)
- **winit** — Window/backend abstraction

Location: `/home/anc/aqua/`

---

## Architecture

```
src/
  main.rs          — Entry point, event loop setup
  state.rs         — Aqua compositor state (all fields)
  winit.rs         — Backend: render loop, window close animation pump
  render.rs        — Frame rendering: wallpaper → windows → panel → dock → overlays
  ui.rs            — Shell: menu bar, dock, wallpaper rasterization + hit testing
  input.rs         — Pointer/keyboard event handling
  text.rs          — TextRenderer using fontdue + SF Pro fonts
  theme.rs         — Theme colors (dark/light), wallpaper palette, dock/window colors
  settings.rs      — System settings overlay window (Dock config, Appearance, About)
  spotlight.rs     — Spotlight search popup + dropdown menus (Apple, File, Edit…)
  control_center.rs— Control Center popup + App Switcher (Super+Tab)
  apps.rs          — Scans .desktop files for Spotlight results
  grabs/           — Smithay pointer grab handlers (move, resize)
  handlers/        — Smithay protocol handlers (xdg_shell, compositor…)
assets/fonts/
  SF-Pro-Text-Regular.otf   — Apple San Francisco Text Regular (embedded)
  SF-Pro-Text-Bold.otf      — Apple San Francisco Text Bold (embedded)
```

---

## Build & Run

```bash
# Check (fast, no binary)
export PATH="$HOME/.cargo/bin:$PATH"
cargo check

# Release build (optimized, ~27s)
cargo build --release

# Run (kill previous instance first)
killall aqua 2>/dev/null || true
/home/anc/aqua/target/release/aqua
```

> **Note**: Aqua must run inside a Wayland session (e.g. from a TTY via
> `startx`, or inside an existing Wayland compositor for testing with winit).

---

## Key Constants & Layout

| Constant | File | Value | Purpose |
|---|---|---|---|
| `PANEL_H` | `ui.rs` | 30 | Menu bar height (px) |
| `DOCK_MARGIN` | `ui.rs` | 4 | Gap between dock bottom and screen edge |
| `DOCK_HEADROOM` | `ui.rs` | 34 | Space above dock for magnified icons + tooltips |
| `ICON` | `ui.rs` | 48 | Default dock icon size (px) |
| `TITLEBAR_H` | `render.rs` | 32 | Titlebar height for windows |
| `SHADOW_PAD` | `render.rs` | 20 | Extra padding around windows for drop shadows |

---

## Visual Design (macOS Sequoia Dark)

### Menu Bar
- **Fully transparent** — no background fill in dark mode
- Light mode: faint white tint (alpha ≈ 110)
- Hairline separator at bottom (alpha ≈ 12)
- Text: SF Pro Text 13pt white (dark) / dark (light)

### Dock
- **Glass dark pill**: `[28, 28, 36, 170]` RGBA, 16px border radius
- Positioned `DOCK_MARGIN = 4px` above the bottom screen edge
- Magnification wave on hover (cosine bump, configurable scale)
- Tooltip floats `6–8px` above glass in `DOCK_HEADROOM` zone
- Running indicator dots at bottom of glass plate

### Wallpaper (Dark)
- Base: indigo → near-black vertical gradient `[18,14,42]` → `[6,4,14]`
- Aurora blobs: purple `[68,48,148]` and deep violet `[38,28,100]`
- 5 overlapping soft blobs create the Sequoia swirling "S-curve" effect

### Windows
- Rounded corners (12px), drop shadow (12-layer soft)
- Traffic lights at `(14, mid-titlebar)`: Close 🔴, Minimize 🟡, Maximize 🟢
- Server-side decorations only

---

## State Fields (Aqua struct in state.rs)

| Field | Type | Purpose |
|---|---|---|
| `shell` | `Shell` | Builds and caches panel/dock/wallpaper MemoryRenderBuffers |
| `deco` | `DecoCache` | Per-window decoration (titlebar, border, traffic lights) cache |
| `settings` | `SettingsWindow` | Settings overlay state + DockConfig |
| `settings_dragging` | `Option<(Point, Point)>` | Settings window drag state |
| `minimized_windows` | `HashMap<u32, (Window, Point)>` | Hidden windows (yellow button) |
| `maximized` | `HashMap<u32, (Point, Size)>` | Saved geometry for maximized windows |
| `spotlight` | `SpotlightState` | Spotlight search popup |
| `control_center` | `ControlCenterState` | Control Center popup |
| `switcher` | `AppSwitcherState` | Super+Tab app switcher |
| `window_open_animations` | `HashMap<u32, Instant>` | Fade-in state per window |
| `window_close_animations` | `HashMap<u32, (Instant, Window)>` | Fade-out before unmap |

---

## Keyboard Shortcuts

| Shortcut | Action |
|---|---|
| `Super + Space` | Open Spotlight search |
| `Super + Tab` | App Switcher |
| `Shift + Super + D` | Toggle dark/light theme |
| `Super + Q` | Close focused window |
| `Super + M` | Minimize focused window (future) |

---

## Adding a New Dock App

1. Add entry to `APPS` array in `src/ui.rs`:

```rust
AppDef {
    id: "myapp",          // used for running indicator matching
    name: "My App",       // tooltip label
    command: "myapp",     // shell command to launch
    kind: IconKind::Files, // icon shape
    color_a: [R, G, B, 255],
    color_b: [R, G, B, 255],
},
```

2. Update `APP_COUNT` constant if changed from 6.
3. Update `running` array size in `src/render.rs` → `running_apps()`.

---

## Adding a New Overlay Panel

Pattern used by Spotlight, Control Center, Settings:

1. Create `src/mypanel.rs` with state struct + `render_mypanel() -> Option<MemoryRenderBuffer>`
2. Add `pub mod mypanel;` to `src/main.rs`
3. Add `pub my_panel: crate::mypanel::MyPanelState` to `Aqua` in `state.rs`
4. In `render.rs`: check `state.my_panel.open`, compute position, push element
5. In `input.rs`: intercept pointer clicks before shell hit-test

---

## Common Issues & Fixes

| Problem | Root Cause | Fix |
|---|---|---|
| Dock overlaps windows | `work_area()` not reserving dock space | Check `ui.rs::work_area()` subtracts DOCK_H + DOCK_MARGIN |
| Settings closes on click | Click-outside handler in input.rs | Don't call `settings.close()` on outside clicks |
| App not shown as running | `window_app_id()` mismatch | Check app `id` matches XDG `app_id` or window title |
| Minimized app not restored | `minimized_windows` lookup | Verify id/name match in `input.rs` dock click handler |
| Font looks wrong | Wrong font file | Ensure `SF-Pro-Text-Regular.otf` is in `assets/fonts/` |
| Black screen on start | Smithay output not configured | Check `winit.rs` output setup |
