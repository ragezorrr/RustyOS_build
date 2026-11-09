//! Aqua shell: macOS-like menu bar, dock, wallpaper and hit testing.
//!
//! Everything here is rasterized on the CPU with `tiny-skia` and uploaded
//! to the compositor renderer as `MemoryRenderBuffer`s.

use std::path::Path;

use smithay::backend::{
    allocator::Fourcc,
    renderer::{
        element::{
            memory::{MemoryRenderBuffer, MemoryRenderBufferRenderElement},
            Kind,
        },
        gles::GlesRenderer,
    },
};
use smithay::utils::{Logical, Physical, Point, Rectangle, Transform};
use tiny_skia::{
    Color, FillRule, GradientStop, LineCap, LinearGradient, Paint, PathBuilder, Pixmap,
    SpreadMode, Stroke, Transform as SkTransform,
};

use crate::text::TextRenderer;
use crate::theme::Theme;

pub const PANEL_H: i32 = 30;
pub const DOCK_MARGIN: i32 = 4; // Dock sits almost flush with the bottom edge, macOS style
pub const DOCK_HEADROOM: i32 = 34; // Extra room above dock for tooltips and icon magnification
const ICON: i32 = 48;
const DOCK_PAD: i32 = 14;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconKind {
    Terminal,
    Files,
    Editor,
    Browser,
    Music,
    Settings,
}

#[derive(Debug, Clone, Copy)]
pub struct AppDef {
    pub id: &'static str,
    pub name: &'static str,
    pub command: &'static str,
    pub kind: IconKind,
    pub color_a: [u8; 4],
    pub color_b: [u8; 4],
}

