---
name: aqua
description: >-
  Runbook and audit checklist for Aqua, a macOS-style Wayland compositor in Rust
  (Smithay, tiny-skia CPU rendering). Use whenever the task touches the aqua
  repo: build/run, UI changes (menu bar, dock, windows, overlays), SSD/CSD window
  decorations, xdg-decoration, move/resize/maximize/minimize, animations,
  smoothness/latency/FPS, rendering bugs, or "fix all issues / audit / refactor".
  Also use for Russian requests: "исправь недочёты", "плавность", "декорации",
  "окна", "лаги", "оптимизируй". Use even if the user doesn't name the project.
---

# Aqua: Wayland compositor (Rust + Smithay), macOS 26 Tahoe look

Stack: Smithay (compositor), tiny-skia (CPU 2D), fontdue (text), winit (nested
dev backend). Repo root = current working directory (never assume an absolute path).

## 0. TOKEN RULES (apply first, always)

1. Never read a file whole. `rg -n "<symbol>" src/`, then `sed -n 'A,Bp' file`.
2. No repo-wide exploration. At most `ls src/ && wc -l src/*.rs`.
3. Build ONE map per session, then work only from it:
   `rg -n "XdgDecorationHandler|decoration_mode|geometry\(\)|move_request|resize_request|send_configure|OutputDamageTracker|VBlank|frame|MemoryRenderBuffer" src/`
4. Do not re-read a file already read this session. Keep notes as `file:line`.
5. Edit with targeted `str_replace`, never rewrite whole files.
6. `cargo check` after each group of edits; `cargo clippy` once at the end.
   Show only errors, never full logs.
7. Report: `file:line, what changed`. No code recaps, no long reasoning.
8. If blocked by ambiguity, ask ONE short question; do not analyze further.
9. Values (sizes, colors, timings) live in code. Read the constant in
   `ui.rs` / `render.rs` / `theme.rs` before relying on a number; do not trust
   numbers remembered from docs.

## 1. Module map

| File | Role |
|---|---|
| `main.rs`, `state.rs` | entry, event loop, `Aqua` state |
| `winit.rs` | backend, render loop, close-animation pump |
| `render.rs` | frame composition, window decorations |
| `ui.rs` | menu bar, dock, wallpaper, hit-testing, `APPS` |
| `input.rs` | pointer/keyboard, shortcuts, dock clicks |
| `theme.rs` | all colors (dark + light) |
| `settings.rs`, `spotlight.rs`, `control_center.rs` | overlays |
| `grabs/` | move/resize pointer grabs |
| `handlers/` | Smithay protocol handlers (xdg_shell, compositor...) |

If the layout differs, trust `ls src/`, not this table.

## 2. Build & run

```bash
cargo check                    # fast
cargo build --release
killall aqua 2>/dev/null; ./target/release/aqua
```
`winit` backend = nested testing inside another Wayland session. Real TTY use
needs the udev/DRM + libinput backend. Check which one exists
(`rg -n "udev|drm|libinput" src/`) before optimizing latency or vblank.

## 3. Windows: SSD and CSD

Policy: **default SSD, honor client choice via `xdg-decoration`**. Clients
without the protocol (GTK4/libadwaita, Electron, some Qt) draw their own CSD;
the compositor must not add a second frame for them.

Full details and code: `references/windows-ssd-csd.md`. Read it only when the
task touches decorations, resize/move, or window states.

Checklist (one or two `rg` queries each):
- [ ] `XdgDecorationHandler` implemented; `request_mode`/`unset_mode` handled; `send_configure()` after change
- [ ] Layout/snap/maximize use `Window::geometry()`, not buffer bbox
- [ ] SSD: client surface offset by titlebar height; configure size = window size minus decorations
- [ ] SSD: hit-test titlebar, edges, traffic lights; double-click titlebar = zoom
- [ ] CSD: `move_request`, `resize_request`, `show_window_menu`, `set_minimized`, `set_maximized`, `unset_maximized`, fullscreen requests handled
- [ ] Window states set: `Maximized`, `Fullscreen`, `Tiled*`, `Activated`; no decorations in fullscreen
- [ ] CSD shadow region stays in input/resize zone; compositor does not draw its own shadow under CSD windows
- [ ] Inactive/active titlebar styling follows `Activated`

