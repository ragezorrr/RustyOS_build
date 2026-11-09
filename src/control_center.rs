//! macOS Control Center and App Switcher overlays for Aqua.

use smithay::backend::{
    allocator::Fourcc,
    renderer::{
        element::{
            memory::MemoryRenderBuffer,
        },
    },
};
use smithay::utils::Transform;
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Transform as SkTransform};

use crate::text::TextRenderer;
use crate::theme::Theme;

// ---------------------------------------------------------------------------
// Control Center State
// ---------------------------------------------------------------------------

pub struct ControlCenterState {
    pub open: bool,
    pub volume: f32, // 0.0 .. 1.0
    pub brightness: f32, // 0.0 .. 1.0
    pub wifi_enabled: bool,
    pub bluetooth_enabled: bool,
    pub dragging_slider: Option<ControlSlider>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlSlider {
    Volume,
    Brightness,
}

impl Default for ControlCenterState {
    fn default() -> Self {
        Self {
            open: false,
            volume: 0.70,
            brightness: 0.85,
            wifi_enabled: true,
            bluetooth_enabled: true,
            dragging_slider: None,
        }
    }
}

impl ControlCenterState {
    pub fn toggle(&mut self) {
        self.open = !self.open;
        self.dragging_slider = None;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.dragging_slider = None;
    }
}

// ---------------------------------------------------------------------------
// App Switcher State (Super + Tab)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SwitcherItem {
    pub key: u32,
    pub title: String,
    pub app_id: Option<String>,
}

pub struct AppSwitcherState {
    pub active: bool,
    pub selected_index: usize,
    pub items: Vec<SwitcherItem>,
}

impl Default for AppSwitcherState {
    fn default() -> Self {
        Self {
            active: false,
            selected_index: 0,
            items: Vec::new(),
        }
    }
}

impl AppSwitcherState {
    pub fn open_or_next(&mut self, items: Vec<SwitcherItem>) {
        if items.is_empty() {
            self.active = false;
            return;
        }
        if !self.active {
            self.active = true;
            self.items = items;
            // Switch to next window if available
            self.selected_index = if self.items.len() > 1 { 1 } else { 0 };
        } else {
            self.selected_index = (self.selected_index + 1) % self.items.len();
        }
    }

    pub fn close(&mut self) -> Option<u32> {
        if !self.active {
            return None;
        }
        self.active = false;
        let chosen = self.items.get(self.selected_index).map(|item| item.key);
        self.items.clear();
        chosen
    }
}

// ---------------------------------------------------------------------------
// Control Center Rendering
// ---------------------------------------------------------------------------

pub const CC_W: i32 = 320;
pub const CC_H: i32 = 340;

