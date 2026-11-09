//! macOS-style Notification Center and HUD OSD overlays (Volume/Brightness).

use std::time::{Duration, Instant};
use smithay::backend::{
    allocator::Fourcc,
    renderer::element::memory::MemoryRenderBuffer,
};
use smithay::utils::Transform;
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform as SkTransform};

use crate::text::TextRenderer;
use crate::theme::Theme;

#[derive(Debug, Clone)]
pub struct Notification {
    pub id: u64,
    pub title: String,
    pub message: String,
    pub app_name: String,
    pub created_at: Instant,
    pub duration: Duration,
}

pub struct NotificationCenter {
    pub notifications: Vec<Notification>,
    pub panel_open: bool,
    next_id: u64,
}

impl Default for NotificationCenter {
    fn default() -> Self {
        let mut nc = Self {
            notifications: Vec::new(),
            panel_open: false,
            next_id: 1,
        };
        // Add a welcome macOS notification
        nc.post("Aqua Compositor", "Добро пожаловать в Aqua!", "macOS Sequoia UI окружение успешно запущено.");
        nc
    }
}

impl NotificationCenter {
    pub fn post(&mut self, app_name: &str, title: &str, message: &str) {
        let id = self.next_id;
        self.next_id += 1;
        self.notifications.insert(
            0,
            Notification {
                id,
                title: title.to_string(),
                message: message.to_string(),
                app_name: app_name.to_string(),
                created_at: Instant::now(),
                duration: Duration::from_secs(6),
            },
        );
        // keep at most 10 in history
        if self.notifications.len() > 10 {
            self.notifications.truncate(10);
        }
    }

    pub fn toggle_panel(&mut self) {
        self.panel_open = !self.panel_open;
    }

    pub fn close_panel(&mut self) {
        self.panel_open = false;
    }