## 4. Smoothness (CPU rendering is the main risk)

Everything is rasterized by tiny-skia on the CPU. Rules:
- **Cache rasters.** Dock plate, icons, titlebars, shadows, menu bar text: rasterize
  once, rebuild only on size/state/theme change. Never rasterize per frame.
- **Animate textures, not pixels.** Magnification, open/close, minimize: scale/
  translate/alpha of cached buffers at render time, not re-rasterization.
- **Shadows:** one 9-slice texture stretched to window size, not a multi-layer
  blur per window per frame.
- **Damage tracking:** `OutputDamageTracker`; skip render when nothing changed.
- **Frame pacing:** render on VBlank/frame-done, never `sleep` loops. Send frame
  callbacks to clients after presentation.
- **Cursor:** hardware cursor plane if the DRM backend exists.
- **Heavy work off the main thread:** icon loading, `.desktop` scan (`apps.rs`),
  wallpaper generation.
- **Time-based animation:** progress = `(now - start) / duration`, never per-frame steps.
- **Easing:** open/move/resize/minimize = ease-out or critically-damped spring;
  interruptible, preserving velocity. Linear only for fade-out of opacity.
  Never linear for position/size.

## 4a. Visual rules (macOS 26 Tahoe / Liquid Glass)

For any UI/visual task read `references/design.md` (gap analysis, shapes,
glass recipe, wallpaper, dock, icons, dialogs, motion). If the user says
"make it look like macOS", follow its "Redesign order" one step at a time. Hard rules:
- ALL colors go through `theme.rs` with dark and light variants. Never hardcode.
- Sizes/colors: read constants in code (`PANEL_H`, `DOCK_MARGIN`, `DOCK_HEADROOM`,
  `ICON`, `TITLEBAR_H`, `SHADOW_PAD`), don't trust remembered numbers.
- Z-order (back to front): wallpaper, windows (+shadow +decorations), menu bar,
  dock, dropdowns, spotlight, control center, app switcher, settings.

## 5. Recipes

**Add dock app:** add `AppDef { id, name, command, kind, color_a, color_b }` to
`APPS` in `ui.rs`; update the app-count constant and the `running` array size in
`render.rs::running_apps()`. `id` must match the XDG `app_id`.

**Add overlay panel:** new `src/mypanel.rs` (state + `render_mypanel() ->
Option<MemoryRenderBuffer>`), `mod` in `main.rs`, field in `Aqua`, push element
in `render.rs` when open, intercept clicks in `input.rs` before shell hit-test.

## 6. Known pitfalls

| Symptom | Likely cause / fix |
|---|---|
| Windows under dock | `work_area()` in `ui.rs` must subtract dock height + margin |
| Settings closes on click | click-outside handler in `input.rs` |
| App not marked running | `window_app_id()` vs `AppDef.id` mismatch |
| Minimized app not restored | `minimized_windows` lookup in dock click handler |
| Double frame on GTK/Electron | compositor forcing SSD over CSD client |
| Windows drift when tiled/maximized | using buffer size instead of `geometry()` |
| Icons/dock look like a diamond, not round | broken corner curve in `squircle_path` (`ui.rs`); use superellipse version from design.md |
| Dock looks like a dark box | opaque tint, no blurred wallpaper under it, flat wallpaper |
| Lag during drag/animation | per-frame rasterization; see section 4 |
| Black screen at start | output setup in `winit.rs` |

## 6a. Reference compositors

For "how does cosmic-comp/niri/anvil do X": read `references/reference-compositors.md`
(paths in `~/ref`, `rg` queries, GPL rules). Study first, then implement; never copy GPL code.

## 7. Audit workflow ("fix all issues")

1. Build the map (Token rule 3).
2. Go through sections 3 and 4 checklists; list findings as `file:line: problem`.
3. Fix one finding at a time; `cargo check` per group.
4. Final report only:
   - ✅ fixed: `src/x.rs:123` short description
   - ⚠️ not fixed: reason
