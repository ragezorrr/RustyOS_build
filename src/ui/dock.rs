//! macOS Dock rendering, icon drawing, plate caching and dock geometry.

use tiny_skia::{
    Color, FillRule, GradientStop, LineCap, LinearGradient, Paint, PathBuilder, Pixmap,
    SpreadMode, Transform as SkTransform,
};

use crate::text::TextRenderer;
use crate::theme::Theme;
use super::shapes::*;
use super::{AppDef, DynamicApp, IconKind, MinimizedItem, APPS, DOCK_MARGIN, DOCK_PAD, ICON};

pub fn draw_folder_icon(pm: &mut Pixmap, x: f32, y: f32, size: f32, _dark: bool) {
    let scale = size / ICON as f32;
    // Blue translucent macOS folder body
    rounded_rect(pm, x + 2.0 * scale, y + 8.0 * scale, 44.0 * scale, 34.0 * scale, 8.0 * scale, [45, 140, 245, 235]);
    // Folder tab
    rounded_rect(pm, x + 4.0 * scale, y + 4.0 * scale, 18.0 * scale, 10.0 * scale, 4.0 * scale, [35, 125, 230, 240]);
    // Folder front flap highlight
    rounded_rect(pm, x + 2.0 * scale, y + 16.0 * scale, 44.0 * scale, 26.0 * scale, 7.0 * scale, [70, 165, 255, 245]);
    stroke_rounded(pm, x + 2.0 * scale, y + 16.0 * scale, 44.0 * scale, 26.0 * scale, 7.0 * scale, [255, 255, 255, 80], 1.0 * scale);
}

pub fn draw_trash_icon(pm: &mut Pixmap, x: f32, y: f32, size: f32, dark: bool) {
    let scale = size / ICON as f32;
    let bg = if dark { [160, 160, 175, 140] } else { [200, 200, 215, 160] };
    let mut pb = PathBuilder::new();
    pb.move_to(x + 10.0 * scale, y + 12.0 * scale);
    pb.line_to(x + 38.0 * scale, y + 12.0 * scale);
    pb.line_to(x + 35.0 * scale, y + 42.0 * scale);
    pb.line_to(x + 13.0 * scale, y + 42.0 * scale);
    pb.close();
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color_rgba8(bg[0], bg[1], bg[2], bg[3]);
        pm.fill_path(&path, &paint, FillRule::Winding, SkTransform::identity(), None);
        let stroke = tiny_skia::Stroke {
            width: 1.5 * scale,
            ..Default::default()
        };
        let mut sp = Paint::default();
        sp.set_color_rgba8(255, 255, 255, 120);
        pm.stroke_path(&path, &sp, &stroke, SkTransform::identity(), None);
    }
    rounded_rect(pm, x + 8.0 * scale, y + 8.0 * scale, 32.0 * scale, 4.0 * scale, 2.0 * scale, [230, 230, 240, 220]);
}

