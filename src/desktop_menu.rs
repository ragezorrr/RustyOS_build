//! macOS-style Desktop Context Menu and Stage Manager (recent app window clusters).

use smithay::backend::{
    allocator::Fourcc,
    renderer::element::memory::MemoryRenderBuffer,
};
use smithay::desktop::Window;
use smithay::utils::{Logical, Point, Transform};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform as SkTransform};

use crate::text::TextRenderer;
use crate::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopMenuAction {
    NewFolder,
    SortBy,
    CleanUp,
    ChangeWallpaper,
    OpenTerminal,
    SystemSettings,
}

#[derive(Debug, Clone)]
pub struct DesktopMenuItem {
    pub label: &'static str,
    pub action: DesktopMenuAction,
    pub is_separator: bool,
}

pub struct DesktopContextMenu {
    pub open: bool,
    pub pos: Point<i32, Logical>,
    pub hovered: Option<usize>,
    pub items: Vec<DesktopMenuItem>,
}

impl Default for DesktopContextMenu {
    fn default() -> Self {
        Self {
            open: false,
            pos: Point::from((0, 0)),
            hovered: None,
            items: vec![
                DesktopMenuItem { label: "Новая папка", action: DesktopMenuAction::NewFolder, is_separator: false },
                DesktopMenuItem { label: "", action: DesktopMenuAction::NewFolder, is_separator: true },
                DesktopMenuItem { label: "Сортировка", action: DesktopMenuAction::SortBy, is_separator: false },
                DesktopMenuItem { label: "Выровнять значки", action: DesktopMenuAction::CleanUp, is_separator: false },
                DesktopMenuItem { label: "", action: DesktopMenuAction::NewFolder, is_separator: true },
                DesktopMenuItem { label: "Сменить обои...", action: DesktopMenuAction::ChangeWallpaper, is_separator: false },
                DesktopMenuItem { label: "Открыть в Терминале", action: DesktopMenuAction::OpenTerminal, is_separator: false },
                DesktopMenuItem { label: "Системные настройки...", action: DesktopMenuAction::SystemSettings, is_separator: false },
            ],
        }
    }
}

impl DesktopContextMenu {
    pub fn open_at(&mut self, pos: Point<i32, Logical>) {
        self.open = true;
        self.pos = pos;
        self.hovered = None;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.hovered = None;
    }
}

pub const DESKTOP_MENU_W: i32 = 210;

pub fn render_desktop_menu(
    menu: &DesktopContextMenu,
    theme: &Theme,
    text: &TextRenderer,
    pointer_pos: Option<(f64, f64)>,
) -> Option<MemoryRenderBuffer> {
    if !menu.open {
        return None;
    }

    let h = 12 + menu.items.iter().map(|it| if it.is_separator { 9 } else { 26 }).sum::<i32>();
    let mut pm = Pixmap::new(DESKTOP_MENU_W as u32, h as u32).expect("desktop menu pixmap");
    let dark = theme.dark();

    let bg = if dark {
        [28, 28, 36, 235]
    } else {
        [248, 248, 252, 238]
    };
    let border = if dark {
        [255, 255, 255, 36]
    } else {
        [0, 0, 0, 25]
    };

    rounded_rect(&mut pm, 0.5, 0.5, DESKTOP_MENU_W as f32 - 1.0, h as f32 - 1.0, 10.0, bg);
    stroke_rounded(&mut pm, 0.5, 0.5, DESKTOP_MENU_W as f32 - 1.0, h as f32 - 1.0, 10.0, border, 1.0);

    let mut cur_y = 6;
    for (_i, item) in menu.items.iter().enumerate() {
        if item.is_separator {
            fill_rect(&mut pm, 10.0, cur_y as f32 + 4.0, DESKTOP_MENU_W as f32 - 20.0, 1.0, if dark { [255, 255, 255, 20] } else { [0, 0, 0, 25] });
            cur_y += 9;
        } else {
            let is_hover = pointer_pos.map_or(false, |(px, py)| {
                let rx = px as i32 - menu.pos.x;
                let ry = py as i32 - menu.pos.y;
                rx >= 4 && rx < DESKTOP_MENU_W - 4 && ry >= cur_y && ry < cur_y + 26
            });

            if is_hover {
                rounded_rect(&mut pm, 5.0, cur_y as f32, DESKTOP_MENU_W as f32 - 10.0, 25.0, 6.0, [0, 122, 255, 255]);
                text.draw(&mut pm, item.label, 16.0, cur_y as f32 + 17.0, 12.5, [255, 255, 255, 255]);
            } else {
                text.draw(&mut pm, item.label, 16.0, cur_y as f32 + 17.0, 12.5, theme.menubar_fg());
            }
            cur_y += 26;
        }
    }

    let buf = MemoryRenderBuffer::from_slice(
        pm.data(),
        Fourcc::Abgr8888,
        (DESKTOP_MENU_W, h),
        1,
        Transform::Normal,
        None,
    );
    Some(buf)
}

