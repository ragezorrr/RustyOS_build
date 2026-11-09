# Aqua Compositor — Project Rules

## Language & Stack
- 100% Rust — no C, C++, Python, or external GUI toolkits (GTK/Qt)
- All UI rasterized with `tiny-skia` on CPU
- Wayland protocol via `smithay`
- Fonts embedded via `include_bytes!()` from `assets/fonts/`

## Code Style
- Follow the existing module structure (see SKILL.md for architecture)
- All colors defined in `src/theme.rs` — never hardcode RGBA elsewhere
- All geometry constants (`PANEL_H`, `DOCK_MARGIN`, etc.) at the top of `src/ui.rs`
- Keep hit-test logic in `rebuild_hits()`, not scattered in input handlers

## Visual Standards
- Match macOS Sequoia Sequoia dark/light appearance exactly
- Menu bar: fully transparent in dark mode
- Dock: `DOCK_MARGIN = 4px` from bottom (flush with edge, macOS style)
- All new overlays follow the glass panel pattern (see `control_center.rs`)
- Animations: open 200ms ease-out cubic, close 160ms linear

## Safety
- Never panic — use `.unwrap_or`, `.unwrap_or_default()`, or propagate errors
- Borrow checker: always `.cloned()` before calling mutable methods on `space.elements()`
- Avoid holding immutable borrows of `state.space` while also calling mutable state methods

## Build Discipline
- Always `cargo check` before `cargo build --release`
- Fix all `error[...]` before submitting — warnings are acceptable
- Release binary at: `/home/anc/aqua/target/release/aqua`