pub fn draw_app_icon(text: &TextRenderer, pm: &mut Pixmap, x: f32, y: f32, size: f32, app: &AppDef) {
    let xf = x;
    let yf = y;
    let radius = (size * 0.225).round().max(4.0);

    for i in (0..4).rev() {
        let grow = (i + 1) as f32 * 1.0;
        let y_off = 1.0 + (i as f32 * 0.8);
        let alpha = match i {
            0 => 18,
            1 => 12,
            2 => 7,
            _ => 3,
        };
        squircle_rect(
            pm,
            xf - grow,
            yf - grow + y_off,
            size + grow * 2.0,
            size + grow * 2.0,
            radius + grow,
            [0, 0, 0, alpha],
        );
    }

    let shader = LinearGradient::new(
        tiny_skia::Point::from_xy(xf, yf),
        tiny_skia::Point::from_xy(xf, yf + size),
        vec![
            GradientStop::new(
                0.0,
                Color::from_rgba8(
                    app.color_a[0],
                    app.color_a[1],
                    app.color_a[2],
                    app.color_a[3],
                ),
            ),
            GradientStop::new(
                1.0,
                Color::from_rgba8(
                    app.color_b[0],
                    app.color_b[1],
                    app.color_b[2],
                    app.color_b[3],
                ),
            ),
        ],
        SpreadMode::Pad,
        SkTransform::identity(),
    )
    .expect("icon gradient");
    let mut paint = Paint::default();
    paint.shader = shader;
    let path = squircle_path(xf, yf, size, size, radius);
    pm.fill_path(&path, &paint, FillRule::Winding, SkTransform::identity(), None);

    stroke_squircle(
        pm,
        xf + 0.8,
        yf + 0.8,
        size - 1.6,
        size - 1.6,
        (radius - 0.8).max(1.0),
        [255, 255, 255, 48],
        1.0,
    );

    let scale = size / ICON as f32;

    match app.kind {
        IconKind::Terminal => {
            let glyph = ">_";
            let font_size = 19.0 * scale;
            let gw = text.width(glyph, font_size);
            text.draw(
                pm,
                glyph,
                xf + (size - gw) / 2.0,
                yf + size / 2.0 + 6.5 * scale,
                font_size,
                [255, 255, 255, 255],
            );
        }
        IconKind::Files => {
            let cx = xf + size / 2.0;
            let cy = yf + size / 2.0;
            let mut pb = PathBuilder::new();
            pb.move_to(cx, yf + 8.0 * scale);
            pb.line_to(cx, cy + 4.0 * scale);
            pb.line_to(cx + 6.0 * scale, cy + 4.0 * scale);
            if let Some(p) = pb.finish() {
                let mut paint = Paint::default();
                paint.set_color_rgba8(20, 60, 130, 230);
                let stroke = tiny_skia::Stroke { width: 2.2 * scale, line_cap: LineCap::Round, ..Default::default() };
                pm.stroke_path(&p, &paint, &stroke, SkTransform::identity(), None);
            }
            fill_circle(pm, cx - 8.0 * scale, cy - 4.0 * scale, 2.5 * scale, [20, 60, 130, 240]);
            fill_circle(pm, cx + 8.0 * scale, cy - 4.0 * scale, 2.5 * scale, [20, 60, 130, 240]);
            let mut pb = PathBuilder::new();
            pb.move_to(cx - 10.0 * scale, cy + 10.0 * scale);
            pb.cubic_to(cx - 6.0 * scale, cy + 16.0 * scale, cx + 6.0 * scale, cy + 16.0 * scale, cx + 10.0 * scale, cy + 10.0 * scale);
            if let Some(p) = pb.finish() {
                let mut paint = Paint::default();
                paint.set_color_rgba8(20, 60, 130, 230);
                let stroke = tiny_skia::Stroke { width: 2.2 * scale, line_cap: LineCap::Round, ..Default::default() };
                pm.stroke_path(&p, &paint, &stroke, SkTransform::identity(), None);
            }
        }
        IconKind::Editor => {
            fill_rect(pm, xf + 4.0 * scale, yf + 4.0 * scale, size - 8.0 * scale, 10.0 * scale, [200, 150, 20, 240]);
            for i in 0..4 {
                fill_rect(
                    pm,
                    xf + 10.0 * scale,
                    yf + (18.0 + i as f32 * 6.5) * scale,
                    size - 20.0 * scale,
                    1.4 * scale,
                    [210, 175, 50, 180],
                );
            }
        }
        IconKind::Music => {
            let cx = xf + size / 2.0;
            let cy = yf + size / 2.0;
            let mut pb = PathBuilder::new();
            pb.push_circle(cx - 6.0 * scale, cy + 8.0 * scale, 4.0 * scale);
            pb.push_circle(cx + 6.0 * scale, cy + 4.0 * scale, 4.0 * scale);
            if let Some(p) = pb.finish() {
                let mut paint = Paint::default();
                paint.set_color_rgba8(255, 255, 255, 250);
                pm.fill_path(&p, &paint, FillRule::Winding, SkTransform::identity(), None);
            }
            fill_rect(pm, cx - 3.0 * scale, cy - 8.0 * scale, 2.5 * scale, 17.0 * scale, [255, 255, 255, 250]);
            fill_rect(pm, cx + 9.0 * scale, cy - 12.0 * scale, 2.5 * scale, 17.0 * scale, [255, 255, 255, 250]);
            fill_rect(pm, cx - 3.0 * scale, cy - 11.0 * scale, 14.5 * scale, 4.0 * scale, [255, 255, 255, 250]);
        }
        IconKind::Settings => {
            let cx = xf + size / 2.0;
            let cy = yf + size / 2.0;
            stroke_circle(pm, cx, cy, 10.0 * scale, [255, 255, 255, 245], 3.5 * scale);
            fill_circle(pm, cx, cy, 4.0 * scale, [110, 115, 125, 255]);
            for i in 0..6 {
                let a = i as f32 * std::f32::consts::PI / 3.0;
                let (s, c) = a.sin_cos();
                let tx = cx + c * 13.0 * scale;
                let ty = cy + s * 13.0 * scale;
                fill_circle(pm, tx, ty, 2.5 * scale, [255, 255, 255, 245]);
            }
        }
        IconKind::Browser => {
            stroke_circle(pm, xf + size / 2.0, yf + size / 2.0, 13.0 * scale, [255, 255, 255, 250], 2.0 * scale);
            let cx = xf + size / 2.0;
            let cy = yf + size / 2.0;
            let mut pb = PathBuilder::new();
            pb.move_to(cx, cy - 9.0 * scale);
            pb.line_to(cx + 4.5 * scale, cy + 2.0 * scale);
            pb.line_to(cx - 4.5 * scale, cy + 2.0 * scale);
            pb.close();
            if let Some(p) = pb.finish() {
                let mut paint = Paint::default();
                paint.set_color_rgba8(255, 90, 100, 250);
                pm.fill_path(&p, &paint, FillRule::Winding, SkTransform::identity(), None);
            }
            let mut pb = PathBuilder::new();
            pb.move_to(cx, cy + 9.0 * scale);
            pb.line_to(cx + 4.5 * scale, cy + 2.0 * scale);
            pb.line_to(cx - 4.5 * scale, cy + 2.0 * scale);
            pb.close();
            if let Some(p) = pb.finish() {
                let mut paint = Paint::default();
                paint.set_color_rgba8(255, 255, 255, 250);
                pm.fill_path(&p, &paint, FillRule::Winding, SkTransform::identity(), None);
            }
        }
    }
}

