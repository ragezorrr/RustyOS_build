# SSD / CSD in Aqua (Smithay)

Read only when working on decorations, move/resize, or window states.

## Negotiation (xdg-decoration)

```rust
use smithay::reexports::wayland_protocols::xdg::decoration::zv1::server::
    zxdg_toplevel_decoration_v1::Mode;

impl XdgDecorationHandler for Aqua {
    fn new_decoration(&mut self, tl: ToplevelSurface) {
        set_mode(&tl, Mode::ServerSide);           // default SSD
    }
    fn request_mode(&mut self, tl: ToplevelSurface, mode: Mode) {
        set_mode(&tl, mode);                        // honor client choice
    }
    fn unset_mode(&mut self, tl: ToplevelSurface) {
        set_mode(&tl, Mode::ServerSide);
    }
}
fn set_mode(tl: &ToplevelSurface, mode: Mode) {
    tl.with_pending_state(|s| s.decoration_mode = Some(mode));
    if tl.is_initial_configure_sent() { tl.send_configure(); }
}
```
Register `XdgDecorationState::new::<Aqua>(&dh)` and `delegate_xdg_decoration!`.
Clients without the protocol are CSD; nothing can change that. Legacy KDE
clients: `KdeDecorationState` (optional).

Per-window record the negotiated mode (e.g. in `DecoCache`) and branch on it
in `render.rs` and in input hit-testing.

## SSD

- Draw titlebar, border, traffic lights, shadow in the compositor.
- Client surface offset = `(0, TITLEBAR_H)`; configure size = outer size minus
  titlebar/borders.
- Hit-test order: traffic lights, titlebar (move; double-click = zoom), edges
  and corners (resize), client.
- Fullscreen: no decorations, no shadow.

## CSD

- Do not draw titlebar/border. Shadow is already inside the client buffer.
- Use `Window::geometry()` (the visible window rect) for layout, snapping,
  maximize, dock overlap. Buffer bbox includes shadow and will cause drift.
- The shadow area is part of the client input region; do not steal pointer
  events there (client uses it for resize via `xdg_toplevel.resize`).
- Handle client requests: `move_request`, `resize_request` (start grab),
  `show_window_menu`, `set_minimized`, `set_maximized`/`unset_maximized`,
  `set_fullscreen`/`unset_fullscreen`.
- Optionally apply a compositor-side rounded-corner mask for visual
  consistency; only if it does not hurt performance.

## Window states (both modes)

On maximize/tile/fullscreen set via `with_pending_state` then `send_configure`:
`State::Maximized`, `State::Fullscreen`, `State::TiledLeft/Right/Top/Bottom`,
`State::Activated` (focused). CSD clients use these to drop shadows and round
corners. Always save/restore the pre-maximize geometry (`maximized` map).

## Common bugs

- Double border: SSD forced on a CSD client, or mode not re-sent after change.
- Window jumps on maximize: layout used buffer bbox instead of `geometry()`.
- Cannot resize CSD window: compositor hit-test covers the shadow region.
- Stale size after mode switch: missing `send_configure()` or wrong decoration
  size subtraction in configure.
- Focus look wrong: `Activated` not set/cleared on focus change.