    pub fn clear(&mut self) {
        self.notifications.clear();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsdKind {
    Volume,
    Brightness,
}

pub struct OsdOverlay {
    pub active: bool,
    pub kind: OsdKind,
    pub value: f32, // 0.0 .. 1.0
    pub changed_at: Instant,
}

impl Default for OsdOverlay {
    fn default() -> Self {
        Self {
            active: false,
            kind: OsdKind::Volume,
            value: 0.65,
            changed_at: Instant::now(),
        }
    }
}

impl OsdOverlay {
    pub fn trigger(&mut self, kind: OsdKind, value: f32) {
        self.active = true;
        self.kind = kind;
        self.value = value.clamp(0.0, 1.0);
        self.changed_at = Instant::now();
    }

    pub fn update(&mut self) {
        if self.active && self.changed_at.elapsed() > Duration::from_millis(1500) {
            self.active = false;
        }
    }
}

pub const TOAST_W: i32 = 340;
pub const TOAST_H: i32 = 72;

/// Render toast notifications floating on the top-right of the screen
    pub fn render_toasts(
    nc: &NotificationCenter,
    theme: &Theme,
    text: &TextRenderer,
    now: Instant,
) -> Vec<(i32, MemoryRenderBuffer)> {
    let mut buffers = Vec::new();
    let dark = theme.dark();

    // Show recent active notifications that haven't expired (duration <= 6s)
    let active_toasts: Vec<&Notification> = nc
        .notifications
        .iter()
        .filter(|n| now.duration_since(n.created_at) < n.duration)
        .take(3)
        .collect();

    let mut cur_y = crate::ui::PANEL_H + 12;

    for notif in active_toasts {
        let mut pm = Pixmap::new(TOAST_W as u32, TOAST_H as u32).expect("toast pixmap");

        // Glass background with soft drop shadow
        for i in (0..4).rev() {
            let s_pad = (i + 1) as f32 * 1.5;
            let alpha = (14 - i * 3) as u8;
            rounded_rect(
                &mut pm,
                0.5 - s_pad,
                0.5 + s_pad * 0.5,
                TOAST_W as f32 - 1.0 + s_pad * 2.0,
                TOAST_H as f32 - 1.0 + s_pad,
                16.0,
                [0, 0, 0, alpha],
            );
        }

        let bg = if dark {
            [30, 30, 38, 225]
        } else {
            [248, 248, 252, 230]
        };
        let border = if dark {
            [255, 255, 255, 38]
        } else {
            [0, 0, 0, 30]
        };
        rounded_rect(&mut pm, 0.5, 0.5, TOAST_W as f32 - 1.0, TOAST_H as f32 - 1.0, 14.0, bg);
        stroke_rounded(&mut pm, 0.5, 0.5, TOAST_W as f32 - 1.0, TOAST_H as f32 - 1.0, 14.0, border, 1.0);

        // App badge icon pill
        rounded_rect(&mut pm, 12.0, 12.0, 18.0, 18.0, 5.0, [0, 122, 255, 230]);
        // App Name + Time
        text.draw_bold(&mut pm, &notif.app_name, 36.0, 24.5, 11.0, theme.menubar_fg_dim());
        text.draw(&mut pm, "сейчас", TOAST_W as f32 - 52.0, 24.5, 11.0, theme.menubar_fg_dim());

        // Title
        text.draw_bold(&mut pm, &notif.title, 14.0, 45.0, 12.5, theme.menubar_fg());
        // Body Message
        let msg = if notif.message.chars().count() > 42 {
            let s: String = notif.message.chars().take(40).collect();
            format!("{}...", s)
        } else {
            notif.message.clone()
        };
        text.draw(&mut pm, &msg, 14.0, 61.0, 11.5, theme.menubar_fg_dim());

        let buf = MemoryRenderBuffer::from_slice(
            pm.data(),
            Fourcc::Abgr8888,
            (TOAST_W, TOAST_H),
            1,
            Transform::Normal,
            None,
        );
        buffers.push((cur_y, buf));

        cur_y += TOAST_H + 8;
    }

    buffers
}

pub const NC_PANEL_W: i32 = 360;
pub const NC_PANEL_H: i32 = 480;

/// Render slide-out Notification Center history panel on the right edge
pub fn render_nc_panel(
    nc: &NotificationCenter,
    theme: &Theme,
    text: &TextRenderer,
) -> Option<MemoryRenderBuffer> {
    if !nc.panel_open {
        return None;
    }

    let mut pm = Pixmap::new(NC_PANEL_W as u32, NC_PANEL_H as u32).expect("nc pixmap");
    let dark = theme.dark();

    // Dark glass plate
    let bg = if dark {
        [24, 24, 30, 230]
    } else {
        [246, 246, 250, 235]
    };
    let border = if dark {
        [255, 255, 255, 30]
    } else {
        [0, 0, 0, 30]
    };

    rounded_rect(&mut pm, 0.5, 0.5, NC_PANEL_W as f32 - 1.0, NC_PANEL_H as f32 - 1.0, 16.0, bg);
    stroke_rounded(&mut pm, 0.5, 0.5, NC_PANEL_W as f32 - 1.0, NC_PANEL_H as f32 - 1.0, 16.0, border, 1.0);

    // Header
    text.draw_bold(&mut pm, "Центр уведомлений", 20.0, 32.0, 15.0, theme.menubar_fg());
    text.draw(&mut pm, "Очистить всё", NC_PANEL_W as f32 - 100.0, 32.0, 12.0, [0, 122, 255, 255]);

    let mut y = 56.0;
    if nc.notifications.is_empty() {
        text.draw(&mut pm, "Нет новых уведомлений", 20.0, 90.0, 13.0, theme.menubar_fg_dim());
    } else {
        for n in nc.notifications.iter().take(6) {
            let card_h = 60.0;
            let card_bg = if dark {
                [38, 38, 48, 180]
            } else {
                [255, 255, 255, 190]
            };
            rounded_rect(&mut pm, 14.0, y, NC_PANEL_W as f32 - 28.0, card_h, 10.0, card_bg);
            text.draw_bold(&mut pm, &n.app_name, 24.0, y + 18.0, 10.5, theme.menubar_fg_dim());
            text.draw_bold(&mut pm, &n.title, 24.0, y + 34.0, 12.0, theme.menubar_fg());
            let body: String = n.message.chars().take(40).collect();
            text.draw(&mut pm, &body, 24.0, y + 49.0, 11.0, theme.menubar_fg_dim());
            y += card_h + 8.0;
        }
    }

    let buf = MemoryRenderBuffer::from_slice(
        pm.data(),
        Fourcc::Abgr8888,
        (NC_PANEL_W, NC_PANEL_H),
        1,
        Transform::Normal,
        None,
    );
    Some(buf)
}

pub const OSD_SIZE: i32 = 180;

/// Render classic macOS HUD OSD (Volume / Brightness) centered on screen
pub fn render_osd(
    osd: &OsdOverlay,
    theme: &Theme,
    text: &TextRenderer,
) -> Option<MemoryRenderBuffer> {
    if !osd.active {
        return None;
    }

    let mut pm = Pixmap::new(OSD_SIZE as u32, OSD_SIZE as u32).expect("osd pixmap");
    let dark = theme.dark();

    let bg = if dark {
        [28, 28, 34, 215]
    } else {
        [240, 240, 245, 215]
    };
    let border = if dark {
        [255, 255, 255, 30]
    } else {
        [0, 0, 0, 25]
    };

    // Soft HUD glass square with rounded corners
    rounded_rect(&mut pm, 0.5, 0.5, OSD_SIZE as f32 - 1.0, OSD_SIZE as f32 - 1.0, 22.0, bg);
    stroke_rounded(&mut pm, 0.5, 0.5, OSD_SIZE as f32 - 1.0, OSD_SIZE as f32 - 1.0, 22.0, border, 1.0);

    let cx = OSD_SIZE as f32 / 2.0;
    let fg = theme.menubar_fg();

    // Draw Icon in center
    match osd.kind {
        OsdKind::Volume => {
            // Speaker icon
            let icon_y = 62.0;
            let mut pb = PathBuilder::new();
            pb.move_to(cx - 16.0, icon_y - 6.0);
            pb.line_to(cx - 8.0, icon_y - 6.0);
            pb.line_to(cx + 2.0, icon_y - 14.0);
            pb.line_to(cx + 2.0, icon_y + 14.0);
            pb.line_to(cx - 8.0, icon_y + 6.0);
            pb.line_to(cx - 16.0, icon_y + 6.0);
            pb.close();
            if let Some(path) = pb.finish() {
                let mut paint = Paint::default();
                paint.set_color_rgba8(fg[0], fg[1], fg[2], fg[3]);
                pm.fill_path(&path, &paint, FillRule::Winding, SkTransform::identity(), None);
            }
            // Sound waves
            for r in [12.0f32, 20.0] {
                let mut pb = PathBuilder::new();
                pb.move_to(cx + 8.0, icon_y - r * 0.7);
                pb.quad_to(cx + 8.0 + r * 0.7, icon_y, cx + 8.0, icon_y + r * 0.7);
                if let Some(p) = pb.finish() {
                    let mut paint = Paint::default();
                    paint.set_color_rgba8(fg[0], fg[1], fg[2], fg[3]);
                    let stroke = Stroke {
                        width: 2.2,
                        ..Default::default()
                    };
                    pm.stroke_path(&p, &paint, &stroke, SkTransform::identity(), None);
                }
            }
            let label = format!("{:.0}%", osd.value * 100.0);
            let lw = text.width_bold(&label, 12.0);
            text.draw_bold(&mut pm, &label, cx - lw / 2.0, 102.0, 12.0, fg);
        }
        OsdKind::Brightness => {
            // Sun icon
            let icon_y = 62.0;
            fill_circle(&mut pm, cx, icon_y, 8.0, fg);
            for i in 0..8 {
                let angle = (i as f32) * (std::f32::consts::PI / 4.0);
                let x1 = cx + angle.cos() * 12.0;
                let y1 = icon_y + angle.sin() * 12.0;
                let x2 = cx + angle.cos() * 17.0;
                let y2 = icon_y + angle.sin() * 17.0;
                let mut pb = PathBuilder::new();
                pb.move_to(x1, y1);
                pb.line_to(x2, y2);
                if let Some(p) = pb.finish() {
                    let mut paint = Paint::default();
                    paint.set_color_rgba8(fg[0], fg[1], fg[2], fg[3]);
                    let stroke = Stroke {
                        width: 2.2,
                        ..Default::default()
                    };
                    pm.stroke_path(&p, &paint, &stroke, SkTransform::identity(), None);
                }
            }
            let label = format!("{:.0}%", osd.value * 100.0);
            let lw = text.width_bold(&label, 12.0);
            text.draw_bold(&mut pm, &label, cx - lw / 2.0, 102.0, 12.0, fg);
        }
    }

    // Progress bar at bottom of HUD
    let bar_x = 24.0;
    let bar_y = 136.0;
    let bar_w = OSD_SIZE as f32 - 48.0;
    let bar_h = 7.0;

    let track_bg = if dark {
        [255, 255, 255, 35]
    } else {
        [0, 0, 0, 30]
    };
    rounded_rect(&mut pm, bar_x, bar_y, bar_w, bar_h, 3.5, track_bg);

    let fill_w = (bar_w * osd.value).max(4.0);
    let fill_fg = if dark {
        [255, 255, 255, 240]
    } else {
        [30, 30, 35, 230]
    };
    rounded_rect(&mut pm, bar_x, bar_y, fill_w, bar_h, 3.5, fill_fg);

    let buf = MemoryRenderBuffer::from_slice(
        pm.data(),
        Fourcc::Abgr8888,
        (OSD_SIZE, OSD_SIZE),
        1,
        Transform::Normal,
        None,
    );
    Some(buf)
}

fn rounded_rect(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, r: f32, color: [u8; 4]) {
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
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
        pm.fill_path(&path, &paint, FillRule::Winding, SkTransform::identity(), None);
    }
}

fn stroke_rounded(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, r: f32, color: [u8; 4], width: f32) {
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
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
        let stroke = Stroke {
            width,
            ..Default::default()
        };
        pm.stroke_path(&path, &paint, &stroke, SkTransform::identity(), None);
    }
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