pub const APPS: [AppDef; 6] = [
    AppDef {
        id: "dolphin",
        name: "Finder",
        command: "dolphin",
        kind: IconKind::Files,
        color_a: [205, 235, 255, 255],
        color_b: [35, 135, 245, 255],
    },
    AppDef {
        id: "firefox",
        name: "Safari",
        command: "firefox",
        kind: IconKind::Browser,
        color_a: [255, 255, 255, 255],
        color_b: [225, 235, 248, 255],
    },
    AppDef {
        id: "konsole",
        name: "Терминал",
        command: "konsole",
        kind: IconKind::Terminal,
        color_a: [52, 52, 58, 255],
        color_b: [18, 18, 22, 255],
    },
    AppDef {
        id: "kate",
        name: "Заметки",
        command: "kate",
        kind: IconKind::Editor,
        color_a: [255, 236, 140, 255],
        color_b: [248, 198, 48, 255],
    },
    AppDef {
        id: "music",
        name: "Музыка",
        command: "plasma-emojier",
        kind: IconKind::Music,
        color_a: [255, 110, 130, 255],
        color_b: [245, 45, 85, 255],
    },
    AppDef {
        id: "settings",
        name: "Настройки",
        command: "systemsettings",
        kind: IconKind::Settings,
        color_a: [225, 228, 235, 255],
        color_b: [140, 145, 155, 255],
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiAction {
    Launch(&'static str, usize),
    ActivateWindow(u32),
    RestoreMinimized(u32),
    ToggleTheme,
    OpenMenu(crate::spotlight::MenuKind, i32, i32),
    ToggleSpotlight,
    ToggleControlCenter,
    ToggleNotifications,
    ToggleWifi,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicApp {
    pub app_id: String,
    pub name: String,
    pub window_key: u32,
    pub is_minimized: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinimizedItem {
    pub key: u32,
    pub title: String,
    pub app_id: String,
}

#[derive(Debug)]
pub struct Hit {
    pub rect: Rectangle<i32, Logical>,
    pub action: UiAction,
}

struct CachedBuffer {
    buffer: MemoryRenderBuffer,
}

#[derive(Clone, PartialEq)]
struct PanelSig {
    width: i32,
    dark: bool,
    minute: i64,
    app_name: String,
    app_category: crate::spotlight::AppCategory,
    battery: u8,
    charging: bool,
    network: crate::wifi::NetworkKind,
}

const APP_COUNT: usize = 6;

#[derive(Clone, PartialEq)]
struct DockSig {
    dark: bool,
    running: [bool; APP_COUNT],
    dynamic_apps: Vec<DynamicApp>,
    minimized_items: Vec<MinimizedItem>,
    hover: Option<(i32, i32)>,
    bouncing: [f32; APP_COUNT], // bounce offset for each icon
    icon_size: i32,
    magnification: bool,
    max_scale: f32,
    show_indicators: bool,
}

#[derive(Clone, PartialEq)]
struct WallpaperSig {
    width: i32,
    height: i32,
    dark: bool,
    theme: crate::theme::WallpaperTheme,
    night_shift: bool,
}

pub struct Shell {
    pub theme: Theme,
    text: TextRenderer,

    panel: Option<CachedBuffer>,
    panel_sig: Option<PanelSig>,
    dock: Option<CachedBuffer>,
    dock_sig: Option<DockSig>,
    wallpaper: Option<CachedBuffer>,
    wallpaper_sig: Option<WallpaperSig>,

    hits: Vec<Hit>,
    pub dock_loc: (i32, i32),
    pub battery: u8,
    pub charging: bool,
    /// Bounce animation start time for each dock icon: index -> Instant
    pub icon_bounces: [Option<std::time::Instant>; APP_COUNT],
}

impl Shell {
    pub fn new(theme: Theme) -> Self {
        let (battery, charging) = read_battery();
        Self {
            theme,
            text: TextRenderer::new(),
            panel: None,
            panel_sig: None,
            dock: None,
            dock_sig: None,
            wallpaper: None,
            wallpaper_sig: None,
            hits: Vec::new(),
            dock_loc: (0, 0),
            battery,
            charging,
            icon_bounces: [None; APP_COUNT],
        }
    }

    pub fn trigger_bounce(&mut self, app_index: usize) {
        if app_index < APP_COUNT {
            self.icon_bounces[app_index] = Some(std::time::Instant::now());
        }
    }

    pub fn refresh_battery(&mut self) {
        let (battery, charging) = read_battery();
        self.battery = battery;
        self.charging = charging;
    }

    pub fn invalidate_cache(&mut self) {
        self.panel_sig = None;
        self.dock_sig = None;
        self.wallpaper_sig = None;
    }

    /// Rebuild any cached shell textures that became stale.
    pub fn ensure(
        &mut self,
        width: i32,
        height: i32,
        focused_title: Option<String>,
        focused_app_id: Option<String>,
        running: [bool; APP_COUNT],
        dynamic_apps: Vec<DynamicApp>,
        minimized_items: Vec<MinimizedItem>,
        minute: i64,
        pointer_pos: Option<(f64, f64)>,
        dock_cfg: &crate::settings::DockConfig,
        network: crate::wifi::NetworkKind,
    ) {
        let (app_name, app_category) = crate::spotlight::detect_app_category(
            focused_app_id.as_deref(),
            focused_title.as_deref(),
        );

        let panel_sig = PanelSig {
            width,
            dark: self.theme.dark(),
            minute,
            app_name: app_name.clone(),
            app_category,
            battery: self.battery,
            charging: self.charging,
            network,
        };
        if self.panel_sig.as_ref() != Some(&panel_sig) {
            self.panel = Some(CachedBuffer {
                buffer: self.build_panel(width, &app_name, app_category, minute, network),
            });
            self.panel_sig = Some(panel_sig);
        }

        let icon_size = dock_cfg.icon_size;
        let (dock_x, dock_y, dock_w, dock_h) =
            dock_rect_dynamic(width, height, icon_size, dynamic_apps.len(), minimized_items.len());
        self.dock_loc = (dock_x, dock_y);
        let dock_hover = pointer_pos.and_then(|(px, py)| {
            if px >= dock_x as f64 - 15.0
                && px < (dock_x + dock_w) as f64 + 15.0
                && py >= (dock_y - DOCK_HEADROOM) as f64
                && py < (dock_y + dock_h) as f64 + 10.0
            {
                Some((px.round() as i32 - dock_x, py.round() as i32 - (dock_y - DOCK_HEADROOM)))
            } else {
                None
            }
        });

        // Calculate bounce offsets
        let now = std::time::Instant::now();
        let mut bouncing = [0.0f32; APP_COUNT];
        let bounce_duration = std::time::Duration::from_millis(500);
        for i in 0..APP_COUNT {
            if let Some(start) = self.icon_bounces[i] {
                let elapsed = now.duration_since(start);
                if elapsed < bounce_duration {
                    let progress = elapsed.as_secs_f32() / bounce_duration.as_secs_f32();
                    // 2 bounces with decaying height: up to 14px
                    let freq = progress * std::f32::consts::PI * 2.0;
                    let height = (1.0 - progress) * 14.0;
                    bouncing[i] = (freq.sin().abs() * height).round();
                } else {
                    self.icon_bounces[i] = None;
                }
            }
        }

        let dock_sig = DockSig {
            dark: self.theme.dark(),
            running,
            dynamic_apps: dynamic_apps.clone(),
            minimized_items: minimized_items.clone(),
            hover: dock_hover,
            bouncing,
            icon_size,
            magnification: dock_cfg.magnification,
            max_scale: dock_cfg.max_scale,
            show_indicators: dock_cfg.show_indicators,
        };
        if self.dock_sig.as_ref() != Some(&dock_sig) {
            self.dock = Some(CachedBuffer {
                buffer: self.build_dock(
                    width,
                    height,
                    running,
                    &dynamic_apps,
                    &minimized_items,
                    dock_hover,
                    bouncing,
                    dock_cfg,
                ),
            });
            self.dock_sig = Some(dock_sig);
        }

        let wallpaper_sig = WallpaperSig {
            width,
            height,
            dark: self.theme.dark(),
            theme: self.theme.wallpaper_theme,
            night_shift: self.theme.night_shift,
        };
        if self.wallpaper_sig.as_ref() != Some(&wallpaper_sig) {
            self.wallpaper = Some(CachedBuffer {
                buffer: self.build_wallpaper(width, height),
            });
            self.wallpaper_sig = Some(wallpaper_sig);
        }

        self.rebuild_hits(width, height, icon_size, &dynamic_apps, &minimized_items, &app_name, app_category);
    }

    // --- render elements --------------------------------------------------

    pub fn wallpaper_element(
        &self,
        renderer: &mut GlesRenderer,
    ) -> Option<MemoryRenderBufferRenderElement<GlesRenderer>> {
        let cached = self.wallpaper.as_ref()?;
        MemoryRenderBufferRenderElement::from_buffer(
            renderer,
            Point::<f64, Physical>::from((0.0, 0.0)),
            &cached.buffer,
            None,
            None,
            None,
            Kind::Unspecified,
        )
        .ok()
    }

    pub fn panel_element(
        &self,
        renderer: &mut GlesRenderer,
    ) -> Option<MemoryRenderBufferRenderElement<GlesRenderer>> {
        let cached = self.panel.as_ref()?;
        MemoryRenderBufferRenderElement::from_buffer(
            renderer,
            Point::<f64, Physical>::from((0.0, 0.0)),
            &cached.buffer,
            None,
            None,
            None,
            Kind::Unspecified,
        )
        .ok()
    }

    pub fn dock_element(
        &self,
        renderer: &mut GlesRenderer,
        _screen_w: i32,
        _screen_h: i32,
    ) -> Option<MemoryRenderBufferRenderElement<GlesRenderer>> {
        let cached = self.dock.as_ref()?;
        let (x, y) = self.dock_loc;
        MemoryRenderBufferRenderElement::from_buffer(
            renderer,
            Point::<f64, Physical>::from((x as f64, (y - DOCK_HEADROOM) as f64)),
            &cached.buffer,
            None,
            None,
            None,
            Kind::Unspecified,
        )
        .ok()
    }

    // --- hit testing ------------------------------------------------------

    pub fn hit_test(&self, point: Point<f64, Logical>) -> Option<UiAction> {
        self.hits
            .iter()
            .find(|hit| {
                let r = hit.rect;
                point.x >= r.loc.x as f64
                    && point.y >= r.loc.y as f64
                    && point.x < (r.loc.x + r.size.w) as f64
                    && point.y < (r.loc.y + r.size.h) as f64
            })
            .map(|hit| hit.action)
    }

    /// Area reserved by the shell (menu bar and dock) for window placement and maximization.
    pub fn work_area(&self, width: i32, height: i32) -> Rectangle<i32, Logical> {
        let (_, dock_y, _, _) = dock_rect(width, height);
        // Reserve space above dock including DOCK_MARGIN and titlebar headroom
        let bottom_limit = dock_y - 10;
        let available_h = (bottom_limit - PANEL_H).max(100);
        Rectangle::new(
            Point::from((0, PANEL_H)),
            smithay::utils::Size::from((width, available_h)),
        )
    }

    fn rebuild_hits(
        &mut self,
        width: i32,
        height: i32,
        icon_size: i32,
        dynamic_apps: &[DynamicApp],
        minimized_items: &[MinimizedItem],
        app_name: &str,
        app_category: crate::spotlight::AppCategory,
    ) {
        self.hits.clear();

        // Right cluster hit test: dynamic coordinates strictly matching build_panel!
        // Clock
        let now = chrono::Local::now();
        let minute = now.timestamp() / 60;
        let clock = clock_text(minute);
        let clock_w = self.text.width_bold(&clock, 13.0);
        let clock_x = width as f32 - 12.0 - clock_w;
        let cc_x = clock_x - 22.0;
        let spot_x = cc_x - 26.0;
        let net_x = spot_x - 26.0;
        let bx = net_x - 42.0;

        // 1. Clock & Date hit area (click to toggle notifications)
        self.hits.push(Hit {
            rect: Rectangle::new(
                Point::from((clock_x as i32 - 6, 0)),
                (clock_w as i32 + 18, PANEL_H).into(),
            ),
            action: UiAction::ToggleNotifications,
        });

        // 2. Control Center icon hit area
        self.hits.push(Hit {
            rect: Rectangle::new(
                Point::from((cc_x as i32 - 11, 0)),
                (22, PANEL_H).into(),
            ),
            action: UiAction::ToggleControlCenter,
        });

        // 3. Spotlight search icon hit area
        self.hits.push(Hit {
            rect: Rectangle::new(
                Point::from((spot_x as i32 - 11, 0)),
                (22, PANEL_H).into(),
            ),
            action: UiAction::ToggleSpotlight,
        });

        // 4. Network (Wi-Fi / Ethernet) hit area (opens network menu)
        self.hits.push(Hit {
            rect: Rectangle::new(
                Point::from((net_x as i32 - 12, 0)),
                (24, PANEL_H).into(),
            ),
            action: UiAction::ToggleWifi,
        });

        // 5. Battery icon hit area
        let show_battery = (self.battery < 100 && self.charging) || (self.battery <= 95 && !self.charging);
        if show_battery {
            self.hits.push(Hit {
                rect: Rectangle::new(
                    Point::from((bx as i32 - 4, 0)),
                    (28, PANEL_H).into(),
                ),
                action: UiAction::ToggleControlCenter,
            });
        }

        // Apple menu
        self.hits.push(Hit {
            rect: Rectangle::new(Point::from((0, 0)), (32, PANEL_H).into()),
            action: UiAction::OpenMenu(crate::spotlight::MenuKind::Apple, 6, PANEL_H + 2),
        });

        // Application menus
        if width > 760 {
            let app_w = self.text.width_bold(app_name, 13.0);
            let menus = crate::spotlight::app_menu_titles(app_category);
            let mut x = 38.0 + app_w + 22.0;
            for menu in menus {
                let mw = self.text.width_bold(menu, 13.0);
                self.hits.push(Hit {
                    rect: Rectangle::new(Point::from((x as i32 - 4, 0)), (mw as i32 + 8, PANEL_H).into()),
                    action: UiAction::OpenMenu(
                        crate::spotlight::MenuKind::App {
                            category: app_category,
                            title: menu,
                        },
                        x as i32 - 8,
                        PANEL_H + 2,
                    ),
                });
                x += mw + 18.0;
            }
        }

        let gap = (icon_size as f32 * 0.28).round() as i32;
        let (dx, dy, _, _) =
            dock_rect_dynamic(width, height, icon_size, dynamic_apps.len(), minimized_items.len());

        // 1. Pinned apps
        for (i, app) in APPS.iter().enumerate() {
            let x = dx + DOCK_PAD + i as i32 * (icon_size + gap);
            let y = dy + 8;
            self.hits.push(Hit {
                rect: Rectangle::new(Point::from((x, y)), (icon_size, icon_size).into()),
                action: UiAction::Launch(app.command, i),
            });
        }

        // 2. Dynamic running apps
        for (i, dyn_app) in dynamic_apps.iter().enumerate() {
            let slot = APPS.len() + i;
            let x = dx + DOCK_PAD + slot as i32 * (icon_size + gap);
            let y = dy + 8;
            self.hits.push(Hit {
                rect: Rectangle::new(Point::from((x, y)), (icon_size, icon_size).into()),
                action: UiAction::ActivateWindow(dyn_app.window_key),
            });
        }

        // Separator offset
        let left_count = APPS.len() + dynamic_apps.len();
        let apps_w = left_count as i32 * icon_size + (left_count as i32 - 1) * gap;

        // 3. Minimized window items
        for (i, min_item) in minimized_items.iter().enumerate() {
            let x = dx + DOCK_PAD + apps_w + 16 + i as i32 * (icon_size + gap);
            let y = dy + 8;
            self.hits.push(Hit {
                rect: Rectangle::new(Point::from((x, y)), (icon_size, icon_size).into()),
                action: UiAction::RestoreMinimized(min_item.key),
            });
        }

        // 4. Folder (Downloads) and Trash hits
        let folder_x = dx + DOCK_PAD + apps_w + 16 + minimized_items.len() as i32 * (icon_size + gap);
        let trash_x = folder_x + icon_size + gap;
        self.hits.push(Hit {
            rect: Rectangle::new(Point::from((folder_x, dy + 8)), (icon_size, icon_size).into()),
            action: UiAction::Launch("dolphin", 1),
        });
        self.hits.push(Hit {
            rect: Rectangle::new(Point::from((trash_x, dy + 8)), (icon_size, icon_size).into()),
            action: UiAction::Launch("dolphin trash:/", 1),
        });
    }

    // --- textures ---------------------------------------------------------

    fn build_wallpaper(&self, width: i32, height: i32) -> MemoryRenderBuffer {
        let w = width.max(1) as u32;
        let h = height.max(1) as u32;
        let mut pm = Pixmap::new(w, h).expect("wallpaper pixmap");
        let t = self.theme.clone();
        let top = t.wallpaper_top();
        let bottom = t.wallpaper_bottom();
        let glow = t.wallpaper_glow();
        let glow2 = t.wallpaper_glow2();

        // Base gradient
        let shader = LinearGradient::new(
            tiny_skia::Point::from_xy(0.0, 0.0),
            tiny_skia::Point::from_xy(0.0, h as f32),
            vec![
                GradientStop::new(0.0, Color::from_rgba8(top[0], top[1], top[2], top[3])),
                GradientStop::new(
                    1.0,
                    Color::from_rgba8(bottom[0], bottom[1], bottom[2], bottom[3]),
                ),
            ],
            SpreadMode::Pad,
            SkTransform::identity(),
        )
        .expect("wallpaper gradient");
        let mut paint = Paint::default();
        paint.shader = shader;
        if let Some(rect) = tiny_skia::Rect::from_xywh(0.0, 0.0, w as f32, h as f32) {
            pm.fill_rect(rect, &paint, SkTransform::identity(), None);
        }

        // macOS Sequoia dark: swirling aurora ribbons
        // Large primary glow — upper-left area
        blob(&mut pm, w as f32 * 0.15, h as f32 * 0.22, 600.0, glow, 38);
        // Upper-right secondary glow
        blob(&mut pm, w as f32 * 0.80, h as f32 * 0.18, 480.0, glow2, 30);
        // Middle sweep — creates the "S-curve" illusion
        blob(&mut pm, w as f32 * 0.50, h as f32 * 0.45, 700.0, glow, 20);
        // Lower accent
        blob(&mut pm, w as f32 * 0.30, h as f32 * 0.80, 500.0, glow2, 22);
        blob(&mut pm, w as f32 * 0.75, h as f32 * 0.70, 420.0, glow, 18);

        MemoryRenderBuffer::from_slice(
            pm.data(),
            Fourcc::Abgr8888,
            (w as i32, h as i32),
            1,
            Transform::Normal,
            None,
        )
    }

    fn build_panel(
        &self,
        width: i32,
        app_name: &str,
        app_category: crate::spotlight::AppCategory,
        minute: i64,
        network: crate::wifi::NetworkKind,
    ) -> MemoryRenderBuffer {
        let w = width.max(1) as u32;
        let h = PANEL_H as u32;
        let mut pm = Pixmap::new(w, h).expect("panel pixmap");
        let t = &self.theme;
        let fg = t.menubar_fg();
        let dim = t.menubar_fg_dim();
        let _bg = t.menubar_bg();

        // Fully transparent menu bar — macOS Sequoia style (no fill, text floats cleanly on wallpaper)
        // Dark mode: 100% transparent without any bottom separator line
        // Light mode: barely-there frosted glass tint
        if !t.dark() {
            fill_rect(&mut pm, 0.0, 0.0, w as f32, h as f32, [255u8, 255, 255, 110]);
            fill_rect(&mut pm, 0.0, h as f32 - 1.0, w as f32, 1.0, [0, 0, 0, 14]);
        }

        // Apple-ish mark.
        draw_apple(&mut pm, 18.0, 15.0, fg);

        // Focused application name (Bold SF Pro).
        self.text.draw_bold(&mut pm, app_name, 38.0, 19.5, 13.0, fg);
        let app_w = self.text.width_bold(app_name, 13.0);

        // Application menus (Bold SF Pro, macOS Sequoia style).
        if width > 760 {
            let menus = crate::spotlight::app_menu_titles(app_category);
            let mut x = 38.0 + app_w + 22.0;
            for menu in menus {
                self.text.draw_bold(&mut pm, menu, x, 19.5, 13.0, fg);
                x += self.text.width_bold(menu, 13.0) + 18.0;
            }
        }

        // Right cluster, strictly matching macOS:
        // [Network]  [Spotlight]  [Control Center]  [Clock & Date]
        // Placed from right edge to left:

        // 1. Clock & Date (Right-most): e.g. "Ср 1 апр 9:41"
        let clock = clock_text(minute);
        let clock_w = self.text.width_bold(&clock, 13.0);
        let clock_x = width as f32 - 12.0 - clock_w;
        self.text.draw_bold(&mut pm, &clock, clock_x, 19.5, 13.0, fg);

        // 2. Control Center icon (to the left of clock)
        let cc_x = clock_x - 22.0;
        draw_control_center(&mut pm, cc_x, 15.0, fg);

        // 3. Spotlight search icon (to the left of control center)
        let spot_x = cc_x - 26.0;
        draw_spotlight_icon(&mut pm, spot_x, 15.0, fg);

        // 4. Network icon (Ethernet or Wi-Fi to the left of spotlight)
        let net_x = spot_x - 26.0;
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

        // 5. Battery icon (only shown when discharging or charging below 100%)
        let show_battery = (self.battery < 100 && self.charging) || (self.battery <= 95 && !self.charging);
        if show_battery {
            let bx = net_x - 42.0;
            draw_battery(&mut pm, bx, 10.0, self.battery, self.charging, fg);
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

    fn build_dock(
        &self,
        screen_w: i32,
        screen_h: i32,
        running: [bool; APP_COUNT],
        dynamic_apps: &[DynamicApp],
        minimized_items: &[MinimizedItem],
        hover: Option<(i32, i32)>,
        bouncing: [f32; APP_COUNT],
        dock_cfg: &crate::settings::DockConfig,
    ) -> MemoryRenderBuffer {
        let icon_size = dock_cfg.icon_size;
        let gap = (icon_size as f32 * 0.28).round() as i32;
        let dw = dock_width_dynamic(icon_size, dynamic_apps.len(), minimized_items.len());
        let dock_h = icon_size + 20;
        let total_dh = dock_h + DOCK_HEADROOM;
        let mut pm = Pixmap::new(dw as u32, total_dh as u32).expect("dock pixmap");
        let t = &self.theme;

        let glass_y = DOCK_HEADROOM as f32;
        let glass_h = dock_h as f32;

        // Glass background with subtle drop shadow
        for i in (0..4).rev() {
            let s_pad = (i + 1) as f32 * 1.5;
            let alpha = (12 - i * 3) as u8;
            rounded_rect(
                &mut pm,
                0.5 - s_pad,
                glass_y + 2.0,
                dw as f32 - 1.0 + s_pad * 2.0,
                glass_h - 1.0 + s_pad,
                18.0,
                [0, 0, 0, alpha],
            );
        }

        // Glass plate body
        rounded_rect(&mut pm, 0.5, glass_y + 0.5, dw as f32 - 1.0, glass_h - 1.0, 16.0, t.dock_bg());
        stroke_rounded(
            &mut pm,
            0.5,
            glass_y + 0.5,
            dw as f32 - 1.0,
            glass_h - 1.0,
            16.0,
            t.dock_border(),
            1.0,
        );
        // Top highlight
        stroke_rounded(
            &mut pm,
            1.5,
            glass_y + 1.5,
            dw as f32 - 3.0,
            glass_h - 3.0,
            15.0,
            if t.dark() {
                [255, 255, 255, 22]
            } else {
                [255, 255, 255, 90]
            },
            1.0,
        );

        let mut hovered_app = None;

        // Helper closure to calculate scale factor and tooltip hover
        let calc_scale = |cx: f32, name: &str, hovered: &mut Option<(String, f32)>| -> f32 {
            if let Some((hx, _)) = hover {
                let dist = (hx as f32 - cx).abs();
                if dist < (icon_size as f32 / 2.0) {
                    *hovered = Some((name.to_string(), cx));
                }
                if dock_cfg.magnification {
                    let max_dist = (icon_size + gap) as f32 * 1.6;
                    if dist < max_dist {
                        let factor = (1.0 - (dist / max_dist)).clamp(0.0, 1.0);
                        let bump = 0.5 * (1.0 - (factor * std::f32::consts::PI).cos());
                        return 1.0 + bump * (dock_cfg.max_scale - 1.0);
                    }
                }
            }
            1.0
        };

        // 1. Pinned applications
        for (i, app) in APPS.iter().enumerate() {
            let base_x = DOCK_PAD + i as i32 * (icon_size + gap);
            let center_x = base_x as f32 + (icon_size as f32 / 2.0);
            let scale = calc_scale(center_x, app.name, &mut hovered_app);

            let size = icon_size as f32 * scale;
            let icon_x = center_x - size / 2.0;
            let icon_bottom = glass_y + glass_h - 14.0 - bouncing[i];
            let icon_y = icon_bottom - size;

            self.draw_app_icon(&mut pm, icon_x, icon_y, size, app);

            // Running indicator dot
            if dock_cfg.show_indicators && running[i] {
                fill_circle(
                    &mut pm,
                    center_x,
                    glass_y + glass_h - 6.5,
                    2.0,
                    if t.dark() {
                        [240, 240, 245, 230]
                    } else {
                        [40, 40, 45, 200]
                    },
                );
            }
        }

        // 2. Dynamic running applications (unpinned)
        for (i, dyn_app) in dynamic_apps.iter().enumerate() {
            let slot = APPS.len() + i;
            let base_x = DOCK_PAD + slot as i32 * (icon_size + gap);
            let center_x = base_x as f32 + (icon_size as f32 / 2.0);
            let scale = calc_scale(center_x, &dyn_app.name, &mut hovered_app);

            let size = icon_size as f32 * scale;
            let icon_x = center_x - size / 2.0;
            let icon_bottom = glass_y + glass_h - 14.0;
            let icon_y = icon_bottom - size;

            self.draw_dynamic_app_icon(&mut pm, icon_x, icon_y, size, dyn_app);

            // Running indicator dot for unpinned apps
            if dock_cfg.show_indicators && !dyn_app.is_minimized {
                fill_circle(
                    &mut pm,
                    center_x,
                    glass_y + glass_h - 6.5,
                    2.0,
                    if t.dark() {
                        [240, 240, 245, 230]
                    } else {
                        [40, 40, 45, 200]
                    },
                );
            }
        }

        // Vertical separator line
        let left_count = APPS.len() + dynamic_apps.len();
        let apps_w = left_count as i32 * icon_size + (left_count as i32 - 1) * gap;
        let sep_x = (DOCK_PAD + apps_w + 8) as f32;
        let sep_color = if t.dark() {
            [255u8, 255, 255, 30]
        } else {
            [0u8, 0, 0, 35]
        };
        fill_rect(&mut pm, sep_x, glass_y + 12.0, 1.0, glass_h - 24.0, sep_color);

        // 3. Minimized window thumbnails (right of separator)
        for (i, min_item) in minimized_items.iter().enumerate() {
            let base_x = DOCK_PAD + apps_w + 16 + i as i32 * (icon_size + gap);
            let center_x = base_x as f32 + (icon_size as f32 / 2.0);
            let scale = calc_scale(center_x, &min_item.title, &mut hovered_app);

            let size = icon_size as f32 * scale;
            let icon_x = center_x - size / 2.0;
            let icon_bottom = glass_y + glass_h - 14.0;
            let icon_y = icon_bottom - size;

            self.draw_minimized_thumbnail(&mut pm, icon_x, icon_y, size, min_item);
        }

        // 4. Right side items: Downloads Folder & Trash
        let min_offset = minimized_items.len() as i32 * (icon_size + gap);
        let folder_cx = (DOCK_PAD + apps_w + 16 + min_offset) as f32 + (icon_size as f32 / 2.0);
        let trash_cx = folder_cx + (icon_size + gap) as f32;

        let right_items = [
            (folder_cx, "Загрузки", false),
            (trash_cx, "Корзина", true),
        ];

        for (cx, name, is_trash) in right_items {
            let scale = calc_scale(cx, name, &mut hovered_app);

            let size = icon_size as f32 * scale;
            let icon_x = cx - size / 2.0;
            let icon_bottom = glass_y + glass_h - 14.0;
            let icon_y = icon_bottom - size;

            if is_trash {
                draw_trash_icon(&mut pm, icon_x, icon_y, size, t.dark());
            } else {
                draw_folder_icon(&mut pm, icon_x, icon_y, size, t.dark());
            }
        }

        // Draw elegant tooltip floating cleanly ABOVE the dock glass
        if let Some((name, cx)) = hovered_app {
            let tw = self.text.width(&name, 11.0);
            let tip_w = tw + 18.0;
            let tip_h = 22.0;
            let tip_x = (cx - tip_w / 2.0).clamp(4.0, dw as f32 - tip_w - 4.0);
            // Floating 8px above the dock glass plate
            let tip_y = glass_y - tip_h - 6.0;

            let tip_bg = if t.dark() {
                [32, 32, 38, 230]
            } else {
                [248, 248, 252, 235]
            };
            // Subtle tooltip shadow
            rounded_rect(&mut pm, tip_x, tip_y + 1.0, tip_w, tip_h, 6.0, [0, 0, 0, 25]);
            rounded_rect(&mut pm, tip_x, tip_y, tip_w, tip_h, 6.0, tip_bg);
            stroke_rounded(&mut pm, tip_x, tip_y, tip_w, tip_h, 6.0, t.dock_border(), 0.8);
            self.text.draw(
                &mut pm,
                &name,
                tip_x + 9.0,
                tip_y + 15.5,
                11.0,
                t.menubar_fg(),
            );
        }

        let _ = screen_w;
        let _ = screen_h;
        MemoryRenderBuffer::from_slice(
            pm.data(),
            Fourcc::Abgr8888,
            (dw, total_dh),
            1,
            Transform::Normal,
            None,
        )
    }

    fn draw_app_icon(&self, pm: &mut Pixmap, x: f32, y: f32, size: f32, app: &AppDef) {
        let xf = x;
        let yf = y;
        let radius = 11.0 * (size / ICON as f32);

        // Drop shadow.
        for i in (0..5).rev() {
            let grow = i as f32 * 1.2;
            rounded_rect(
                pm,
                xf - grow,
                yf - grow + 2.0,
                size + grow * 2.0,
                size + grow * 2.0,
                radius + grow,
                [0, 0, 0, 10],
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
        let path = round_rect_path(xf, yf, size, size, radius);
        pm.fill_path(&path, &paint, FillRule::Winding, SkTransform::identity(), None);

        // Inner top highlight.
        stroke_rounded(
            pm,
            xf + 1.0,
            yf + 1.0,
            size - 2.0,
            size - 2.0,
            (radius - 1.0).max(1.0),
            [255, 255, 255, 40],
            1.0,
        );

        let scale = size / ICON as f32;

        match app.kind {
            IconKind::Terminal => {
                let glyph = ">_";
                let font_size = 19.0 * scale;
                let gw = self.text.width(glyph, font_size);
                self.text.draw(
                    pm,
                    glyph,
                    xf + (size - gw) / 2.0,
                    yf + size / 2.0 + 6.5 * scale,
                    font_size,
                    [255, 255, 255, 255],
                );
            }
            IconKind::Files => {
                // macOS Finder smiling two-tone face
                let cx = xf + size / 2.0;
                let cy = yf + size / 2.0;
                // Face divider line
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
                // Eyes
                fill_circle(pm, cx - 8.0 * scale, cy - 4.0 * scale, 2.5 * scale, [20, 60, 130, 240]);
                fill_circle(pm, cx + 8.0 * scale, cy - 4.0 * scale, 2.5 * scale, [20, 60, 130, 240]);
                // Big smile
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
                // macOS Notes lined yellow pad
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
                // Musical double eighth note
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
                // macOS Gear wheel
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
                // Compass.
                stroke_circle(pm, xf + size / 2.0, yf + size / 2.0, 13.0 * scale, [255, 255, 255, 250], 2.0 * scale);
                // Needle.
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

    fn draw_dynamic_app_icon(&self, pm: &mut Pixmap, xf: f32, yf: f32, size: f32, app: &DynamicApp) {
        let radius = 11.0 * (size / ICON as f32);

        // Drop shadow
        for i in (0..5).rev() {
            let grow = i as f32 * 1.2;
            rounded_rect(
                pm,
                xf - grow,
                yf - grow + 2.0,
                size + grow * 2.0,
                size + grow * 2.0,
                radius + grow,
                [0, 0, 0, 10],
            );
        }

        // Color based on app_id
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
        let path = round_rect_path(xf, yf, size, size, radius);
        pm.fill_path(&path, &paint, FillRule::Winding, SkTransform::identity(), None);

        // Glass rim highlight
        stroke_rounded(pm, xf + 1.0, yf + 1.0, size - 2.0, size - 2.0, (radius - 1.0).max(1.0), [255, 255, 255, 45], 1.0);

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
            let lw = self.text.width_bold(&letter, font_size);
            self.text.draw_bold(
                pm,
                &letter,
                xf + (size - lw) / 2.0,
                yf + size / 2.0 + 6.5 * scale,
                font_size,
                [255, 255, 255, 255],
            );
        }
    }

    fn draw_minimized_thumbnail(&self, pm: &mut Pixmap, xf: f32, yf: f32, size: f32, _item: &MinimizedItem) {
        let scale = size / ICON as f32;
        let w = size * 0.94;
        let h = size * 0.74;
        let rx = xf + (size - w) / 2.0;
        let ry = yf + size - h - 2.0;
        let r = 7.0 * scale;

        // Shadow
        for i in (0..4).rev() {
            let grow = i as f32 * 1.5;
            rounded_rect(pm, rx - grow, ry - grow + 2.0, w + grow * 2.0, h + grow * 2.0, r + grow, [0, 0, 0, 15]);
        }

        // Window plate body
        let body_col = if self.theme.dark() { [34u8, 36, 44, 245] } else { [245u8, 246, 250, 245] };
        rounded_rect(pm, rx, ry, w, h, r, body_col);
        stroke_rounded(pm, rx, ry, w, h, r, [255, 255, 255, 35], 1.0);

        // Titlebar
        let tb_h = 10.0 * scale;
        let tb_col = if self.theme.dark() { [28u8, 30, 38, 255] } else { [230u8, 232, 238, 255] };
        rounded_rect(pm, rx, ry, w, tb_h, r, tb_col);
        fill_rect(pm, rx, ry + tb_h - 1.0, w, 1.0, [255, 255, 255, 15]);

        // Mini traffic lights (3 tiny dots)
        fill_circle(pm, rx + 4.5 * scale, ry + 5.0 * scale, 1.6 * scale, [255, 95, 87, 255]);
        fill_circle(pm, rx + 8.5 * scale, ry + 5.0 * scale, 1.6 * scale, [254, 188, 46, 255]);
        fill_circle(pm, rx + 12.5 * scale, ry + 5.0 * scale, 1.6 * scale, [40, 200, 64, 255]);

        // Content lines placeholder
        let line_col = if self.theme.dark() { [255, 255, 255, 18] } else { [0, 0, 0, 18] };
        fill_rect(pm, rx + 6.0 * scale, ry + 15.0 * scale, w - 12.0 * scale, 2.0 * scale, line_col);
        fill_rect(pm, rx + 6.0 * scale, ry + 20.0 * scale, (w - 12.0 * scale) * 0.7, 2.0 * scale, line_col);
        fill_rect(pm, rx + 6.0 * scale, ry + 25.0 * scale, (w - 12.0 * scale) * 0.5, 2.0 * scale, line_col);
    }
}

// --- geometry helpers -----------------------------------------------------

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

/// Top-left corner and size of the dock.
pub fn dock_rect_with_size(screen_w: i32, screen_h: i32, icon_size: i32) -> (i32, i32, i32, i32) {
    dock_rect_dynamic(screen_w, screen_h, icon_size, 0, 0)
}

pub fn dock_rect(screen_w: i32, screen_h: i32) -> (i32, i32, i32, i32) {
    dock_rect_with_size(screen_w, screen_h, ICON)
}

fn round_rect_path(x: f32, y: f32, w: f32, h: f32, r: f32) -> tiny_skia::Path {
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
    pb.finish().expect("round rect path")
}

fn fill_rect(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, color: [u8; 4]) {
    let mut paint = Paint::default();
    paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
    if let Some(r) = tiny_skia::Rect::from_xywh(x, y, w, h) {
        pm.fill_rect(r, &paint, SkTransform::identity(), None);
    }
}

fn rounded_rect(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, r: f32, color: [u8; 4]) {
    let path = round_rect_path(x, y, w, h, r);
    let mut paint = Paint::default();
    paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
    pm.fill_path(&path, &paint, FillRule::Winding, SkTransform::identity(), None);
}

fn stroke_rounded(
    pm: &mut Pixmap,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    r: f32,
    color: [u8; 4],
    width: f32,
) {
    let path = round_rect_path(x, y, w, h, r);
    let mut paint = Paint::default();
    paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
    let stroke = Stroke {
        width,
        line_cap: LineCap::Round,
        ..Default::default()
    };
    pm.stroke_path(&path, &paint, &stroke, SkTransform::identity(), None);
}

fn fill_circle(pm: &mut Pixmap, cx: f32, cy: f32, r: f32, color: [u8; 4]) {
    let mut pb = PathBuilder::new();
    pb.push_circle(cx, cy, r);
    if let Some(p) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
        pm.fill_path(&p, &paint, FillRule::Winding, SkTransform::identity(), None);
    }
}

fn stroke_circle(pm: &mut Pixmap, cx: f32, cy: f32, r: f32, color: [u8; 4], width: f32) {
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

fn blob(pm: &mut Pixmap, cx: f32, cy: f32, r: f32, color: [u8; 4], alpha: u8) {
    // Layered circles fake a soft radial falloff without needing a blur filter.
    let steps = 14;
    for i in (0..steps).rev() {
        let t = i as f32 / steps as f32;
        let rr = r * (0.35 + 0.65 * t);
        let a = (alpha as f32 * (1.0 - t) * 0.35).max(1.0) as u8;
        fill_circle(pm, cx, cy, rr, [color[0], color[1], color[2], a]);
    }
}

// --- glyphs ---------------------------------------------------------------

fn draw_folder_icon(pm: &mut Pixmap, x: f32, y: f32, size: f32, _dark: bool) {
    let scale = size / ICON as f32;
    // Blue translucent macOS folder body
    rounded_rect(pm, x + 2.0 * scale, y + 8.0 * scale, 44.0 * scale, 34.0 * scale, 8.0 * scale, [45, 140, 245, 235]);
    // Folder tab
    rounded_rect(pm, x + 4.0 * scale, y + 4.0 * scale, 18.0 * scale, 10.0 * scale, 4.0 * scale, [35, 125, 230, 240]);
    // Folder front flap highlight
    rounded_rect(pm, x + 2.0 * scale, y + 16.0 * scale, 44.0 * scale, 26.0 * scale, 7.0 * scale, [70, 165, 255, 245]);
    stroke_rounded(pm, x + 2.0 * scale, y + 16.0 * scale, 44.0 * scale, 26.0 * scale, 7.0 * scale, [255, 255, 255, 80], 1.0 * scale);
}

fn draw_trash_icon(pm: &mut Pixmap, x: f32, y: f32, size: f32, dark: bool) {
    let scale = size / ICON as f32;
    let bg = if dark { [160, 160, 175, 140] } else { [200, 200, 215, 160] };
    // Trash bin body
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
    // Trash lid
    rounded_rect(pm, x + 8.0 * scale, y + 8.0 * scale, 32.0 * scale, 4.0 * scale, 2.0 * scale, [230, 230, 240, 220]);
}

fn draw_spotlight_icon(pm: &mut Pixmap, cx: f32, cy: f32, color: [u8; 4]) {
    // macOS Sequoia Spotlight magnifying glass: 45 degree angle, clean handle
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

fn draw_apple(pm: &mut Pixmap, cx: f32, cy: f32, color: [u8; 4]) {
    // Apple logo in macOS Sequoia: authentic silhouette
    // Left lobe & Right lobe via cubic bezier
    let mut pb = PathBuilder::new();
    let top = cy - 6.5;
    let bot = cy + 7.0;
    
    // Apple body: smooth contour with dimple at top & bottom and bite on right
    pb.move_to(cx, top + 1.8);
    // Top-left shoulder
    pb.cubic_to(cx - 3.8, top - 0.2, cx - 7.5, top + 3.0, cx - 7.5, cy + 1.0);
    // Left waist & bottom lobe
    pb.cubic_to(cx - 7.5, cy + 4.8, cx - 4.5, bot, cx - 1.8, bot);
    // Bottom dimple
    pb.cubic_to(cx - 0.8, bot, cx, bot - 0.8, cx + 0.5, bot);
    pb.cubic_to(cx + 1.5, bot, cx + 4.5, cy + 5.5, cx + 7.5, cy + 1.0);
    // Right side bite
    pb.cubic_to(cx + 5.8, cy + 0.6, cx + 4.8, cy - 1.5, cx + 5.6, cy - 3.5);
    // Top-right shoulder back to top dimple
    pb.cubic_to(cx + 4.2, top + 0.5, cx + 2.0, top + 1.0, cx, top + 1.8);
    pb.close();

    if let Some(body) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
        pm.fill_path(&body, &paint, FillRule::Winding, SkTransform::identity(), None);
    }

    // Leaf (crescent angle pointing top-right)
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

fn draw_control_center(pm: &mut Pixmap, cx: f32, cy: f32, color: [u8; 4]) {
    // macOS Sequoia Control Center icon: two rounded capsules with toggle knobs
    let w = 14.5;
    let h = 5.4;
    let r = 2.7;

    // Top pill (knob on right)
    let y1 = cy - 3.8;
    stroke_rounded(pm, cx - w / 2.0, y1 - h / 2.0, w, h, r, color, 1.2);
    fill_circle(pm, cx + 2.7, y1, 1.7, color);

    // Bottom pill (knob on left)
    let y2 = cy + 3.8;
    stroke_rounded(pm, cx - w / 2.0, y2 - h / 2.0, w, h, r, color, 1.2);
    fill_circle(pm, cx - 2.7, y2, 1.7, color);
}

fn draw_battery(
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

fn fill_rounded_rect(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, r: f32, color: [u8; 4]) {
    rounded_rect(pm, x, y, w, h, r, color);
}

fn draw_arc(
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
            line_cap: tiny_skia::LineCap::Round,
            line_join: tiny_skia::LineJoin::Round,
            ..Default::default()
        };
        pm.stroke_path(&path, &paint, &stroke, SkTransform::identity(), None);
    }
}

fn draw_wifi(pm: &mut Pixmap, cx: f32, cy: f32, color: [u8; 4]) {
    // macOS Sequoia Wi-Fi symbol: 1 base dot + 2 concentric smooth circular arcs
    let base_y = cy + 3.6;
    fill_circle(pm, cx, base_y, 1.3, color);
    draw_arc(pm, cx, base_y, 5.0, -132.0, -48.0, color, 1.5);
    draw_arc(pm, cx, base_y, 9.2, -135.0, -45.0, color, 1.5);
}

fn draw_ethernet(pm: &mut Pixmap, cx: f32, cy: f32, color: [u8; 4]) {
    // macOS Sequoia Ethernet symbol: <···>
    let mut paint = Paint::default();
    paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
    let stroke = Stroke {
        width: 1.5,
        line_cap: tiny_skia::LineCap::Round,
        line_join: tiny_skia::LineJoin::Round,
        ..Default::default()
    };

    // Left chevron <
    let mut pb_l = PathBuilder::new();
    pb_l.move_to(cx - 5.0, cy - 3.4);
    pb_l.line_to(cx - 8.0, cy);
    pb_l.line_to(cx - 5.0, cy + 3.4);
    if let Some(p) = pb_l.finish() {
        pm.stroke_path(&p, &paint, &stroke, SkTransform::identity(), None);
    }

    // 3 Dots in the middle
    fill_circle(pm, cx - 2.5, cy, 1.1, color);
    fill_circle(pm, cx, cy, 1.1, color);
    fill_circle(pm, cx + 2.5, cy, 1.1, color);

    // Right chevron >
    let mut pb_r = PathBuilder::new();
    pb_r.move_to(cx + 5.0, cy - 3.4);
    pb_r.line_to(cx + 8.0, cy);
    pb_r.line_to(cx + 5.0, cy + 3.4);
    if let Some(p) = pb_r.finish() {
        pm.stroke_path(&p, &paint, &stroke, SkTransform::identity(), None);
    }
}

// --- system info ----------------------------------------------------------

fn read_battery() -> (u8, bool) {
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

fn clock_text(minute: i64) -> String {
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
