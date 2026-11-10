//! macOS menu bar rendering and system indicator glyphs.

use std::path::Path;
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform as SkTransform};
use smithay::backend::{
    allocator::Fourcc,
    renderer::element::memory::MemoryRenderBuffer,
};
use smithay::utils::Transform;

use crate::text::TextRenderer;
use crate::theme::Theme;
use super::shapes::*;
use super::PANEL_H;

pub fn read_battery() -> (u8, bool) {
    let base = Path::new("/sys/class/power_supply");
    let (mut capacity, mut charging) = (100u8, false);
    if let Ok(entries) = std::fs::read_dir(base) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !name.starts_with("BAT") {
                continue;
            }
            if let Ok(s) = std::fs::read_to_string(entry.path().join("capacity")) {
                if let Ok(v) = s.trim().parse::<u8>() {
                    capacity = v;
                }
            }
            if let Ok(s) = std::fs::read_to_string(entry.path().join("status")) {
                charging = matches!(s.trim(), "Charging" | "Full");
            }
        }
    }
    (capacity, charging)
}

pub fn clock_text(minute: i64) -> String {
    let now = chrono::Local::now();
    let weekday = match chrono::Datelike::weekday(&now) {
        chrono::Weekday::Mon => "Пн",
        chrono::Weekday::Tue => "Вт",
        chrono::Weekday::Wed => "Ср",
        chrono::Weekday::Thu => "Чт",
        chrono::Weekday::Fri => "Пт",
        chrono::Weekday::Sat => "Сб",
        chrono::Weekday::Sun => "Вс",
    };
    let month = match chrono::Datelike::month(&now) {
        1 => "янв",
        2 => "фев",
        3 => "мар",
        4 => "апр",
        5 => "мая",
        6 => "июн",
        7 => "июл",
        8 => "авг",
        9 => "сен",
        10 => "окт",
        11 => "ноя",
        12 => "дек",
        _ => "",
    };
    let day = chrono::Datelike::day(&now);
    let _ = minute;
    format!("{} {} {} {}", weekday, day, month, now.format("%H:%M"))
}

pub fn draw_apple(pm: &mut Pixmap, cx: f32, cy: f32, color: [u8; 4]) {
    let mut pb = PathBuilder::new();
    let top = cy - 6.5;
    let bot = cy + 7.0;

    pb.move_to(cx, top + 1.8);
    pb.cubic_to(cx - 3.8, top - 0.2, cx - 7.5, top + 3.0, cx - 7.5, cy + 1.0);
    pb.cubic_to(cx - 7.5, cy + 4.8, cx - 4.5, bot, cx - 1.8, bot);
    pb.cubic_to(cx - 0.8, bot, cx, bot - 0.8, cx + 0.5, bot);
    pb.cubic_to(cx + 1.5, bot, cx + 4.5, cy + 5.5, cx + 7.5, cy + 1.0);
    pb.cubic_to(cx + 5.8, cy + 0.6, cx + 4.8, cy - 1.5, cx + 5.6, cy - 3.5);
    pb.cubic_to(cx + 4.2, top + 0.5, cx + 2.0, top + 1.0, cx, top + 1.8);
    pb.close();

    if let Some(body) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
        pm.fill_path(&body, &paint, FillRule::Winding, SkTransform::identity(), None);
    }

    let mut pb = PathBuilder::new();
    let lx = cx + 0.2;
    let ly = top - 1.5;
    pb.move_to(lx, ly);
    pb.cubic_to(lx + 0.8, ly - 2.8, lx + 3.8, ly - 3.8, lx + 4.8, ly - 3.2);
    pb.cubic_to(lx + 4.0, ly - 0.4, lx + 1.0, ly + 0.6, lx, ly);
    pb.close();

    if let Some(leaf) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
        pm.fill_path(&leaf, &paint, FillRule::Winding, SkTransform::identity(), None);
    }
}

pub fn draw_control_center(pm: &mut Pixmap, cx: f32, cy: f32, color: [u8; 4]) {
    let w = 14.5;
    let h = 5.4;
    let r = 2.7;

    let y1 = cy - 3.8;
    stroke_rounded(pm, cx - w / 2.0, y1 - h / 2.0, w, h, r, color, 1.2);
    fill_circle(pm, cx + 2.7, y1, 1.7, color);

    let y2 = cy + 3.8;
    stroke_rounded(pm, cx - w / 2.0, y2 - h / 2.0, w, h, r, color, 1.2);
    fill_circle(pm, cx - 2.7, y2, 1.7, color);
}

pub fn draw_battery(
    pm: &mut Pixmap,
    x: f32,
    y: f32,
    level: u8,
    charging: bool,
    color: [u8; 4],
) {
    stroke_rounded(pm, x, y, 20.0, 10.5, 2.6, color, 1.2);
    fill_rounded_rect(pm, x + 20.0, y + 3.5, 1.8, 3.5, 0.9, color);

    let inner_w = 16.0;
    let fill_w = ((level.min(100) as f32 / 100.0) * inner_w).max(1.0);
    let fill_color = if charging {
        [52, 199, 89, 255]
    } else if level <= 20 {
        [255, 69, 58, 255]
    } else {
        [242, 242, 247, 220]
    };
    fill_rounded_rect(pm, x + 2.0, y + 2.0, fill_w, 6.5, 1.4, fill_color);
}

