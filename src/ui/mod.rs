//! Aqua shell: macOS-like menu bar, dock, wallpaper and hit testing.
//!
//! Everything here is rasterized on the CPU with `tiny-skia` and uploaded
//! to the compositor renderer as `MemoryRenderBuffer`s.

pub mod dock;
pub mod panel;
pub mod shapes;

pub use dock::*;
pub use panel::*;
pub use shapes::*;

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
    Color, GradientStop, LinearGradient, Paint, Pixmap,
    SpreadMode, Transform as SkTransform,
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
    CycleKeyboardLayout,
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
    layout_name: String,
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
    dock_plate: Option<(i32, i32, bool, Pixmap)>,
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
            dock_plate: None,
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
        self.dock_plate = None;
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
        layout_name: &str,
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
            layout_name: layout_name.to_string(),
        };
        if self.panel_sig.as_ref() != Some(&panel_sig) {
            self.panel = Some(CachedBuffer {
                buffer: self.build_panel(width, &app_name, app_category, minute, network, layout_name),
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
                let raw_x = px.round() as i32 - dock_x;
                let raw_y = py.round() as i32 - (dock_y - DOCK_HEADROOM);
                // Quantize hover coordinates to 2px grid to eliminate subpixel micro-jitter
                let q_x = (raw_x / 2) * 2;
                let q_y = (raw_y / 2) * 2;
                Some((q_x, q_y))
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

        self.rebuild_hits(width, height, icon_size, &dynamic_apps, &minimized_items, &app_name, app_category, layout_name);
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
    pub fn work_area(&self, width: i32, height: i32, icon_size: i32) -> Rectangle<i32, Logical> {
        let (_, dock_y, _, _) = dock_rect_with_size(width, height, icon_size);
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
        layout_name: &str,
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

        let lang_code = if layout_name.to_lowercase().contains("ru") || layout_name.contains("Рус") {
            "RU"
        } else {
            "US"
        };
        let lang_w = self.text.width_bold(lang_code, 12.0);
        let lang_x = spot_x - 14.0 - lang_w;
        let net_x = lang_x - 22.0;
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

        // 4. Keyboard Layout indicator hit area (click to cycle layout)
        self.hits.push(Hit {
            rect: Rectangle::new(
                Point::from((lang_x as i32 - 6, 0)),
                (lang_w as i32 + 12, PANEL_H).into(),
            ),
            action: UiAction::CycleKeyboardLayout,
        });

        // 5. Network (Wi-Fi / Ethernet) hit area (opens network menu)
        self.hits.push(Hit {
            rect: Rectangle::new(
                Point::from((net_x as i32 - 12, 0)),
                (24, PANEL_H).into(),
            ),
            action: UiAction::ToggleWifi,
        });

        // 6. Battery icon hit area
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
        layout_name: &str,
    ) -> MemoryRenderBuffer {
        crate::ui::panel::build_panel(
            &self.theme,
            &self.text,
            self.battery,
            self.charging,
            width,
            app_name,
            app_category,
            minute,
            network,
            layout_name,
        )
    }

    fn ensure_dock_plate(&mut self, dw: i32, icon_size: i32, dark: bool, dynamic_len: usize) -> &Pixmap {
        let needs_rebuild = match &self.dock_plate {
            Some((w, sz, d, _)) => *w != dw || *sz != icon_size || *d != dark,
            None => true,
        };
        if needs_rebuild {
            let dock_h = icon_size + 20;
            let total_dh = dock_h + DOCK_HEADROOM;
            let mut pm = Pixmap::new(dw as u32, total_dh as u32).expect("dock plate pixmap");
            let t = &self.theme;

            let glass_y = DOCK_HEADROOM as f32;
            let glass_h = dock_h as f32;
            let plate_radius = (icon_size as f32 * 0.36).round().clamp(14.0, 24.0);

            // Multi-layer physical soft drop shadow for elevation
            let shadow_steps = 6;
            for i in (0..shadow_steps).rev() {
                let s_pad = (i + 1) as f32 * 1.6;
                let y_off = 2.0 + (i as f32 * 1.1);
                let alpha = match i {
                    0 => 22,
                    1 => 16,
                    2 => 11,
                    3 => 7,
                    4 => 4,
                    _ => 2,
                };
                squircle_rect(
                    &mut pm,
                    0.5 - s_pad,
                    glass_y + y_off,
                    dw as f32 - 1.0 + s_pad * 2.0,
                    glass_h - 1.0 + s_pad,
                    plate_radius + s_pad,
                    [0, 0, 0, alpha],
                );
            }

            // Glass plate body with frosted appearance
            squircle_rect(
                &mut pm,
                0.5,
                glass_y + 0.5,
                dw as f32 - 1.0,
                glass_h - 1.0,
                plate_radius,
                t.dock_bg(),
            );

            // Perimeter border
            stroke_squircle(
                &mut pm,
                0.5,
                glass_y + 0.5,
                dw as f32 - 1.0,
                glass_h - 1.0,
                plate_radius,
                t.dock_border(),
                1.0,
            );

            // Top edge inner specular rim highlight (Sequoia glass)
            stroke_squircle(
                &mut pm,
                1.5,
                glass_y + 1.5,
                dw as f32 - 3.0,
                glass_h - 3.0,
                (plate_radius - 1.0).max(1.0),
                t.dock_top_rim(),
                1.0,
            );

            // Bottom bounce specular reflection hairline
            let bottom_rim_col = if t.dark() {
                [255, 255, 255, 12]
            } else {
                [255, 255, 255, 60]
            };
            fill_rect(
                &mut pm,
                plate_radius * 0.8,
                glass_y + glass_h - 2.0,
                dw as f32 - plate_radius * 1.6,
                1.0,
                bottom_rim_col,
            );

            // Vertical separator line (dual-line etched groove)
            let gap = (icon_size as f32 * 0.28).round() as i32;
            let left_count = APPS.len() + dynamic_len;
            let apps_w = left_count as i32 * icon_size + (left_count as i32 - 1) * gap;
            let sep_x = (DOCK_PAD + apps_w + 8) as f32;
            let (sep_dark, sep_light) = t.dock_separator();
            fill_rect(&mut pm, sep_x, glass_y + 11.0, 1.0, glass_h - 22.0, sep_dark);
            fill_rect(&mut pm, sep_x + 1.0, glass_y + 11.0, 1.0, glass_h - 22.0, sep_light);

            self.dock_plate = Some((dw, icon_size, dark, pm));
        }
        &self.dock_plate.as_ref().unwrap().3
    }

    fn build_dock(
        &mut self,
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
        let dark = self.theme.dark();

        let plate = self.ensure_dock_plate(dw, icon_size, dark, dynamic_apps.len());
        let mut pm = plate.clone();
        let t = &self.theme;

        let glass_y = DOCK_HEADROOM as f32;
        let glass_h = dock_h as f32;

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

            crate::ui::dock::draw_app_icon(&self.text, &mut pm, icon_x, icon_y, size, app);

            // Running indicator dot
            if dock_cfg.show_indicators && running[i] {
                // Subtle dot glow
                fill_circle(
                    &mut pm,
                    center_x,
                    glass_y + glass_h - 6.5,
                    3.2,
                    t.dock_indicator_glow(),
                );
                // Crisp core dot
                fill_circle(
                    &mut pm,
                    center_x,
                    glass_y + glass_h - 6.5,
                    2.0,
                    t.dock_indicator(),
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

            crate::ui::dock::draw_dynamic_app_icon(&self.text, &mut pm, icon_x, icon_y, size, dyn_app);

            // Running indicator dot for unpinned apps
            if dock_cfg.show_indicators && !dyn_app.is_minimized {
                fill_circle(
                    &mut pm,
                    center_x,
                    glass_y + glass_h - 6.5,
                    3.2,
                    t.dock_indicator_glow(),
                );
                fill_circle(
                    &mut pm,
                    center_x,
                    glass_y + glass_h - 6.5,
                    2.0,
                    t.dock_indicator(),
                );
            }
        }

        // 3. Minimized window thumbnails (right of separator)
        let left_count = APPS.len() + dynamic_apps.len();
        let apps_w = left_count as i32 * icon_size + (left_count as i32 - 1) * gap;
        for (i, min_item) in minimized_items.iter().enumerate() {
            let base_x = DOCK_PAD + apps_w + 16 + i as i32 * (icon_size + gap);
            let center_x = base_x as f32 + (icon_size as f32 / 2.0);
            let scale = calc_scale(center_x, &min_item.title, &mut hovered_app);

            let size = icon_size as f32 * scale;
            let icon_x = center_x - size / 2.0;
            let icon_bottom = glass_y + glass_h - 14.0;
            let icon_y = icon_bottom - size;

            crate::ui::dock::draw_minimized_thumbnail(&self.theme, &mut pm, icon_x, icon_y, size, min_item);
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
            let tw = self.text.width(&name, 11.5);
            let tip_w = (tw + 20.0).max(36.0);
            let tip_h = 24.0;
            let tip_x = (cx - tip_w / 2.0).clamp(4.0, dw as f32 - tip_w - 4.0);
            // Floating 8px above the dock glass plate
            let tip_y = glass_y - tip_h - 7.0;

            let tip_bg = t.tooltip_bg();
            // Layered tooltip shadow
            squircle_rect(&mut pm, tip_x, tip_y + 1.5, tip_w, tip_h, 7.0, [0, 0, 0, 30]);
            squircle_rect(&mut pm, tip_x, tip_y + 0.5, tip_w, tip_h, 7.0, [0, 0, 0, 18]);
            squircle_rect(&mut pm, tip_x, tip_y, tip_w, tip_h, 7.0, tip_bg);
            stroke_squircle(&mut pm, tip_x, tip_y, tip_w, tip_h, 7.0, t.dock_border(), 0.8);
            // Tooltip top edge highlight
            stroke_squircle(
                &mut pm,
                tip_x + 0.5,
                tip_y + 0.5,
                tip_w - 1.0,
                tip_h - 1.0,
                6.5,
                if t.dark() { [255, 255, 255, 30] } else { [255, 255, 255, 90] },
                0.8,
            );
            self.text.draw(
                &mut pm,
                &name,
                tip_x + 10.0,
                tip_y + 16.5,
                11.5,
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
}
