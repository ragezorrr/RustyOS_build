use tiny_skia::{
    FillRule, LineCap, Paint, PathBuilder, Pixmap, Stroke, Transform as SkTransform,
};

/// Squircle (Apple continuous corner superellipse curvature, n=5).
/// Generates true G2 continuous curvature without pointed or diamond-like artifacts.
pub fn squircle_path(x: f32, y: f32, w: f32, h: f32, r: f32) -> tiny_skia::Path {
    let a = (r * 1.5286).min(w * 0.5).min(h * 0.5).max(0.0);
    if a <= 0.001 {
        let mut pb = PathBuilder::new();
        pb.move_to(x, y);
        pb.line_to(x + w, y);
        pb.line_to(x + w, y + h);
        pb.line_to(x, y + h);
        pb.close();
        return pb.finish().expect("rect path");
    }

    let e = 2.0_f32 / 5.0;
    let seg = 24;
    let hp = std::f32::consts::FRAC_PI_2;
    let corners = [
        (x + w - a, y + a, 1.0_f32, -1.0_f32, true),
        (x + w - a, y + h - a, 1.0_f32, 1.0_f32, false),
        (x + a, y + h - a, -1.0_f32, 1.0_f32, true),
        (x + a, y + a, -1.0_f32, -1.0_f32, false),
    ];

    let mut pb = PathBuilder::new();
    let mut first = true;
    for (cx, cy, sx, sy, rev) in corners {
        for i in 0..=seg {
            let mut t = hp * (i as f32 / seg as f32);
            if rev {
                t = hp - t;
            }
            let px = cx + sx * a * t.cos().max(0.0).powf(e);
            let py = cy + sy * a * t.sin().max(0.0).powf(e);
            if first {
                pb.move_to(px, py);
                first = false;
            } else {
                pb.line_to(px, py);
            }
        }
    }
    pb.close();
    pb.finish().unwrap_or_else(|| {
        let mut fallback = PathBuilder::new();
        fallback.push_rect(
            tiny_skia::Rect::from_xywh(x, y, w, h)
                .unwrap_or(tiny_skia::Rect::from_xywh(0.0, 0.0, 1.0, 1.0).unwrap()),
        );
        fallback.finish().expect("fallback rect path")
    })
}

pub fn squircle_rect(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, r: f32, color: [u8; 4]) {
    let path = squircle_path(x, y, w, h, r);
    let mut paint = Paint::default();
    paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
    pm.fill_path(&path, &paint, FillRule::Winding, SkTransform::identity(), None);
}

pub fn stroke_squircle(
    pm: &mut Pixmap,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    r: f32,
    color: [u8; 4],
    width: f32,
) {
    let path = squircle_path(x, y, w, h, r);
    let mut paint = Paint::default();
    paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
    let stroke = Stroke {
        width,
        line_cap: LineCap::Round,
        ..Default::default()
    };
    pm.stroke_path(&path, &paint, &stroke, SkTransform::identity(), None);
}

pub fn fill_rect(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, color: [u8; 4]) {
    let mut paint = Paint::default();
    paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
    if let Some(r) = tiny_skia::Rect::from_xywh(x, y, w, h) {
        pm.fill_rect(r, &paint, SkTransform::identity(), None);
    }
}

pub fn rounded_rect(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, r: f32, color: [u8; 4]) {
    squircle_rect(pm, x, y, w, h, r, color);
}

pub fn stroke_rounded(
    pm: &mut Pixmap,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    r: f32,
    color: [u8; 4],
    width: f32,
) {
    stroke_squircle(pm, x, y, w, h, r, color, width);
}

pub fn fill_rounded_rect(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, r: f32, color: [u8; 4]) {
    rounded_rect(pm, x, y, w, h, r, color);
}

pub fn fill_circle(pm: &mut Pixmap, cx: f32, cy: f32, r: f32, color: [u8; 4]) {
    let mut pb = PathBuilder::new();
    pb.push_circle(cx, cy, r);
    if let Some(p) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
        pm.fill_path(&p, &paint, FillRule::Winding, SkTransform::identity(), None);
    }
}

pub fn stroke_circle(pm: &mut Pixmap, cx: f32, cy: f32, r: f32, color: [u8; 4], width: f32) {
    let mut pb = PathBuilder::new();
    pb.push_circle(cx, cy, r);
    if let Some(p) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
        let stroke = Stroke {
            width,
            ..Default::default()
        };
        pm.stroke_path(&p, &paint, &stroke, SkTransform::identity(), None);
    }
}

pub fn blob(pm: &mut Pixmap, cx: f32, cy: f32, r: f32, color: [u8; 4], alpha: u8) {
    // Layered circles fake a soft radial falloff without needing a blur filter.
    let steps = 14;
    for i in (0..steps).rev() {
        let t = i as f32 / steps as f32;
        let rr = r * (0.35 + 0.65 * t);
        let a = (alpha as f32 * (1.0 - t) * 0.35).max(1.0) as u8;
        fill_circle(pm, cx, cy, rr, [color[0], color[1], color[2], a]);
    }
}

pub fn draw_arc(
    pm: &mut Pixmap,
    cx: f32,
    cy: f32,
    r: f32,
    start_deg: f32,
    end_deg: f32,
    color: [u8; 4],
    width: f32,
) {
    let mut pb = PathBuilder::new();
    let steps = 18;
    let start_rad = start_deg.to_radians();
    let end_rad = end_deg.to_radians();
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let rad = start_rad + (end_rad - start_rad) * t;
        let x = cx + r * rad.cos();
        let y = cy + r * rad.sin();
        if i == 0 {
            pb.move_to(x, y);
        } else {
            pb.line_to(x, y);
        }
    }
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
        let stroke = Stroke {
            width,
            line_cap: LineCap::Round,
            line_join: tiny_skia::LineJoin::Round,
            ..Default::default()
        };
        pm.stroke_path(&path, &paint, &stroke, SkTransform::identity(), None);
    }
}