pub fn draw_dynamic_app_icon(text: &TextRenderer, pm: &mut Pixmap, xf: f32, yf: f32, size: f32, app: &DynamicApp) {
    let radius = (size * 0.225).round().max(4.0);

    for i in (0..4).rev() {
        let grow = (i + 1) as f32 * 1.0;
        let y_off = 1.0 + (i as f32 * 0.8);
        let alpha = match i {
            0 => 18,
            1 => 12,
            2 => 7,
            _ => 3,
        };
        squircle_rect(
            pm,
            xf - grow,
            yf - grow + y_off,
            size + grow * 2.0,
            size + grow * 2.0,
            radius + grow,
            [0, 0, 0, alpha],
        );
    }

    let lower = app.app_id.to_lowercase();
    let (color_a, color_b) = if lower.contains("chrome") || lower.contains("chromium") {
        ([234u8, 67, 53, 255], [66u8, 133, 244, 255])
    } else if lower.contains("telegram") {
        ([42u8, 171, 238, 255], [30u8, 136, 229, 255])
    } else if lower.contains("code") {
        ([0u8, 122, 255, 255], [0u8, 80, 200, 255])
    } else if lower.contains("alacritty") || lower.contains("kitty") {
        ([45u8, 45, 52, 255], [20u8, 20, 25, 255])
    } else {
        let mut h: u32 = 2166136261;
        for b in app.app_id.bytes() {
            h = (h ^ (b as u32)).wrapping_mul(16777619);
        }
        let r1 = ((h & 0xFF) as u8).max(50).min(220);
        let g1 = (((h >> 8) & 0xFF) as u8).max(50).min(220);
        let b1 = (((h >> 16) & 0xFF) as u8).max(50).min(220);
        ([r1, g1, b1, 255], [r1.saturating_sub(40), g1.saturating_sub(40), b1.saturating_sub(40), 255])
    };

    let shader = LinearGradient::new(
        tiny_skia::Point::from_xy(xf, yf),
        tiny_skia::Point::from_xy(xf, yf + size),
        vec![
            GradientStop::new(0.0, Color::from_rgba8(color_a[0], color_a[1], color_a[2], color_a[3])),
            GradientStop::new(1.0, Color::from_rgba8(color_b[0], color_b[1], color_b[2], color_b[3])),
        ],
        SpreadMode::Pad,
        SkTransform::identity(),
    ).expect("icon grad");

    let mut paint = Paint::default();
    paint.shader = shader;
    let path = squircle_path(xf, yf, size, size, radius);
    pm.fill_path(&path, &paint, FillRule::Winding, SkTransform::identity(), None);

    stroke_squircle(pm, xf + 0.8, yf + 0.8, size - 1.6, size - 1.6, (radius - 0.8).max(1.0), [255, 255, 255, 48], 1.0);

    let scale = size / ICON as f32;

    if lower.contains("chrome") || lower.contains("chromium") {
        let cx = xf + size / 2.0;
        let cy = yf + size / 2.0;
        fill_circle(pm, cx, cy, 14.0 * scale, [255, 255, 255, 230]);
        fill_circle(pm, cx, cy, 8.5 * scale, [66, 133, 244, 255]);
    } else if lower.contains("telegram") {
        let cx = xf + size / 2.0;
        let cy = yf + size / 2.0;
        let mut pb = PathBuilder::new();
        pb.move_to(cx - 10.0 * scale, cy + 1.0 * scale);
        pb.line_to(cx + 11.0 * scale, cy - 8.0 * scale);
        pb.line_to(cx + 2.0 * scale, cy + 9.0 * scale);
        pb.line_to(cx - 1.0 * scale, cy + 3.0 * scale);
        if let Some(p) = pb.finish() {
            let mut paint = Paint::default();
            paint.set_color_rgba8(255, 255, 255, 255);
            pm.fill_path(&p, &paint, FillRule::Winding, SkTransform::identity(), None);
        }
    } else {
        let letter = app.name.chars().next().or_else(|| app.app_id.chars().next()).unwrap_or('A').to_uppercase().to_string();
        let font_size = 18.0 * scale;
        let lw = text.width_bold(&letter, font_size);
        text.draw_bold(
            pm,
            &letter,
            xf + (size - lw) / 2.0,
            yf + size / 2.0 + 6.5 * scale,
            font_size,
            [255, 255, 255, 255],
        );
    }
}