pub fn draw_wifi(pm: &mut Pixmap, cx: f32, cy: f32, color: [u8; 4]) {
    let base_y = cy + 3.6;
    fill_circle(pm, cx, base_y, 1.3, color);
    draw_arc(pm, cx, base_y, 5.0, -132.0, -48.0, color, 1.5);
    draw_arc(pm, cx, base_y, 9.2, -135.0, -45.0, color, 1.5);
}

pub fn draw_ethernet(pm: &mut Pixmap, cx: f32, cy: f32, color: [u8; 4]) {
    let mut paint = Paint::default();
    paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
    let stroke = Stroke {
        width: 1.5,
        line_cap: tiny_skia::LineCap::Round,
        line_join: tiny_skia::LineJoin::Round,
        ..Default::default()
    };

    let mut pb_l = PathBuilder::new();
    pb_l.move_to(cx - 5.0, cy - 3.4);
    pb_l.line_to(cx - 8.0, cy);
    pb_l.line_to(cx - 5.0, cy + 3.4);
    if let Some(p) = pb_l.finish() {
        pm.stroke_path(&p, &paint, &stroke, SkTransform::identity(), None);
    }

    fill_circle(pm, cx - 2.5, cy, 1.1, color);
    fill_circle(pm, cx, cy, 1.1, color);
    fill_circle(pm, cx + 2.5, cy, 1.1, color);

    let mut pb_r = PathBuilder::new();
    pb_r.move_to(cx + 5.0, cy - 3.4);
    pb_r.line_to(cx + 8.0, cy);
    pb_r.line_to(cx + 5.0, cy + 3.4);
    if let Some(p) = pb_r.finish() {
        pm.stroke_path(&p, &paint, &stroke, SkTransform::identity(), None);
    }
}

pub fn draw_spotlight_icon(pm: &mut Pixmap, cx: f32, cy: f32, color: [u8; 4]) {
    stroke_circle(pm, cx - 1.2, cy - 1.2, 4.4, color, 1.5);
    let mut pb = PathBuilder::new();
    pb.move_to(cx + 2.0, cy + 2.0);
    pb.line_to(cx + 6.0, cy + 6.0);
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
        let stroke = tiny_skia::Stroke {
            width: 1.6,
            line_cap: tiny_skia::LineCap::Round,
            ..Default::default()
        };
        pm.stroke_path(&path, &paint, &stroke, SkTransform::identity(), None);
    }
}

pub fn build_panel(
    theme: &Theme,
    text: &TextRenderer,
    battery: u8,
    charging: bool,
    width: i32,
    app_name: &str,
    app_category: crate::spotlight::AppCategory,
    minute: i64,
    network: crate::wifi::NetworkKind,
    layout_name: &str,
) -> MemoryRenderBuffer {
    let w = width.max(1) as u32;
    let h = PANEL_H as u32;
    let mut pm = Pixmap::new(w, h).expect("panel pixmap");
    let fg = theme.menubar_fg();
    let dim = theme.menubar_fg_dim();

    if !theme.dark() {
        fill_rect(&mut pm, 0.0, 0.0, w as f32, h as f32, [255u8, 255, 255, 110]);
        fill_rect(&mut pm, 0.0, h as f32 - 1.0, w as f32, 1.0, [0, 0, 0, 14]);
    }

    draw_apple(&mut pm, 18.0, 15.0, fg);

    let bold_title = if app_name.is_empty() { "Finder" } else { app_name };
    text.draw_bold(&mut pm, bold_title, 38.0, 19.5, 13.0, fg);
    let app_w = text.width_bold(bold_title, 13.0);

    if width > 760 {
        let menus = crate::spotlight::app_menu_titles(app_category);
        let mut x = 38.0 + app_w + 22.0;
        for menu in menus {
            text.draw_bold(&mut pm, menu, x, 19.5, 13.0, fg);
            x += text.width_bold(menu, 13.0) + 18.0;
        }
    }

    let clock = clock_text(minute);
    let clock_w = text.width_bold(&clock, 13.0);
    let clock_x = width as f32 - 12.0 - clock_w;
    text.draw_bold(&mut pm, &clock, clock_x, 19.5, 13.0, fg);

    let cc_x = clock_x - 22.0;
    draw_control_center(&mut pm, cc_x, 15.0, fg);

    let spot_x = cc_x - 26.0;
    draw_spotlight_icon(&mut pm, spot_x, 15.0, fg);

    let lang_code = if layout_name.to_lowercase().contains("ru") || layout_name.contains("Рус") {
        "RU"
    } else {
        "US"
    };
    let lang_w = text.width_bold(lang_code, 12.0);
    let lang_x = spot_x - 14.0 - lang_w;
    text.draw_bold(&mut pm, lang_code, lang_x, 19.5, 12.0, fg);

    let net_x = lang_x - 22.0;
    match network {
        crate::wifi::NetworkKind::Ethernet => {
            draw_ethernet(&mut pm, net_x, 15.0, fg);
        }
        crate::wifi::NetworkKind::Wifi => {
            draw_wifi(&mut pm, net_x, 15.0, fg);
        }
        crate::wifi::NetworkKind::Disconnected => {
            draw_wifi(&mut pm, net_x, 15.0, dim);
        }
    }

    let show_battery = (battery < 100 && charging) || (battery <= 95 && !charging);
    if show_battery {
        let bx = net_x - 42.0;
        draw_battery(&mut pm, bx, 10.0, battery, charging, fg);
    }

    MemoryRenderBuffer::from_slice(
        pm.data(),
        Fourcc::Abgr8888,
        (w as i32, h as i32),
        1,
        Transform::Normal,
        None,
    )
}
