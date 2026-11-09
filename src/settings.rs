//! System settings window for Aqua (Rust / tiny-skia UI).

use smithay::backend::{
    allocator::Fourcc,
    renderer::element::memory::MemoryRenderBuffer,
};
use smithay::utils::Transform;
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Transform as SkTransform};

use crate::text::TextRenderer;
use crate::theme::{Appearance, Theme};

pub const SETTINGS_W: i32 = 680;
pub const SETTINGS_H: i32 = 460;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    Dock,
    Appearance,
    About,
}

#[derive(Debug, Clone)]
pub struct DockConfig {
    pub icon_size: i32,        // 36 .. 64 (default 48)
    pub magnification: bool,   // on / off
    pub max_scale: f32,        // 1.1 .. 1.6 (default 1.32)
    pub autohide: bool,        // on / off (future)
    pub show_indicators: bool, // on / off
}

impl Default for DockConfig {
    fn default() -> Self {
        Self {
            icon_size: 48,
            magnification: true,
            max_scale: 1.32,
            autohide: false,
            show_indicators: true,
        }
    }
}

pub struct SettingsWindow {
    pub open: bool,
    pub current_tab: SettingsTab,
    pub dock: DockConfig,
    pub loc: Option<(i32, i32)>,
}

impl Default for SettingsWindow {
    fn default() -> Self {
        Self {
            open: false,
            current_tab: SettingsTab::Dock,
            dock: DockConfig::default(),
            loc: None,
        }
    }
}