pub fn draw_minimized_thumbnail(theme: &Theme, pm: &mut Pixmap, xf: f32, yf: f32, size: f32, _item: &MinimizedItem) {
    let scale = size / ICON as f32;
    let w = size * 0.94;
    let h = size * 0.74;
    let rx = xf + (size - w) / 2.0;
    let ry = yf + size - h - 2.0;
    let r = 7.0 * scale;

    for i in (0..4).rev() {
        let grow = (i + 1) as f32 * 1.0;
        let y_off = 1.0 + (i as f32 * 0.8);
        let alpha = match i {
            0 => 20,
            1 => 13,
            2 => 7,
            _ => 3,
        };
        squircle_rect(pm, rx - grow, ry - grow + y_off, w + grow * 2.0, h + grow * 2.0, r + grow, [0, 0, 0, alpha]);
    }

    let body_col = if theme.dark() { [34u8, 36, 44, 245] } else { [245u8, 246, 250, 245] };
    squircle_rect(pm, rx, ry, w, h, r, body_col);
    stroke_squircle(pm, rx, ry, w, h, r, [255, 255, 255, 35], 1.0);

    let tb_h = 10.0 * scale;
    let tb_col = if theme.dark() { [28u8, 30, 38, 255] } else { [230u8, 232, 238, 255] };
    squircle_rect(pm, rx, ry, w, tb_h, r, tb_col);
    fill_rect(pm, rx, ry + tb_h - 1.0, w, 1.0, [255, 255, 255, 15]);

    fill_circle(pm, rx + 4.5 * scale, ry + 5.0 * scale, 1.6 * scale, [255, 95, 87, 255]);
    fill_circle(pm, rx + 8.5 * scale, ry + 5.0 * scale, 1.6 * scale, [254, 188, 46, 255]);
    fill_circle(pm, rx + 12.5 * scale, ry + 5.0 * scale, 1.6 * scale, [40, 200, 64, 255]);

    let line_col = if theme.dark() { [255, 255, 255, 18] } else { [0, 0, 0, 18] };
    fill_rect(pm, rx + 6.0 * scale, ry + 15.0 * scale, w - 12.0 * scale, 2.0 * scale, line_col);
    fill_rect(pm, rx + 6.0 * scale, ry + 20.0 * scale, (w - 12.0 * scale) * 0.7, 2.0 * scale, line_col);
    fill_rect(pm, rx + 6.0 * scale, ry + 25.0 * scale, (w - 12.0 * scale) * 0.5, 2.0 * scale, line_col);
}

pub fn dock_width_dynamic(icon_size: i32, dynamic_count: usize, minimized_count: usize) -> i32 {
    let gap = (icon_size as f32 * 0.28).round() as i32;
    let left_count = APPS.len() + dynamic_count;
    let apps_w = left_count as i32 * icon_size + (left_count as i32 - 1) * gap;
    let right_count = minimized_count + 2;
    let right_w = right_count as i32 * icon_size + (right_count as i32 - 1) * gap;
    DOCK_PAD * 2 + apps_w + 16 + right_w
}

pub fn dock_rect_dynamic(
    screen_w: i32,
    screen_h: i32,
    icon_size: i32,
    dynamic_count: usize,
    minimized_count: usize,
) -> (i32, i32, i32, i32) {
    let w = dock_width_dynamic(icon_size, dynamic_count, minimized_count);
    let h = icon_size + 20;
    let x = (screen_w - w) / 2;
    let y = screen_h - h - DOCK_MARGIN;
    (x, y, w, h)
}

pub fn dock_rect_with_size(screen_w: i32, screen_h: i32, icon_size: i32) -> (i32, i32, i32, i32) {
    dock_rect_dynamic(screen_w, screen_h, icon_size, 0, 0)
}

#[allow(dead_code)]
pub fn dock_rect(screen_w: i32, screen_h: i32) -> (i32, i32, i32, i32) {
    dock_rect_with_size(screen_w, screen_h, ICON)
}
