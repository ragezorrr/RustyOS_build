# Aqua design guide: macOS 26 "Tahoe" (Liquid Glass), dark

Target = real macOS screenshot: ribbon wallpaper, transparent menu bar, clear
glass dock with glowing rim, detailed icons, capsule buttons, large radii.
Read for any UI/visual task. Numbers are targets; code (`theme.rs`, constants
in `ui.rs`/`render.rs`) is the source of truth. Change values in code only via
`theme.rs`, never inline.

## Gap analysis: what makes it look "not macOS"
1. Near-black flat wallpaper -> glass has nothing to show. FIX FIRST.
2. Dock plate dark/opaque with uniform grey border. Needs clear glass + gradient rim.
3. Flat hand-drawn icons. Need gradient, top highlight, inner shadow, drop shadow.
4. Corners look like a rotated square (diamond). Broken squircle, see "Shapes".
5. Small radii and plain rects. Tahoe uses big radii and capsules.
6. Uniform grey strokes. Use light gradient rims (bright top/bottom, faint sides).

## Principles
- Liquid Glass: clear, refractive-looking, bright rim, soft wide shadow.
- Concentric radii: inner radius = outer radius - padding.
- Restraint: neutral glass; color only in icons, accent buttons, wallpaper.
- Physical motion: springs, interruptible, time-based, on cached textures.
- SF Pro Text; 13pt UI, 11pt secondary; titles semibold/bold.

## Shapes (fix the "diamond" bug)
- Icons: squircle, corner ~22.5% of size (Tahoe up to ~26%).
- Never build corners from badly placed cubic handles: a wrong handle makes a
  pointed, diamond-like corner. Prefer superellipse sampling (n=5):

```rust
pub fn squircle_path(x: f32, y: f32, w: f32, h: f32, r: f32) -> tiny_skia::Path {
    let a = (r * 1.5286).min(w * 0.5).min(h * 0.5).max(0.0);
    let e = 2.0_f32 / 5.0;
    let seg = 24;
    let hp = std::f32::consts::FRAC_PI_2;
    let corners = [
        (x + w - a, y + a,     1.0, -1.0, true),
        (x + w - a, y + h - a, 1.0,  1.0, false),
        (x + a,     y + h - a, -1.0, 1.0, true),
        (x + a,     y + a,     -1.0, -1.0, false),
    ];
    let mut pb = PathBuilder::new();
    let mut first = true;
    for (cx, cy, sx, sy, rev) in corners {
        for i in 0..=seg {
            let mut t = hp * i as f32 / seg as f32;
            if rev { t = hp - t; }
            let px = cx + sx * a * t.cos().powf(e);
            let py = cy + sy * a * t.sin().powf(e);
            if first { pb.move_to(px, py); first = false } else { pb.line_to(px, py) }
        }
    }
    pb.close();
    pb.finish().unwrap()
}
```
- All shapes (icons, dock, windows, dialogs, tooltips) go through ONE function
  (`squircle_path` in `ui.rs`). Cache masks per size; do not rebuild per frame.
- Radii: window ~26-28 (Tahoe), dialog ~28, dock plate ~0.32-0.4 of its height,
  tooltip ~10, capsule = height/2.
- Buttons are capsules. Primary = accent blue (~`[10,132,255]`), secondary =
  translucent dark `[255,255,255,~22]`. Height ~32, label 13pt.

## Liquid Glass recipe (CPU, cached)
Wallpaper is static: blur it ONCE (downscale 1/4, kawase/box, upscale) and
cache. Under any glass surface draw that crop, then:
1. pre-blurred wallpaper crop, saturation +20-40%, slight brightness lift
2. very light tint (dark: white alpha ~6-12 over darkened crop; NOT opaque grey)
3. inner top highlight: 1-2px light gradient fading downward
4. rim: 1px gradient stroke, alpha ~90 at top/bottom-diagonal, ~20 at sides
5. outer shadow: wide, soft, 3-4 layers, low alpha (also a faint colored glow
   is fine on purple wallpaper)