impl SettingsWindow {
    pub fn toggle(&mut self) {
        self.open = !self.open;
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn handle_click(&mut self, local_x: i32, local_y: i32, theme: &mut Theme) -> bool {
        // Tab sidebar clicks (x: 12..180, y: 50..)
        if local_x >= 12 && local_x < 180 {
            if local_y >= 54 && local_y < 86 {
                self.current_tab = SettingsTab::Dock;
                return true;
            } else if local_y >= 88 && local_y < 120 {
                self.current_tab = SettingsTab::Appearance;
                return true;
            } else if local_y >= 122 && local_y < 154 {
                self.current_tab = SettingsTab::About;
                return true;
            }
        }

        // Tab Content clicks
        let cx = local_x - 200;
        let cy = local_y - 50;

        match self.current_tab {
            SettingsTab::Dock => {
                // 1. Icon size slider (cx: 20..380, cy: 30..54)
                if cx >= 20 && cx < 380 && cy >= 30 && cy < 56 {
                    let ratio = (cx - 20) as f32 / 360.0;
                    self.dock.icon_size = (36.0 + ratio * 28.0).round() as i32;
                    return true;
                }
                // 2. Magnification toggle (cx: 20..70, cy: 90..116)
                if cx >= 20 && cx < 380 && cy >= 90 && cy < 120 {
                    self.dock.magnification = !self.dock.magnification;
                    return true;
                }
                // 3. Magnification scale slider (cx: 20..380, cy: 150..176)
                if cx >= 20 && cx < 380 && cy >= 150 && cy < 176 {
                    let ratio = (cx - 20) as f32 / 360.0;
                    self.dock.max_scale = 1.1 + ratio * 0.5;
                    return true;
                }
                // 4. Show indicators toggle
                if cx >= 20 && cx < 380 && cy >= 210 && cy < 240 {
                    self.dock.show_indicators = !self.dock.show_indicators;
                    return true;
                }
            }
            SettingsTab::Appearance => {
                // Dark mode card (cx: 20..140, cy: 30..110)
                if cx >= 20 && cx < 140 && cy >= 30 && cy < 110 {
                    theme.appearance = Appearance::Dark;
                    theme.save();
                    return true;
                }
                // Light mode card (cx: 160..280, cy: 30..110)
                if cx >= 160 && cx < 280 && cy >= 30 && cy < 110 {
                    theme.appearance = Appearance::Light;
                    theme.save();
                    return true;
                }
            }
            SettingsTab::About => {}
        }

        false
    }
}

pub fn render_settings(
    settings: &SettingsWindow,
    theme: &Theme,
    text: &TextRenderer,
) -> Option<MemoryRenderBuffer> {
    if !settings.open {
        return None;
    }

    let w = SETTINGS_W as u32;
    let h = SETTINGS_H as u32;
    let mut pm = Pixmap::new(w, h)?;

    let dark = theme.dark();
    let bg = if dark { [30u8, 30, 36, 245] } else { [246u8, 246, 250, 245] };
    let sidebar_bg = if dark { [22u8, 22, 28, 245] } else { [236u8, 236, 242, 245] };
    let border = if dark { [255u8, 255, 255, 30] } else { [0u8, 0, 0, 35] };
    let fg = theme.menubar_fg();
    let fg_dim = theme.menubar_fg_dim();

    // Drop shadow
    for i in (0..6).rev() {
        let pad = (i + 1) as f32 * 2.5;
        draw_round_rect(&mut pm, 0.5 - pad, 0.5 - pad + 4.0, w as f32 - 1.0 + pad * 2.0, h as f32 - 1.0 + pad * 2.0, 16.0, [0, 0, 0, (14 - i * 2) as u8]);
    }

    // Main window plate
    draw_round_rect(&mut pm, 0.5, 0.5, w as f32 - 1.0, h as f32 - 1.0, 14.0, bg);
    stroke_round_rect(&mut pm, 0.5, 0.5, w as f32 - 1.0, h as f32 - 1.0, 14.0, border, 1.0);

    // Sidebar background (left column)
    draw_round_rect(&mut pm, 0.5, 0.5, 186.0, h as f32 - 1.0, 14.0, sidebar_bg);
    fill_rect(&mut pm, 186.0, 0.0, 1.0, h as f32, if dark { [255, 255, 255, 18] } else { [0, 0, 0, 25] });

    // Window control buttons (traffic lights fake inside settings header)
    draw_circle(&mut pm, 20.0, 22.0, 6.0, [255, 95, 87, 255]);
    draw_circle(&mut pm, 38.0, 22.0, 6.0, [254, 188, 46, 255]);
    draw_circle(&mut pm, 56.0, 22.0, 6.0, [40, 200, 64, 255]);

    // Sidebar items
    let tabs = [
        (SettingsTab::Dock, "Dock & Меню"),
        (SettingsTab::Appearance, "Оформление"),
        (SettingsTab::About, "О системе Aqua"),
    ];

    let mut tab_y = 54.0;
    for (tab, label) in tabs {
        let is_sel = settings.current_tab == tab;
        if is_sel {
            draw_round_rect(&mut pm, 10.0, tab_y, 166.0, 30.0, 7.0, theme.accent);
        }
        let col = if is_sel { [255, 255, 255, 255] } else { fg };
        text.draw_bold(&mut pm, label, 24.0, tab_y + 19.5, 12.5, col);
        tab_y += 34.0;
    }

    // Content area (x: 200 .. 660, y: 30 .. 440)
    let cx = 210.0;
    let cy = 34.0;

    match settings.current_tab {
        SettingsTab::Dock => {
            text.draw_bold(&mut pm, "Настройки панели Dock", cx, cy + 18.0, 18.0, fg);

            // 1. Icon Size Slider
            text.draw_bold(&mut pm, "Размер значков", cx, cy + 60.0, 13.0, fg);
            let size_str = format!("{} px", settings.dock.icon_size);
            text.draw(&mut pm, &size_str, cx + 320.0, cy + 60.0, 12.0, fg_dim);

            let bar_y = cy + 70.0;
            let bar_w = 360.0;
            draw_round_rect(&mut pm, cx, bar_y, bar_w, 8.0, 4.0, if dark { [60, 60, 70, 255] } else { [220, 220, 230, 255] });
            let ratio = (settings.dock.icon_size - 36) as f32 / 28.0;
            let fill_w = bar_w * ratio.clamp(0.0, 1.0);
            draw_round_rect(&mut pm, cx, bar_y, fill_w, 8.0, 4.0, theme.accent);
            fill_circle(&mut pm, cx + fill_w, bar_y + 4.0, 8.0, [255, 255, 255, 255]);

            // 2. Magnification Checkbox
            let mag_y = cy + 110.0;
            text.draw_bold(&mut pm, "Увеличение (Magnification)", cx, mag_y + 16.0, 13.0, fg);
            draw_toggle(&mut pm, cx + 310.0, mag_y, settings.dock.magnification, theme.accent);

            // 3. Magnification Scale
            if settings.dock.magnification {
                text.draw_bold(&mut pm, "Коэффициент увеличения", cx, cy + 160.0, 13.0, fg);
                let scale_str = format!("{:.0}%", settings.dock.max_scale * 100.0);
                text.draw(&mut pm, &scale_str, cx + 320.0, cy + 160.0, 12.0, fg_dim);

                let scale_bar_y = cy + 170.0;
                draw_round_rect(&mut pm, cx, scale_bar_y, bar_w, 8.0, 4.0, if dark { [60, 60, 70, 255] } else { [220, 220, 230, 255] });
                let s_ratio = (settings.dock.max_scale - 1.1) / 0.5;
                let s_fill_w = bar_w * s_ratio.clamp(0.0, 1.0);
                draw_round_rect(&mut pm, cx, scale_bar_y, s_fill_w, 8.0, 4.0, theme.accent);
                fill_circle(&mut pm, cx + s_fill_w, scale_bar_y + 4.0, 8.0, [255, 255, 255, 255]);
            }

            // 4. Indicator dots
            let ind_y = cy + 220.0;
            text.draw_bold(&mut pm, "Индикаторы открытых программ", cx, ind_y + 16.0, 13.0, fg);
            draw_toggle(&mut pm, cx + 310.0, ind_y, settings.dock.show_indicators, theme.accent);
        }
        SettingsTab::Appearance => {
            text.draw_bold(&mut pm, "Оформление интерфейса", cx, cy + 18.0, 18.0, fg);

            // Dark card
            let card_w = 120.0;
            let card_h = 80.0;
            let dark_card_x = cx;
            let card_y = cy + 50.0;
            let is_dark = theme.dark();

            draw_round_rect(&mut pm, dark_card_x, card_y, card_w, card_h, 10.0, [24, 24, 30, 255]);
            if is_dark {
                stroke_round_rect(&mut pm, dark_card_x, card_y, card_w, card_h, 10.0, theme.accent, 2.5);
            }
            text.draw_bold(&mut pm, "Темная", dark_card_x + 36.0, card_y + 45.0, 13.0, [240, 240, 245, 255]);

            // Light card
            let light_card_x = cx + 140.0;
            draw_round_rect(&mut pm, light_card_x, card_y, card_w, card_h, 10.0, [240, 240, 246, 255]);
            if !is_dark {
                stroke_round_rect(&mut pm, light_card_x, card_y, card_w, card_h, 10.0, theme.accent, 2.5);
            }
            text.draw_bold(&mut pm, "Светлая", light_card_x + 34.0, card_y + 45.0, 13.0, [40, 40, 45, 255]);
        }
        SettingsTab::About => {
            text.draw_bold(&mut pm, "Aqua Compositor", cx, cy + 24.0, 20.0, fg);
            text.draw(&mut pm, "macOS-styled Wayland desktop environment for Linux", cx, cy + 46.0, 12.0, fg_dim);
            text.draw(&mut pm, "Версия: 0.2.0 (Release)", cx, cy + 80.0, 12.5, fg);
            text.draw(&mut pm, "Движок рендеринга: tiny-skia + Smithay + OpenGL ES", cx, cy + 104.0, 12.0, fg_dim);
            text.draw(&mut pm, "Написано на 100% Rust", cx, cy + 128.0, 12.0, theme.accent);
        }
    }

    Some(MemoryRenderBuffer::from_slice(
        pm.data(),
        Fourcc::Abgr8888,
        (w as i32, h as i32),
        1,
        Transform::Normal,
        None,
    ))
}

fn draw_toggle(pm: &mut Pixmap, x: f32, y: f32, enabled: bool, accent: [u8; 4]) {
    let w = 44.0;
    let h = 24.0;
    let bg = if enabled { accent } else { [120, 120, 130, 200] };
    draw_round_rect(pm, x, y, w, h, 12.0, bg);
    let circle_x = if enabled { x + w - 12.0 } else { x + 12.0 };
    fill_circle(pm, circle_x, y + 12.0, 9.5, [255, 255, 255, 255]);
}

fn fill_circle(pm: &mut Pixmap, cx: f32, cy: f32, r: f32, color: [u8; 4]) {
    let mut pb = PathBuilder::new();
    pb.push_circle(cx, cy, r);
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
        pm.fill_path(&path, &paint, FillRule::Winding, SkTransform::identity(), None);
    }
}

fn draw_circle(pm: &mut Pixmap, cx: f32, cy: f32, r: f32, color: [u8; 4]) {
    fill_circle(pm, cx, cy, r, color);
}

fn fill_rect(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, color: [u8; 4]) {
    if let Some(r) = tiny_skia::Rect::from_xywh(x, y, w, h) {
        let mut paint = Paint::default();
        paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
        pm.fill_rect(r, &paint, SkTransform::identity(), None);
    }
}

fn draw_round_rect(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, r: f32, color: [u8; 4]) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let r = r.min(w / 2.0).min(h / 2.0);
    let k = 0.552_284_8 * r;
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.cubic_to(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    pb.close();
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
        pm.fill_path(&path, &paint, FillRule::Winding, SkTransform::identity(), None);
    }
}

fn stroke_round_rect(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, r: f32, color: [u8; 4], width: f32) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let r = r.min(w / 2.0).min(h / 2.0);
    let k = 0.552_284_8 * r;
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.cubic_to(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    pb.close();
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
        let stroke = tiny_skia::Stroke {
            width,
            ..Default::default()
        };
        pm.stroke_path(&path, &paint, &stroke, SkTransform::identity(), None);
    }
}
