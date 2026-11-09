//! System settings window for Aqua (Rust / tiny-skia UI).

use smithay::backend::{
    allocator::Fourcc,
    renderer::element::memory::MemoryRenderBuffer,
};
use smithay::utils::Transform;
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Transform as SkTransform};

use crate::text::TextRenderer;
use crate::theme::{Appearance, Theme, WallpaperTheme};

pub const SETTINGS_W: i32 = 680;
pub const SETTINGS_H: i32 = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsTab {
    Displays,
    Wallpaper,
    Dock,
    Appearance,
    About,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SettingsAction {
    None,
    ThemeChanged,
    BrightnessChanged(f32),
    WallpaperChanged,
}

#[derive(Debug, Clone)]
pub struct DisplayConfig {
    pub resolution: (i32, i32),
    pub refresh_rate: i32,
    pub brightness: f32,        // 0.0 .. 1.0 (default 0.85)
    pub scaling: i32,           // 100, 125, 150, 200
    pub selected_preset: usize, // 0: 1280x720, 1: 1600x900, 2: 1920x1080, 3: 2560x1440
    pub night_shift: bool,
    pub auto_brightness: bool,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            resolution: (1920, 1080),
            refresh_rate: 60,
            brightness: 0.85,
            scaling: 100,
            selected_preset: 2,
            night_shift: false,
            auto_brightness: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DockConfig {
    pub icon_size: i32,        // 36 .. 64 (default 48)
    pub magnification: bool,   // on / off
    pub max_scale: f32,        // 1.1 .. 1.6 (default 1.32)
    pub autohide: bool,        // on / off
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
    pub display: DisplayConfig,
    pub loc: Option<(i32, i32)>,
}

impl Default for SettingsWindow {
    fn default() -> Self {
        Self {
            open: false,
            current_tab: SettingsTab::Displays,
            dock: DockConfig::default(),
            display: DisplayConfig::default(),
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

    pub fn handle_click(&mut self, local_x: i32, local_y: i32, theme: &mut Theme) -> SettingsAction {
        // Tab sidebar clicks (x: 10..180)
        if local_x >= 10 && local_x < 186 {
            let tab_y = 54;
            let tab_h = 34;
            if local_y >= tab_y && local_y < tab_y + tab_h {
                self.current_tab = SettingsTab::Displays;
                return SettingsAction::None;
            } else if local_y >= tab_y + tab_h && local_y < tab_y + tab_h * 2 {
                self.current_tab = SettingsTab::Wallpaper;
                return SettingsAction::None;
            } else if local_y >= tab_y + tab_h * 2 && local_y < tab_y + tab_h * 3 {
                self.current_tab = SettingsTab::Dock;
                return SettingsAction::None;
            } else if local_y >= tab_y + tab_h * 3 && local_y < tab_y + tab_h * 4 {
                self.current_tab = SettingsTab::Appearance;
                return SettingsAction::None;
            } else if local_y >= tab_y + tab_h * 4 && local_y < tab_y + tab_h * 5 {
                self.current_tab = SettingsTab::About;
                return SettingsAction::None;
            }
        }

        // Tab Content clicks (content origin: x = 200, y = 50)
        let cx = local_x - 200;
        let cy = local_y - 50;

        match self.current_tab {
            SettingsTab::Displays => {
                // 1. Brightness slider (cx: 10..380, cy: 190..230)
                if cx >= 10 && cx < 380 && cy >= 190 && cy < 232 {
                    let ratio = ((cx - 10) as f32 / 360.0).clamp(0.05, 1.0);
                    self.display.brightness = ratio;
                    return SettingsAction::BrightnessChanged(ratio);
                }

                // 2. Preset resolution cards (cy: 260..324)
                if cy >= 260 && cy < 324 {
                    if cx >= 10 && cx < 96 {
                        self.display.selected_preset = 0;
                        self.display.resolution = (1280, 720);
                        self.display.scaling = 100;
                        return SettingsAction::None;
                    } else if cx >= 102 && cx < 188 {
                        self.display.selected_preset = 1;
                        self.display.resolution = (1600, 900);
                        self.display.scaling = 100;
                        return SettingsAction::None;
                    } else if cx >= 194 && cx < 280 {
                        self.display.selected_preset = 2;
                        self.display.resolution = (1920, 1080);
                        self.display.scaling = 100;
                        return SettingsAction::None;
                    } else if cx >= 286 && cx < 372 {
                        self.display.selected_preset = 3;
                        self.display.resolution = (1920, 1080);
                        self.display.scaling = 125;
                        return SettingsAction::None;
                    }
                }

                // 3. Night Shift toggle (cx: 10..380, cy: 345..385)
                if cx >= 10 && cx < 380 && cy >= 345 && cy < 385 {
                    self.display.night_shift = !self.display.night_shift;
                    theme.night_shift = self.display.night_shift;
                    theme.save();
                    return SettingsAction::ThemeChanged;
                }

                // 4. Auto brightness toggle (cx: 10..380, cy: 395..435)
                if cx >= 10 && cx < 380 && cy >= 395 && cy < 435 {
                    self.display.auto_brightness = !self.display.auto_brightness;
                    return SettingsAction::None;
                }
            }
            SettingsTab::Wallpaper => {
                // 4 Wallpaper cards grid (2 columns x 2 rows)
                // Row 1: cy: 40..170
                if cy >= 40 && cy < 170 {
                    if cx >= 10 && cx < 220 {
                        theme.wallpaper_theme = WallpaperTheme::Sequoia;
                        theme.save();
                        return SettingsAction::WallpaperChanged;
                    } else if cx >= 235 && cx < 445 {
                        theme.wallpaper_theme = WallpaperTheme::Sonoma;
                        theme.save();
                        return SettingsAction::WallpaperChanged;
                    }
                }
                // Row 2: cy: 190..320
                if cy >= 190 && cy < 320 {
                    if cx >= 10 && cx < 220 {
                        theme.wallpaper_theme = WallpaperTheme::Monterey;
                        theme.save();
                        return SettingsAction::WallpaperChanged;
                    } else if cx >= 235 && cx < 445 {
                        theme.wallpaper_theme = WallpaperTheme::Midnight;
                        theme.save();
                        return SettingsAction::WallpaperChanged;
                    }
                }
            }
            SettingsTab::Dock => {
                // 1. Icon size slider (cx: 10..380, cy: 30..56)
                if cx >= 10 && cx < 380 && cy >= 30 && cy < 56 {
                    let ratio = ((cx - 10) as f32 / 360.0).clamp(0.0, 1.0);
                    self.dock.icon_size = (36.0 + ratio * 28.0).round() as i32;
                    return SettingsAction::None;
                }
                // 2. Magnification toggle (cx: 10..380, cy: 90..120)
                if cx >= 10 && cx < 380 && cy >= 90 && cy < 120 {
                    self.dock.magnification = !self.dock.magnification;
                    return SettingsAction::None;
                }
                // 3. Magnification scale slider (cx: 10..380, cy: 150..176)
                if cx >= 10 && cx < 380 && cy >= 150 && cy < 176 {
                    let ratio = ((cx - 10) as f32 / 360.0).clamp(0.0, 1.0);
                    self.dock.max_scale = 1.1 + ratio * 0.5;
                    return SettingsAction::None;
                }
                // 4. Show indicators toggle (cx: 10..380, cy: 210..240)
                if cx >= 10 && cx < 380 && cy >= 210 && cy < 240 {
                    self.dock.show_indicators = !self.dock.show_indicators;
                    return SettingsAction::None;
                }
            }
            SettingsTab::Appearance => {
                // Dark mode card (cx: 10..140, cy: 40..125)
                if cx >= 10 && cx < 140 && cy >= 40 && cy < 125 {
                    theme.appearance = Appearance::Dark;
                    theme.save();
                    return SettingsAction::ThemeChanged;
                }
                // Light mode card (cx: 155..285, cy: 40..125)
                if cx >= 155 && cx < 285 && cy >= 40 && cy < 125 {
                    theme.appearance = Appearance::Light;
                    theme.save();
                    return SettingsAction::ThemeChanged;
                }

                // Accent colors circles (cy: 175..215)
                if cy >= 175 && cy < 215 {
                    let accents: [[u8; 4]; 7] = [
                        [0, 122, 255, 255],   // Blue
                        [175, 82, 222, 255],  // Purple
                        [255, 45, 85, 255],   // Pink
                        [255, 59, 48, 255],   // Red
                        [255, 149, 0, 255],   // Orange
                        [52, 199, 89, 255],   // Green
                        [142, 142, 147, 255], // Graphite
                    ];
                    for (i, &col) in accents.iter().enumerate() {
                        let ac_x = 10 + i as i32 * 38;
                        if cx >= ac_x && cx < ac_x + 30 {
                            theme.accent = col;
                            theme.save();
                            return SettingsAction::ThemeChanged;
                        }
                    }
                }
            }
            SettingsTab::About => {}
        }

        SettingsAction::None
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
    let bg = if dark { [30u8, 30, 36, 248] } else { [246u8, 246, 250, 248] };
    let sidebar_bg = if dark { [22u8, 22, 28, 248] } else { [236u8, 236, 242, 248] };
    let border = if dark { [255u8, 255, 255, 30] } else { [0u8, 0, 0, 35] };
    let fg = theme.menubar_fg();
    let fg_dim = theme.menubar_fg_dim();

    // Soft drop shadow
    for i in (0..6).rev() {
        let pad = (i + 1) as f32 * 2.5;
        draw_round_rect(
            &mut pm,
            0.5 - pad,
            0.5 - pad + 4.0,
            w as f32 - 1.0 + pad * 2.0,
            h as f32 - 1.0 + pad * 2.0,
            16.0,
            [0, 0, 0, (14 - i * 2) as u8],
        );
    }

    // Main window plate
    draw_round_rect(&mut pm, 0.5, 0.5, w as f32 - 1.0, h as f32 - 1.0, 14.0, bg);
    stroke_round_rect(&mut pm, 0.5, 0.5, w as f32 - 1.0, h as f32 - 1.0, 14.0, border, 1.0);

    // Sidebar background (left column)
    draw_round_rect(&mut pm, 0.5, 0.5, 186.0, h as f32 - 1.0, 14.0, sidebar_bg);
    fill_rect(&mut pm, 186.0, 0.0, 1.0, h as f32, if dark { [255, 255, 255, 18] } else { [0, 0, 0, 25] });

    // Window traffic lights (inside settings window header)
    draw_circle(&mut pm, 20.0, 22.0, 6.0, [255, 95, 87, 255]);
    draw_circle(&mut pm, 38.0, 22.0, 6.0, [254, 188, 46, 255]);
    draw_circle(&mut pm, 56.0, 22.0, 6.0, [40, 200, 64, 255]);

    // Sidebar items
    let tabs = [
        (SettingsTab::Displays, "Дисплеи", [0, 122, 255, 255]),
        (SettingsTab::Wallpaper, "Обои", [255, 149, 0, 255]),
        (SettingsTab::Dock, "Dock & Меню", [175, 82, 222, 255]),
        (SettingsTab::Appearance, "Оформление", [142, 142, 147, 255]),
        (SettingsTab::About, "О системе", [100, 100, 110, 255]),
    ];

    let mut tab_y = 54.0;
    for (tab, label, badge_col) in tabs {
        let is_sel = settings.current_tab == tab;
        if is_sel {
            draw_round_rect(&mut pm, 10.0, tab_y, 166.0, 30.0, 7.0, theme.accent);
        }

        // Little macOS style rounded icon badge in sidebar
        draw_round_rect(&mut pm, 18.0, tab_y + 6.0, 18.0, 18.0, 4.0, badge_col);

        let col = if is_sel { [255, 255, 255, 255] } else { fg };
        text.draw_bold(&mut pm, label, 44.0, tab_y + 19.5, 12.5, col);
        tab_y += 34.0;
    }

    // Content area (x: 210 .. 660, y: 30 .. 490)
    let cx = 210.0;
    let cy = 30.0;

    match settings.current_tab {
        SettingsTab::Displays => {
            text.draw_bold(&mut pm, "Настройки дисплея", cx, cy + 18.0, 18.0, fg);

            // 1. Retina Display Preview Illustration
            let mon_x = cx + 80.0;
            let mon_y = cy + 34.0;
            let mon_w = 160.0;
            let mon_h = 96.0;

            // Monitor stand
            fill_rect(&mut pm, mon_x + 72.0, mon_y + mon_h, 16.0, 12.0, [130, 130, 140, 255]);
            draw_round_rect(&mut pm, mon_x + 56.0, mon_y + mon_h + 10.0, 48.0, 4.0, 2.0, [160, 160, 170, 255]);

            // Monitor frame / bezel
            draw_round_rect(&mut pm, mon_x, mon_y, mon_w, mon_h, 8.0, [32, 32, 38, 255]);
            stroke_round_rect(&mut pm, mon_x, mon_y, mon_w, mon_h, 8.0, [80, 80, 95, 255], 1.2);

            // Screen content (wallpaper preview inside monitor)
            let sc_x = mon_x + 5.0;
            let sc_y = mon_y + 5.0;
            let sc_w = mon_w - 10.0;
            let sc_h = mon_h - 10.0;
            let top_c = theme.wallpaper_top();
            let bot_c = theme.wallpaper_bottom();
            let glow_c = theme.wallpaper_glow();
            draw_round_rect(&mut pm, sc_x, sc_y, sc_w, sc_h, 4.0, bot_c);
            fill_rect(&mut pm, sc_x, sc_y, sc_w, sc_h * 0.5, top_c);
            fill_circle(&mut pm, sc_x + sc_w * 0.4, sc_y + sc_h * 0.4, 24.0, glow_c);
            // Miniature menu bar & dock
            fill_rect(&mut pm, sc_x, sc_y, sc_w, 3.0, [255, 255, 255, 60]);
            draw_round_rect(&mut pm, sc_x + sc_w * 0.25, sc_y + sc_h - 4.0, sc_w * 0.5, 3.0, 1.5, [255, 255, 255, 90]);

            // Display Title & Info
            text.draw_bold(&mut pm, "Встроенный дисплей Liquid Retina", cx + 55.0, mon_y + mon_h + 28.0, 12.5, fg);
            let mode_str = format!("{} × {} @ {} Гц", settings.display.resolution.0, settings.display.resolution.1, settings.display.refresh_rate);
            text.draw(&mut pm, &mode_str, cx + 110.0, mon_y + mon_h + 44.0, 11.5, fg_dim);

            // 2. Brightness Slider
            let b_y = cy + 196.0;
            text.draw_bold(&mut pm, "Яркость", cx, b_y + 16.0, 12.5, fg);
            let b_str = format!("{:.0}%", settings.display.brightness * 100.0);
            text.draw(&mut pm, &b_str, cx + 335.0, b_y + 16.0, 11.5, fg_dim);

            let bar_y = b_y + 26.0;
            let bar_w = 360.0;
            draw_round_rect(&mut pm, cx, bar_y, bar_w, 8.0, 4.0, if dark { [60, 60, 70, 255] } else { [220, 220, 230, 255] });
            let fill_w = bar_w * settings.display.brightness.clamp(0.0, 1.0);
            draw_round_rect(&mut pm, cx, bar_y, fill_w, 8.0, 4.0, theme.accent);
            fill_circle(&mut pm, cx + fill_w, bar_y + 4.0, 8.0, [255, 255, 255, 255]);

            // 3. Resolution / Scaling Cards
            let sc_y = cy + 248.0;
            text.draw_bold(&mut pm, "Разрешение и масштабирование", cx, sc_y + 14.0, 12.5, fg);

            let presets = [
                ("1280×720", "Крупнее"),
                ("1600×900", "Среднее"),
                ("1920×1080", "Стандарт"),
                ("Больше", "125%"),
            ];

            let card_w = 84.0;
            let card_h = 50.0;
            for (i, (res, label)) in presets.iter().enumerate() {
                let px = cx + i as f32 * 92.0;
                let py = sc_y + 24.0;
                let is_sel = settings.display.selected_preset == i;
                let card_bg = if dark {
                    if is_sel { [42, 42, 52, 255] } else { [32, 32, 38, 255] }
                } else {
                    if is_sel { [255, 255, 255, 255] } else { [236, 236, 242, 255] }
                };

                draw_round_rect(&mut pm, px, py, card_w, card_h, 8.0, card_bg);
                if is_sel {
                    stroke_round_rect(&mut pm, px, py, card_w, card_h, 8.0, theme.accent, 2.0);
                } else {
                    stroke_round_rect(&mut pm, px, py, card_w, card_h, 8.0, if dark { [60, 60, 72, 200] } else { [210, 210, 220, 255] }, 1.0);
                }

                text.draw_bold(&mut pm, res, px + 12.0, py + 22.0, 11.0, fg);
                text.draw(&mut pm, label, px + 18.0, py + 38.0, 10.0, fg_dim);
            }

            // 4. Night Shift Toggle
            let ns_y = cy + 342.0;
            text.draw_bold(&mut pm, "Night Shift", cx, ns_y + 14.0, 12.5, fg);
            text.draw(&mut pm, "Снижает нагрузку на глаза, делая цвета теплее", cx, ns_y + 28.0, 10.5, fg_dim);
            draw_toggle(&mut pm, cx + 316.0, ns_y + 8.0, settings.display.night_shift, theme.accent);

            // 5. Auto Brightness Toggle
            let ab_y = cy + 392.0;
            text.draw_bold(&mut pm, "Автоматическая яркость", cx, ab_y + 14.0, 12.5, fg);
            text.draw(&mut pm, "Плавная адаптация подсветки под окружающий свет", cx, ab_y + 28.0, 10.5, fg_dim);
            draw_toggle(&mut pm, cx + 316.0, ab_y + 8.0, settings.display.auto_brightness, theme.accent);
        }
        SettingsTab::Wallpaper => {
            text.draw_bold(&mut pm, "Обои рабочего стола", cx, cy + 18.0, 18.0, fg);
            text.draw(&mut pm, "Выберите стиль градиента и авроры macOS", cx, cy + 36.0, 11.5, fg_dim);

            let wallpapers = [
                (WallpaperTheme::Sequoia, "macOS Sequoia", "Фирменная аврора индиго и фиолета", [18, 14, 42, 255], [6, 4, 14, 255], [68, 48, 148, 255]),
                (WallpaperTheme::Sonoma, "Sonoma Sunset", "Теплые лучи золотого заката", [34, 16, 12, 255], [10, 6, 8, 255], [175, 75, 30, 255]),
                (WallpaperTheme::Monterey, "Monterey Blue", "Глубокий океанический сапфир", [12, 22, 52, 255], [4, 8, 22, 255], [20, 115, 165, 255]),
                (WallpaperTheme::Midnight, "Midnight Space", "Минималистичный глубокий космос", [14, 16, 24, 255], [4, 4, 8, 255], [55, 68, 95, 255]),
            ];

            let card_w = 200.0;

            for (i, (theme_variant, title, desc, top_col, bot_col, glow_col)) in wallpapers.iter().enumerate() {
                let col_idx = i % 2;
                let row_idx = i / 2;
                let wx = cx + col_idx as f32 * 220.0;
                let wy = cy + 54.0 + row_idx as f32 * 155.0;
                let is_sel = theme.wallpaper_theme == *theme_variant;

                // Thumbnail plate
                let thumb_h = 88.0;
                draw_round_rect(&mut pm, wx, wy, card_w, thumb_h, 8.0, *bot_col);
                fill_rect(&mut pm, wx, wy, card_w, thumb_h * 0.5, *top_col);
                fill_circle(&mut pm, wx + card_w * 0.45, wy + thumb_h * 0.45, 36.0, *glow_col);

                // Thumbnail border
                if is_sel {
                    stroke_round_rect(&mut pm, wx, wy, card_w, thumb_h, 8.0, theme.accent, 2.5);
                } else {
                    stroke_round_rect(&mut pm, wx, wy, card_w, thumb_h, 8.0, if dark { [60, 60, 72, 180] } else { [210, 210, 225, 220] }, 1.0);
                }

                text.draw_bold(&mut pm, title, wx + 4.0, wy + thumb_h + 18.0, 12.0, fg);
                text.draw(&mut pm, desc, wx + 4.0, wy + thumb_h + 32.0, 10.0, fg_dim);
            }
        }
        SettingsTab::Dock => {
            text.draw_bold(&mut pm, "Настройки панели Dock", cx, cy + 18.0, 18.0, fg);

            // 1. Icon Size Slider
            text.draw_bold(&mut pm, "Размер значков", cx, cy + 56.0, 12.5, fg);
            let size_str = format!("{} px", settings.dock.icon_size);
            text.draw(&mut pm, &size_str, cx + 335.0, cy + 56.0, 11.5, fg_dim);

            let bar_y = cy + 68.0;
            let bar_w = 360.0;
            draw_round_rect(&mut pm, cx, bar_y, bar_w, 8.0, 4.0, if dark { [60, 60, 70, 255] } else { [220, 220, 230, 255] });
            let ratio = (settings.dock.icon_size - 36) as f32 / 28.0;
            let fill_w = bar_w * ratio.clamp(0.0, 1.0);
            draw_round_rect(&mut pm, cx, bar_y, fill_w, 8.0, 4.0, theme.accent);
            fill_circle(&mut pm, cx + fill_w, bar_y + 4.0, 8.0, [255, 255, 255, 255]);

            // 2. Magnification Checkbox
            let mag_y = cy + 104.0;
            text.draw_bold(&mut pm, "Увеличение (Magnification)", cx, mag_y + 16.0, 12.5, fg);
            draw_toggle(&mut pm, cx + 316.0, mag_y + 4.0, settings.dock.magnification, theme.accent);

            // 3. Magnification Scale
            if settings.dock.magnification {
                text.draw_bold(&mut pm, "Коэффициент увеличения", cx, cy + 154.0, 12.5, fg);
                let scale_str = format!("{:.0}%", settings.dock.max_scale * 100.0);
                text.draw(&mut pm, &scale_str, cx + 335.0, cy + 154.0, 11.5, fg_dim);

                let scale_bar_y = cy + 166.0;
                draw_round_rect(&mut pm, cx, scale_bar_y, bar_w, 8.0, 4.0, if dark { [60, 60, 70, 255] } else { [220, 220, 230, 255] });
                let s_ratio = (settings.dock.max_scale - 1.1) / 0.5;
                let s_fill_w = bar_w * s_ratio.clamp(0.0, 1.0);
                draw_round_rect(&mut pm, cx, scale_bar_y, s_fill_w, 8.0, 4.0, theme.accent);
                fill_circle(&mut pm, cx + s_fill_w, scale_bar_y + 4.0, 8.0, [255, 255, 255, 255]);
            }

            // 4. Indicator dots
            let ind_y = cy + 214.0;
            text.draw_bold(&mut pm, "Индикаторы открытых программ", cx, ind_y + 16.0, 12.5, fg);
            draw_toggle(&mut pm, cx + 316.0, ind_y + 4.0, settings.dock.show_indicators, theme.accent);
        }
        SettingsTab::Appearance => {
            text.draw_bold(&mut pm, "Оформление интерфейса", cx, cy + 18.0, 18.0, fg);

            // Dark card
            let card_w = 130.0;
            let card_h = 84.0;
            let dark_card_x = cx;
            let card_y = cy + 50.0;
            let is_dark = theme.dark();

            draw_round_rect(&mut pm, dark_card_x, card_y, card_w, card_h, 10.0, [24, 24, 30, 255]);
            if is_dark {
                stroke_round_rect(&mut pm, dark_card_x, card_y, card_w, card_h, 10.0, theme.accent, 2.5);
            } else {
                stroke_round_rect(&mut pm, dark_card_x, card_y, card_w, card_h, 10.0, [60, 60, 70, 180], 1.0);
            }
            // Small window mockup inside dark card
            fill_rect(&mut pm, dark_card_x + 10.0, card_y + 10.0, card_w - 20.0, 16.0, [40, 40, 48, 255]);
            text.draw_bold(&mut pm, "Темная", dark_card_x + 40.0, card_y + 60.0, 12.5, [240, 240, 245, 255]);

            // Light card
            let light_card_x = cx + 150.0;
            draw_round_rect(&mut pm, light_card_x, card_y, card_w, card_h, 10.0, [240, 240, 246, 255]);
            if !is_dark {
                stroke_round_rect(&mut pm, light_card_x, card_y, card_w, card_h, 10.0, theme.accent, 2.5);
            } else {
                stroke_round_rect(&mut pm, light_card_x, card_y, card_w, card_h, 10.0, [200, 200, 215, 255], 1.0);
            }
            // Small window mockup inside light card
            fill_rect(&mut pm, light_card_x + 10.0, card_y + 10.0, card_w - 20.0, 16.0, [220, 220, 228, 255]);
            text.draw_bold(&mut pm, "Светлая", light_card_x + 38.0, card_y + 60.0, 12.5, [40, 40, 45, 255]);

            // Accent colors section
            let ac_y = cy + 160.0;
            text.draw_bold(&mut pm, "Цвет акцента", cx, ac_y + 14.0, 12.5, fg);
            text.draw(&mut pm, "Используется для кнопок, меню и активных элементов", cx, ac_y + 28.0, 10.5, fg_dim);

            let accents: [[u8; 4]; 7] = [
                [0, 122, 255, 255],   // Blue
                [175, 82, 222, 255],  // Purple
                [255, 45, 85, 255],   // Pink
                [255, 59, 48, 255],   // Red
                [255, 149, 0, 255],   // Orange
                [52, 199, 89, 255],   // Green
                [142, 142, 147, 255], // Graphite
            ];

            for (i, &col) in accents.iter().enumerate() {
                let ax = cx + i as f32 * 38.0 + 10.0;
                let ay = ac_y + 44.0;
                let is_active = theme.accent == col;

                fill_circle(&mut pm, ax + 12.0, ay + 12.0, 12.0, col);
                if is_active {
                    stroke_circle(&mut pm, ax + 12.0, ay + 12.0, 15.0, [255, 255, 255, 220], 2.0);
                }
            }
        }
        SettingsTab::About => {
            text.draw_bold(&mut pm, "Aqua Compositor", cx, cy + 24.0, 20.0, fg);
            text.draw(&mut pm, "macOS-styled Wayland desktop environment for Linux", cx, cy + 46.0, 12.0, fg_dim);
            text.draw(&mut pm, "Версия: 0.2.0 (Sequoia Edition)", cx, cy + 80.0, 12.5, fg);
            text.draw(&mut pm, "Движок рендеринга: tiny-skia (CPU) + Smithay + DRM/KMS", cx, cy + 104.0, 12.0, fg_dim);
            text.draw(&mut pm, "Разрешение экрана: 1920 × 1080 @ 60 Гц", cx, cy + 128.0, 12.0, fg_dim);
            text.draw(&mut pm, "100% Rust • Безопасность памяти • Высокая производительность", cx, cy + 158.0, 12.0, theme.accent);
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

fn stroke_circle(pm: &mut Pixmap, cx: f32, cy: f32, r: f32, color: [u8; 4], width: f32) {
    let mut pb = PathBuilder::new();
    pb.push_circle(cx, cy, r);
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
