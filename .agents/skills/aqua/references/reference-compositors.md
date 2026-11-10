# Reference compositors (read-only study material)

Use to learn how real compositors solve a problem, then implement it in Aqua
in your own words. Read this file only when the task says "see how
cosmic-comp / niri / anvil does X" or when starting a backend/protocol/animation task.

## Locations (cloned outside the Aqua repo)
| Path | What | License |
|---|---|---|
| `~/ref/smithay/anvil` | minimal reference compositor, `src/udev.rs` = DRM backend | MIT (safe to adapt) |
| `~/ref/niri` | scrollable tiling; best animations, frame clock, damage | GPL-3.0 |
| `~/ref/cosmic-comp` | COSMIC desktop compositor; protocols, KMS, XWayland, config | GPL-3.0 |

GPL-3.0 projects: study and explain the design, then write original code.
Never paste their code into Aqua unless Aqua is GPL-3.0.
Start with `anvil` (smallest, MIT), then niri/cosmic-comp for production detail.

## Hard rules for reading
1. Never open a file whole. `rg -n` first, then `sed -n 'A,Bp' file`.
2. Search with explicit absolute paths, e.g. `rg -n "VBlank" ~/ref/niri/src`.
   If using a symlink inside the project, add `-L` (and `--no-ignore` if ignored).
3. First run `ls <repo>/src` and `wc -l` once; paths below may have moved.
4. Check the Smithay version before borrowing an API shape:
   `rg -n "^smithay" ~/ref/<repo>/Cargo.toml` and compare with Aqua's `Cargo.toml`.
   If versions differ, adapt to Aqua's version; do not assume identical signatures.
5. Output of a study task = a short explanation (<= 15 lines) with
   `repo/path:line` pointers. No code dumps.

## Where to look (verify with ls)
| Aqua need | cosmic-comp | niri | anvil |
|---|---|---|---|
| DRM/KMS backend, vblank, planes | `src/backend/kms/` | `src/backend/tty.rs` | `anvil/src/udev.rs` |
| Nested (winit) backend | `src/backend/winit.rs` | `src/backend/winit.rs` | `anvil/src/winit.rs` |
| Render, damage tracking, effects | `src/backend/render/` | `src/render_helpers/` | `anvil/src/render.rs` |
| Frame clock / animation timing | n/a | `src/animation/`, `src/frame_clock.rs` | n/a |
| Window layout, focus, states | `src/shell/` | `src/layout/` | `anvil/src/shell/` |
| Protocol handlers | `src/wayland/handlers/` | `src/handlers/` | `anvil/src/handlers/` |
| Input, gestures, shortcuts | `src/input/` | `src/input/` | `anvil/src/input_handler.rs` |
| XWayland | `src/xwayland.rs` | `src/utils/xwayland/` | `anvil/src/` (xwayland) |
| Config | `src/config/` | `niri-config/` | n/a |

## Ready queries
```bash
# DRM + vblank pacing
rg -n "DrmCompositor|VBlank|frame_submitted|queue_frame" ~/ref/niri/src ~/ref/smithay/anvil/src
# Decorations
rg -n "XdgDecorationHandler|decoration_mode|request_mode" ~/ref/cosmic-comp/src ~/ref/niri/src
# Damage / render
rg -n "OutputDamageTracker|render_output|damage" ~/ref/niri/src ~/ref/cosmic-comp/src/backend/render
# Animations / springs
rg -n "struct Spring|Animation::|fn value|clock" ~/ref/niri/src/animation
# Clipboard / DnD
rg -n "DataDeviceHandler|SelectionHandler|PrimarySelection" ~/ref/cosmic-comp/src ~/ref/niri/src
# Fractional scale / viewporter / dmabuf
rg -n "FractionalScale|ViewporterState|DmabufHandler" ~/ref/cosmic-comp/src ~/ref/niri/src
# XWayland
rg -n "XWayland|X11Wm" ~/ref/cosmic-comp/src ~/ref/niri/src
# Session lock, idle inhibit, activation
rg -n "SessionLock|IdleInhibit|XdgActivation" ~/ref/cosmic-comp/src ~/ref/niri/src
```

## Study task template
> Use skill aqua. In `~/ref/<repo>` find how <topic> works (use the ready
> queries, read only matched ranges). Explain the design in <= 15 lines with
> `path:line` pointers, then list what Aqua must change (file names only).
> Do not copy code. Do not read files whole.

## Implementation task template (after study)
> Use skill aqua. Implement <topic> in Aqua per the summary above, in Aqua's
> own style. Touch only the files listed. `cargo check` after edits.