pub fn render_control_center(
    cc: &ControlCenterState,
    theme: &Theme,
    text: &TextRenderer,
    _pointer_pos: Option<(f64, f64)>,
) -> Option<MemoryRenderBuffer> {
    if !cc.open {
        return None;
    }

    let w = CC_W as u32;
    let h = CC_H as u32;
    let mut pm = Pixmap::new(w, h)?;

    let dark = theme.dark();
    let bg = if dark {
        [24u8, 24, 30, 240]
    } else {
        [244u8, 244, 248, 242]
    };
    let tile_bg = if dark {
        [40u8, 40, 48, 255]
    } else {
        [255u8, 255, 255, 255]
    };
    let border = if dark {
        [255u8, 255, 255, 30]
    } else {
        [0u8, 0, 0, 30]
    };
    let fg = theme.menubar_fg();
    let fg_dim = theme.menubar_fg_dim();

    // Drop shadow
    for i in (0..5).rev() {
        let pad = (i + 1) as f32 * 2.0;
        draw_round_rect(&mut pm, 0.5 - pad, 0.5 - pad + 3.0, w as f32 - 1.0 + pad * 2.0, h as f32 - 1.0 + pad * 2.0, 18.0, [0, 0, 0, (12 - i * 2) as u8]);
    }

    // Main glass plate
    draw_round_rect(&mut pm, 0.5, 0.5, w as f32 - 1.0, h as f32 - 1.0, 16.0, bg);
    stroke_round_rect(&mut pm, 0.5, 0.5, w as f32 - 1.0, h as f32 - 1.0, 16.0, border, 1.0);

    // 1. Connectivity Big Tile (left: Wi-Fi, Bluetooth, AirDrop)
    let left_w = 142.0;
    let top_h = 138.0;
    draw_round_rect(&mut pm, 12.0, 12.0, left_w, top_h, 12.0, tile_bg);

    // Wi-Fi row
    let wifi_col = if cc.wifi_enabled { theme.accent } else { if dark { [60, 60, 70, 255] } else { [210, 210, 220, 255] } };
    fill_circle(&mut pm, 28.0, 32.0, 13.0, wifi_col);
    text.draw_bold(&mut pm, "Wi-Fi", 48.0, 28.5, 12.0, fg);
    text.draw(&mut pm, if cc.wifi_enabled { "Включен" } else { "Выключен" }, 48.0, 41.0, 10.0, fg_dim);

    // Bluetooth row
    let bt_col = if cc.bluetooth_enabled { theme.accent } else { if dark { [60, 60, 70, 255] } else { [210, 210, 220, 255] } };
    fill_circle(&mut pm, 28.0, 74.0, 13.0, bt_col);
    text.draw_bold(&mut pm, "Bluetooth", 48.0, 70.5, 12.0, fg);
    text.draw(&mut pm, if cc.bluetooth_enabled { "Включен" } else { "Выключен" }, 48.0, 83.0, 10.0, fg_dim);

    // AirDrop row
    fill_circle(&mut pm, 28.0, 116.0, 13.0, if dark { [60, 60, 70, 255] } else { [210, 210, 220, 255] });
    text.draw_bold(&mut pm, "AirDrop", 48.0, 112.5, 12.0, fg);
    text.draw(&mut pm, "Только контакты", 48.0, 125.0, 9.5, fg_dim);

    // 2. Right Small Tiles (Theme switch & Battery info)
    let right_x = 164.0;
    let right_w = 144.0;
    let small_h = 64.0;

    // Theme tile
    draw_round_rect(&mut pm, right_x, 12.0, right_w, small_h, 12.0, tile_bg);
    fill_circle(&mut pm, right_x + 22.0, 44.0, 14.0, theme.accent);
    text.draw_bold(&mut pm, "Тема", right_x + 44.0, 38.0, 12.0, fg);
    text.draw(&mut pm, if dark { "Темная" } else { "Светлая" }, right_x + 44.0, 52.0, 10.5, fg_dim);

    // Sound Tile
    draw_round_rect(&mut pm, right_x, 86.0, right_w, small_h, 12.0, tile_bg);
    fill_circle(&mut pm, right_x + 22.0, 118.0, 14.0, [255, 60, 90, 240]);
    text.draw_bold(&mut pm, "Аудио", right_x + 44.0, 112.0, 12.0, fg);
    text.draw(&mut pm, "Встроенный", right_x + 44.0, 126.0, 10.5, fg_dim);

    // 3. Display Brightness Slider Tile
    let slider_w = w as f32 - 24.0;
    let slider_h = 68.0;
    let bright_y = 162.0;
    draw_round_rect(&mut pm, 12.0, bright_y, slider_w, slider_h, 12.0, tile_bg);
    text.draw_bold(&mut pm, "Яркость монитора", 24.0, bright_y + 20.0, 11.5, fg);

    // Brightness bar (track & filled)
    let bar_x = 24.0;
    let bar_y = bright_y + 30.0;
    let bar_w = slider_w - 24.0;
    let bar_h = 24.0;
    let track_bg = if dark { [60, 60, 72, 255] } else { [220, 220, 230, 255] };
    draw_round_rect(&mut pm, bar_x, bar_y, bar_w, bar_h, 12.0, track_bg);
    let fill_w = bar_w * cc.brightness.clamp(0.0, 1.0);
    if fill_w > 0.0 {
        draw_round_rect(&mut pm, bar_x, bar_y, fill_w, bar_h, 12.0, [255, 255, 255, 245]);
    }

    // 4. Sound Volume Slider Tile
    let sound_y = 242.0;
    draw_round_rect(&mut pm, 12.0, sound_y, slider_w, slider_h, 12.0, tile_bg);
    text.draw_bold(&mut pm, "Звук", 24.0, sound_y + 20.0, 11.5, fg);

    let sound_bar_y = sound_y + 30.0;
    draw_round_rect(&mut pm, bar_x, sound_bar_y, bar_w, bar_h, 12.0, track_bg);
    let sound_fill_w = bar_w * cc.volume.clamp(0.0, 1.0);
    if sound_fill_w > 0.0 {
        draw_round_rect(&mut pm, bar_x, sound_bar_y, sound_fill_w, bar_h, 12.0, [255, 255, 255, 245]);
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

// ---------------------------------------------------------------------------
// App Switcher Rendering (Super + Tab)
// ---------------------------------------------------------------------------

pub fn render_app_switcher(
    switcher: &AppSwitcherState,
    theme: &Theme,
    text: &TextRenderer,
) -> Option<MemoryRenderBuffer> {
    if !switcher.active || switcher.items.is_empty() {
        return None;
    }

    let item_size = 72;
    let gap = 14;
    let pad = 16;
    let count = switcher.items.len();
    let total_w = (pad * 2 + count * item_size + (count - 1) * gap) as u32;
    let total_h = 104 as u32;

    let mut pm = Pixmap::new(total_w, total_h)?;
    let dark = theme.dark();

    let bg = if dark {
        [24u8, 24, 30, 235]
    } else {
        [245u8, 245, 250, 235]
    };
    let border = if dark {
        [255u8, 255, 255, 35]
    } else {
        [0u8, 0, 0, 40]
    };
    let fg = theme.menubar_fg();

    // Drop shadow
    for i in (0..5).rev() {
        let p = (i + 1) as f32 * 2.0;
        draw_round_rect(&mut pm, 0.5 - p, 0.5 - p + 3.0, total_w as f32 - 1.0 + p * 2.0, total_h as f32 - 1.0 + p * 2.0, 18.0, [0, 0, 0, (12 - i * 2) as u8]);
    }

    // Body
    draw_round_rect(&mut pm, 0.5, 0.5, total_w as f32 - 1.0, total_h as f32 - 1.0, 16.0, bg);
    stroke_round_rect(&mut pm, 0.5, 0.5, total_w as f32 - 1.0, total_h as f32 - 1.0, 16.0, border, 1.0);

    for (i, item) in switcher.items.iter().enumerate() {
        let x = (pad + i * (item_size + gap)) as f32;
        let y = 16.0;

        let is_sel = i == switcher.selected_index;
        if is_sel {
            // Selection blue box
            draw_round_rect(&mut pm, x - 4.0, y - 4.0, (item_size + 8) as f32, (item_size + 8) as f32, 10.0, theme.accent);
        }

        // App icon badge
        let badge_bg = if is_sel {
            [255, 255, 255, 50]
        } else if dark {
            [48, 48, 56, 255]
        } else {
            [225, 225, 235, 255]
        };
        draw_round_rect(&mut pm, x + 8.0, y + 4.0, 56.0, 48.0, 8.0, badge_bg);

        // App letter inside badge
        if let Some(ch) = item.title.chars().next() {
            let s = ch.to_uppercase().to_string();
            let col = if is_sel { [255, 255, 255, 255] } else { fg };
            text.draw_bold(&mut pm, &s, x + 30.0, y + 34.0, 18.0, col);
        }

        // Window Title below icon
        let max_label_len = 10;
        let short_title: String = if item.title.chars().count() > max_label_len {
            item.title.chars().take(max_label_len).collect::<String>() + "…"
        } else {
            item.title.clone()
        };
        let tw = text.width(&short_title, 10.0);
        let tx = x + (item_size as f32 - tw) / 2.0;
        let tcol = if is_sel { [255, 255, 255, 255] } else { theme.menubar_fg_dim() };
        text.draw(&mut pm, &short_title, tx, y + 66.0, 10.0, tcol);
    }

    Some(MemoryRenderBuffer::from_slice(
        pm.data(),
        Fourcc::Abgr8888,
        (total_w as i32, total_h as i32),
        1,
        Transform::Normal,
        None,
    ))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn fill_circle(pm: &mut Pixmap, cx: f32, cy: f32, r: f32, color: [u8; 4]) {
    let mut pb = PathBuilder::new();
    pb.push_circle(cx, cy, r);
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
        pm.fill_path(&path, &paint, FillRule::Winding, SkTransform::identity(), None);
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