/// Stage Manager state (sidebar showing grouped thumbnails of inactive applications on the left)
pub struct StageManager {
    pub enabled: bool,
    pub hovered_group: Option<usize>,
}

impl Default for StageManager {
    fn default() -> Self {
        Self {
            enabled: true, // macOS Ventura+ style stage manager
            hovered_group: None,
        }
    }
}

pub const STAGE_W: i32 = 72;

pub fn render_stage_manager(
    stage: &StageManager,
    windows: &[Window],
    focused_window: Option<&Window>,
    theme: &Theme,
    text: &TextRenderer,
    _screen_h: i32,
    pointer_pos: Option<(f64, f64)>,
) -> Option<(i32, MemoryRenderBuffer)> {
    if !stage.enabled {
        return None;
    }

    // Collect inactive windows (up to 4 groups)
    let inactive_windows: Vec<&Window> = windows
        .iter()
        .filter(|w| Some(*w) != focused_window)
        .take(4)
        .collect();

    if inactive_windows.is_empty() {
        return None;
    }

    let item_h = 64;
    let gap = 14;
    let total_h = inactive_windows.len() as i32 * (item_h + gap);
    let mut pm = Pixmap::new(STAGE_W as u32, total_h as u32).expect("stage pixmap");
    let dark = theme.dark();

    let mut y = 0.0;
    for (i, win) in inactive_windows.iter().enumerate() {
        let is_hover = pointer_pos.map_or(false, |(px, py)| {
            let rx = px as i32 - 12;
            let ry = py as i32 - (i as i32 * (item_h + gap));
            rx >= 0 && rx < STAGE_W && ry >= 0 && ry < item_h
        });

        let card_w = if is_hover { STAGE_W as f32 } else { STAGE_W as f32 - 6.0 };
        let card_bg = if dark {
            if is_hover { [44, 44, 56, 210] } else { [32, 32, 42, 160] }
        } else {
            if is_hover { [255, 255, 255, 230] } else { [245, 245, 250, 180] }
        };
        let border = if is_hover { [0, 122, 255, 200] } else { [255, 255, 255, 30] };

        // Soft miniature card representing window
        rounded_rect(&mut pm, 3.0, y + 2.0, card_w, item_h as f32 - 4.0, 10.0, card_bg);
        stroke_rounded(&mut pm, 3.0, y + 2.0, card_w, item_h as f32 - 4.0, 10.0, border, 1.0);

        // Traffic light miniature dots in card header
        fill_circle(&mut pm, 12.0, y + 10.0, 2.0, [255, 95, 87, 240]);
        fill_circle(&mut pm, 18.0, y + 10.0, 2.0, [254, 188, 46, 240]);
        fill_circle(&mut pm, 24.0, y + 10.0, 2.0, [40, 201, 64, 240]);

        // Mini app icon or title
        let title = crate::render::window_title(win).unwrap_or_else(|| "Окно".to_string());
        let short: String = title.chars().take(6).collect();
        text.draw(&mut pm, &short, 10.0, y + 36.0, 10.5, theme.menubar_fg());

        y += (item_h + gap) as f32;
    }

    let buf = MemoryRenderBuffer::from_slice(
        pm.data(),
        Fourcc::Abgr8888,
        (STAGE_W, total_h),
        1,
        Transform::Normal,
        None,
    );
    Some((total_h, buf))
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

fn fill_rect(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, color: [u8; 4]) {
    let mut paint = Paint::default();
    paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
    if let Some(rect) = tiny_skia::Rect::from_xywh(x, y, w, h) {
        pm.fill_rect(rect, &paint, SkTransform::identity(), None);
    }
}
