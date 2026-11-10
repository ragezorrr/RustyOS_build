# Aqua design guide (macOS look)

Read only for UI/visual tasks. Numbers are targets; the source of truth is
`theme.rs` and the constants in `ui.rs`/`render.rs`. If code differs, change the
code to match this only when the user asks for a redesign.

## Principles
1. Glass, not flat fills: translucent + blurred backdrop + 1px light rim.
2. Restraint: neutral surfaces, color only for icons, traffic lights, accents.
3. Depth by layered soft shadows, not hard borders.
4. Motion is physical: springs, interruptible, time-based.
5. Typography: SF Pro Text 13pt UI, 11pt secondary, slight negative tracking.

## Glass recipe (works with CPU rendering)
The wallpaper is static, so **blur it once** (downscale 1/4, box/kawase blur,
upscale) and cache it. For dock, menu bar and overlays, draw the matching
crop of the pre-blurred wallpaper under the tint. Real per-frame blur of live
windows is not needed for the shell and is too slow on CPU.
Layers, bottom to top:
1. blurred wallpaper crop (slightly saturated, +10-20%)
2. tint fill (dark `[28,28,36,~150-170]`, light white alpha ~150)
3. inner 1px rim `[255,255,255,~35-45]`, brighter on the top edge
4. outer soft shadow, 3-4 layers, low alpha

## Shapes
- App icons: **squircle** (continuous-corner superellipse), not circle or
  plain rounded rect. Corner ~22.5% of icon size.
- Window radius ~12px (10-14 acceptable); dock plate radius ~ icon * 0.35.
- Use the same radius family everywhere; inner radius = outer minus padding.

## Menu bar
- Transparent or very light glass, hairline bottom separator.
- Active app name Bold, other items Regular, baseline-aligned.
- Right side: status icons spaced evenly, clock last. Highlight pill on
  hover/open menu (rounded, alpha ~20 white).

## Dock
- Plate hugs icons with even padding; separator line before trash/folders.
- Running indicator: small dot under icon, not on the plate edge.
- Magnification: cosine/gaussian bump over ~2-3 neighbors, spring-smoothed.
- Tooltip: small glass pill above hovered icon.
- Launch bounce: decaying spring, not sine on a timer.

## Windows (SSD)
- Titlebar height ~28-32, subtle gradient or flat dark, bottom hairline.
- Traffic lights: 12px circles, 8px gap, ~12px from left, vertically centered.
  Glyphs (x, -, +) appear on hover of the group. Inactive window = grey circles.
- Title centered, Bold 13pt, dimmed when inactive.
- Shadow larger and darker when focused, lighter when unfocused.
- CSD windows: draw nothing extra (see windows-ssd-csd.md).

## Motion
| Animation | Curve |
|---|---|
| open | scale 0.96->1 + fade, ease-out or spring (~200-250ms) |
| close | scale 1->0.97 + fade, ease-in (~150ms) |
| move/tile/maximize | critically damped spring, interruptible |
| minimize | scale/translate toward dock icon (genie-lite) |
| dock bounce | decaying spring |
All animate cached textures (transform/alpha), never re-rasterize.

## Wallpaper
Avoid near-black flat fields: glass needs color variation behind it to read
as glass. Keep some luminance range (aurora blobs brighter at the edges) and
low-frequency gradients; add subtle noise/dither to avoid banding.

## Colors
All colors live in `theme.rs`, dark + light variant each. Never hardcode
elsewhere.