Shell surfaces (dock, menu highlights, dialogs, tooltips, control center) use
this. Live windows are not blurred behind; do not try per-frame blur on CPU.

## Wallpaper (highest impact)
Reference: deep indigo/navy field with large flowing ribbon curves.
- Base: vertical/diagonal gradient, dark navy `[8,6,26]` to `[20,16,52]`.
- Ribbons: 2-3 wide S-curves filled with gradient from violet
  `[90,80,170]` (lit edge) to dark navy; each has a thin bright rim line
  (`[170,160,255,~140]`) along the lit edge plus a soft glow.
- Right side: cool blue glow `[70,110,190]`.
- Luminance range must be large (bright ribbon edges vs near-black gaps).
- Add subtle noise/dither to avoid banding. Render once at output size, cache
  as MemoryRenderBuffer; regenerate only on output resize/theme change.

## Menu bar
- Fully transparent in dark mode, no separator needed over this wallpaper.
- Left: Apple logo, active app name Bold, then Regular items, ~14-16px spacing.
- Right: Wi-Fi, Search, Control Center icons, then date+time ("Wed Apr 1 9:41 AM"),
  evenly spaced; text 13pt Regular, near-white `[242,242,247]`.
- Hover/open menu item: capsule highlight, white alpha ~20.

## Dock
- Clear glass plate (recipe above), radius large, padding even.
- Icons ~ 48 default (see `ICON`), spacing ~ icon*0.18-0.28.
- Separator thin vertical line before folders/trash.
- Running indicator: small dot centered under icon, below the icon, inside plate.
- Magnification: gaussian/cosine bump over 2-3 neighbors, spring-smoothed;
  scale the cached icon texture, do not re-rasterize.
- Tooltip: glass capsule/squircle above icon. Launch bounce: decaying spring.

## App icons (detailed, not flat)
Every icon is: squircle mask + vertical gradient (lighter top) + top-third
soft highlight + inner shadow at bottom edge + glyph with its own subtle
shadow + drop shadow under the whole icon. Draw once at 2x, cache, scale.
Prefer SVG paths (resvg/usvg) for glyphs over hand-written bezier code.
Finder: two-tone blue face; Safari: compass on white/blue; Terminal: dark
gradient with `>_`; Notes: yellow header + lines; Music: red-pink gradient
with note; Settings: grey gear with metallic gradient.

## Windows (SSD)
- Radius ~26-28 (Tahoe), squircle, rim 1px gradient, large soft shadow
  (stronger when focused). Titlebar translucent, no hard divider.
- Traffic lights: 12-13px circles, ~8px gap, left inset ~14-16px, vertically
  centered; glyphs (x - +) appear on group hover; inactive = grey.
- Title centered, 13pt Bold, dimmed when inactive.
- CSD clients: draw nothing extra (see `windows-ssd-csd.md`).

## Dialogs / alerts (e.g. "Save this message as a draft?")
~260 wide, radius ~28, glass recipe with slightly stronger tint; padding ~22.
Title 13pt Bold, body 11pt Regular dimmed (`[200,200,210]`), left-aligned.
Two capsule buttons side by side, equal width, 32 high, 10 gap, bottom padding
~14; default action = accent blue, other = translucent grey. Centered on screen,
open animation scale 0.96->1 + fade.

## Motion
| Animation | Curve |
|---|---|
| open | scale 0.96->1 + fade, spring/ease-out ~200-250ms |
| close | scale 1->0.97 + fade, ease-in ~150ms |
| move/tile/maximize | critically damped spring, interruptible |
| minimize | scale + translate toward dock icon |
| dock magnify / bounce | spring |
Animate cached textures only (transform/alpha). Linear only for plain opacity.

## Colors
All in `theme.rs`, dark and light variants each. Light mode: white glass alpha
~150, near-black text, peach-to-sky wallpaper. Never hardcode elsewhere.

## Redesign order (when asked to "make it look like macOS")
1. squircle fix  2. wallpaper  3. glass dock  4. icons  5. menu bar
6. window radius/rim/shadow  7. dialogs + capsule buttons  8. motion
Verify visually after each step; do not batch all eight.
