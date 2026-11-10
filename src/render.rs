//! Scene assembly and server side decorations for Aqua.
//!
//! We build the frame ourselves instead of using `Space::render_output`, which
//! lets us interleave a per-window shadow, border and macOS traffic lights with
//! the client surfaces.

use std::collections::HashMap;

use smithay::{
    backend::{
        allocator::Fourcc,
        renderer::{
            element::{
                memory::{MemoryRenderBuffer, MemoryRenderBufferRenderElement},
                surface::WaylandSurfaceRenderElement,
                AsRenderElements, Kind,
            },
            gles::{GlesRenderer, GlesTarget},
            damage::OutputDamageTracker,
            ImportAll, ImportMem,
        },
    },
    desktop::Window,
    output::Output,
    utils::{Logical, Physical, Point, Rectangle, Scale, Transform},
    wayland::compositor::with_states,
    wayland::shell::xdg::XdgToplevelSurfaceData,
    reexports::wayland_server::Resource,
};

use crate::text::TextRenderer;
use crate::theme::Theme;
use crate::ui;

smithay::backend::renderer::element::render_elements! {
    pub AquaRenderElement<R> where R: ImportAll + ImportMem;
    Surface=WaylandSurfaceRenderElement<R>,
    Memory=MemoryRenderBufferRenderElement<R>,
}

pub const TITLEBAR_H: i32 = 32;
pub const SHADOW_PAD: i32 = 22;
pub const RADIUS: f32 = 11.0;

struct Deco {
    w: i32,
    h: i32,
    dark: bool,
    title: Option<String>,
    focused: bool,
    is_csd: bool,
    /// Shadow + window background + titlebar + outer border, drawn *behind* the client.
    frame: MemoryRenderBuffer,
    /// Thin rounded border drawn *above* the client, so edges stay crisp.
    border: MemoryRenderBuffer,
}

pub struct DecoCache {
    windows: HashMap<u32, Deco>,
    lights: HashMap<(bool, bool, bool), MemoryRenderBuffer>,
    pub text: TextRenderer,
    cursor: Option<MemoryRenderBuffer>,
}

impl Default for DecoCache {
    fn default() -> Self {
        Self {
            windows: HashMap::new(),
            lights: HashMap::new(),
            text: TextRenderer::new(),
            cursor: None,
        }
    }
}

impl DecoCache {
    pub fn remove(&mut self, key: u32) {
        self.windows.remove(&key);
    }

    pub fn cursor(&mut self) -> &MemoryRenderBuffer {
        self.cursor.get_or_insert_with(default_cursor_buffer)
    }

    fn deco(&mut self, key: u32, w: i32, h: i32, title: Option<&str>, focused: bool, is_csd: bool, theme: &Theme) -> &Deco {
        let title_string = title.map(|s| s.to_string());
        let stale = match self.windows.get(&key) {
            Some(d) => d.w != w || d.h != h || d.dark != theme.dark() || d.title != title_string || d.focused != focused || d.is_csd != is_csd,
            None => true,
        };
        if stale {
            self.windows
                .insert(key, build_deco(w, h, title, focused, is_csd, theme, &self.text));
        }
        self.windows.get(&key).unwrap()
    }

    fn lights(&mut self, dark: bool, focused: bool, hovered: bool) -> &MemoryRenderBuffer {
        self.lights
            .entry((dark, focused, hovered))
            .or_insert_with(|| build_lights(dark, focused, hovered))
    }
}

pub fn window_key(window: &Window) -> Option<u32> {
    Some(window.toplevel()?.wl_surface().id().protocol_id())
}

pub fn window_title(window: &Window) -> Option<String> {
    let surface = window.toplevel()?.wl_surface();
    with_states(surface, |states| {
        states
            .data_map
            .get::<XdgToplevelSurfaceData>()
            .and_then(|data| data.lock().ok())
            .and_then(|attrs| attrs.title.clone())
    })
}

pub fn window_app_id(window: &Window) -> Option<String> {
    let surface = window.toplevel()?.wl_surface();
    with_states(surface, |states| {
        states
            .data_map
            .get::<XdgToplevelSurfaceData>()
            .and_then(|data| data.lock().ok())
            .and_then(|attrs| attrs.app_id.clone())
    })
}

pub fn window_is_csd(window: &Window) -> bool {
    if let Some(toplevel) = window.toplevel() {
        if toplevel.current_state().decoration_mode
            == Some(smithay::reexports::wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode::ClientSide)
        {
            return true;
        }
    }
    let app_id = window_app_id(window).unwrap_or_default().to_lowercase();
    let title = window_title(window).unwrap_or_default().to_lowercase();
    app_id.contains("cursor")
        || app_id.contains("code")
        || app_id.contains("chrome")
        || app_id.contains("chromium")
        || app_id.contains("brave")
        || app_id.contains("firefox")
        || app_id.contains("edge")
        || title.contains("cursor")
        || title.contains("visual studio code")
        || title.contains("google chrome")
        || title.contains("chromium")
        || title.contains("brave")
        || title.contains("mozilla firefox")
}

pub fn window_is_fullscreen(window: &Window) -> bool {
    if let Some(toplevel) = window.toplevel() {
        toplevel.current_state().states.contains(smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State::Fullscreen)
    } else {
        false
    }
}

pub fn window_titlebar_h(window: &Window) -> i32 {
    if window_is_csd(window) || window_is_fullscreen(window) {
        0
    } else {
        TITLEBAR_H
    }
}

/// Which traffic light is under `point` (screen coordinates)?
pub fn traffic_light_at(window_loc: Point<i32, Logical>, point: Point<f64, Logical>) -> Option<Light> {
    let origin = Point::<f64, Logical>::from((
        window_loc.x as f64 + 14.0,
        window_loc.y as f64 + ((TITLEBAR_H - 14) / 2) as f64,
    ));
    let local = point - origin;
    if local.y < -4.0 || local.y > 18.0 {
        return None;
    }
    if local.x >= 0.0 && local.x < 16.0 {
        Some(Light::Close)
    } else if local.x >= 16.0 && local.x < 36.0 {
        Some(Light::Minimize)
    } else if local.x >= 36.0 && local.x < 56.0 {
        Some(Light::Maximize)
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Light {
    Close,
    Minimize,
    Maximize,
}

#[allow(clippy::too_many_arguments)]
pub fn render(
    state: &mut crate::Aqua,
    renderer: &mut GlesRenderer,
    framebuffer: &mut GlesTarget<'_>,
    output: &Output,
    damage_tracker: &mut OutputDamageTracker,
) -> Result<(), Box<dyn std::error::Error>> {
    let geometry = state
        .space
        .output_geometry(output)
        .ok_or("output has no geometry")?;
    let scale = output.current_scale().fractional_scale();
    let scale_s = Scale::from(scale);
    let dark = state.shell.theme.dark();

    let windows: Vec<Window> = state.space.elements_for_output(output).cloned().collect();
    let focused_key = state.focused_window.as_ref().and_then(window_key);
    let focused_title = state.focused_window.as_ref().and_then(window_title);
    let focused_app_id = state.focused_window.as_ref().and_then(window_app_id);
    let mut all_windows = windows.clone();
    for (min_win, _) in state.minimized_windows.values() {
        all_windows.push(min_win.clone());
    }
    let running = running_apps(&all_windows);
    let minute = chrono::Local::now().timestamp() / 60;

    let pointer_pos = state.seat.get_pointer().map(|p| {
        let loc = p.current_location();
        (loc.x, loc.y)
    });

    let network_kind = state.wifi_menu.network_kind();

    // 1. Dynamic running apps (open windows not covered by pinned APPS)
    let mut dynamic_apps = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();

    for window in &windows {
        let app_id = window_app_id(window).unwrap_or_else(|| "App".into());
        let app_id_lower = app_id.to_lowercase();
        let title = window_title(window).unwrap_or_else(|| "Окно".into());
        let title_lower = title.to_lowercase();

        let is_pinned = ui::APPS.iter().any(|app| {
            let id = app.id.to_lowercase();
            let name = app.name.to_lowercase();
            app_id_lower.contains(&id)
                || app_id_lower.contains(&name)
                || title_lower.contains(&id)
                || title_lower.contains(&name)
        });

        if !is_pinned && seen_ids.insert(app_id.clone()) {
            let key = window_key(window).unwrap_or(0);
            dynamic_apps.push(ui::DynamicApp {
                app_id: app_id.clone(),
                name: title.clone(),
                window_key: key,
                is_minimized: false,
            });
        }
    }

    // 2. Minimized items (displayed in dock tray)
    let mut minimized_items = Vec::new();
    for (&key, (win, _)) in &state.minimized_windows {
        let title = window_title(win).unwrap_or_else(|| "Окно".into());
        let app_id = window_app_id(win).unwrap_or_else(|| "App".into());
        minimized_items.push(ui::MinimizedItem {
            key,
            title,
            app_id,
        });
    }

    let layout_name = state.current_layout_name();

    state.shell.ensure(
        geometry.size.w,
        geometry.size.h,
        focused_title,
        focused_app_id,
        running,
        dynamic_apps,
        minimized_items,
        minute,
        pointer_pos,
        &state.settings.dock,
        network_kind,
        &layout_name,
    );

    let mut elements: Vec<AquaRenderElement<GlesRenderer>> = Vec::new();

    if let Some(elem) = state.shell.wallpaper_element(renderer) {
        elements.push(AquaRenderElement::Memory(elem));
    }

    let now = std::time::Instant::now();

    for window in &windows {
        let Some(mut loc) = state.space.element_location(window) else {
            continue;
        };
        let size = window.geometry().size;
        let key = window_key(window).unwrap_or(0);
        let focused = focused_key == Some(key);
        let title = window_title(window);
        let is_csd = window_is_csd(window);
        let titlebar_h = window_titlebar_h(window);

        // Alpha / Scale animation calculation
        let mut alpha = 1.0f32;

        if let Some(&start) = state.window_open_animations.get(&key) {
            let elapsed = now.duration_since(start).as_millis() as f32;
            let progress = (elapsed / 200.0).clamp(0.0, 1.0);
            // Ease out cubic
            let ease = 1.0 - (1.0 - progress).powi(3);
            alpha = (0.2 + ease * 0.8).clamp(0.0, 1.0);
            // Gentle slide-in upward by 10px
            loc.y += ((1.0 - ease) * 10.0).round() as i32;
        } else if let Some(&(start, _)) = state.window_close_animations.get(&key) {
            let elapsed = now.duration_since(start).as_millis() as f32;
            let progress = (elapsed / 160.0).clamp(0.0, 1.0);
            alpha = (1.0 - progress).clamp(0.0, 1.0);
            // Gentle slide-down / zoom-out slightly
            loc.y += (progress * 8.0).round() as i32;
        }

        let is_fullscreen = window_is_fullscreen(window);

        // Shadow, background, titlebar and outer border (behind the client).
        if !is_csd && !is_fullscreen {
            let deco = state.deco.deco(key, size.w, size.h, title.as_deref(), focused, is_csd, &state.shell.theme);
            let pos = Point::<f64, Physical>::from((
                (loc.x - SHADOW_PAD) as f64,
                (loc.y - SHADOW_PAD) as f64,
            ));
            if let Ok(elem) = MemoryRenderBufferRenderElement::from_buffer(
                renderer,
                pos,
                &deco.frame,
                alpha.into(),
                None,
                None,
                Kind::Unspecified,
            ) {
                elements.push(AquaRenderElement::Memory(elem));
            }
        }

        // Client surface (and popups) shifted down by titlebar_h (0 for CSD windows).
        // For Wayland clients, window.geometry().loc accounts for client-side window margins/shadows.
        let geom_loc = window.geometry().loc;
        let client_loc = Point::from((loc.x - geom_loc.x, loc.y + titlebar_h - geom_loc.y));
        let loc_phys = client_loc.to_physical_precise_round(scale_s);
        let surface_elements: Vec<WaylandSurfaceRenderElement<GlesRenderer>> =
            window.render_elements(renderer, loc_phys, scale_s, alpha);
        elements.extend(surface_elements.into_iter().map(AquaRenderElement::Surface));

        // Traffic lights (in titlebar above the background, only for non-CSD windows).
        if !is_csd && !is_fullscreen {
            let pointer_pos = state.seat.get_pointer().map(|p| p.current_location());
            let lights_hovered = pointer_pos.map_or(false, |pos| {
                traffic_light_at(loc, pos).is_some()
            });
            let lights = state.deco.lights(dark, focused, lights_hovered);
            let pos = Point::<f64, Physical>::from((
                (loc.x + 14) as f64,
                (loc.y + (TITLEBAR_H - 14) / 2) as f64,
            ));
            if let Ok(elem) = MemoryRenderBufferRenderElement::from_buffer(
                renderer,
                pos,
                lights,
                alpha.into(),
                None,
                None,
                Kind::Unspecified,
            ) {
                elements.push(AquaRenderElement::Memory(elem));
            }
        }

        // Crisp rounded border above the window (SSD only).
        if !is_csd && !is_fullscreen {
            let deco = state.deco.deco(key, size.w, size.h, title.as_deref(), focused, is_csd, &state.shell.theme);
            let pos = Point::<f64, Physical>::from((loc.x as f64, loc.y as f64));
            if let Ok(elem) = MemoryRenderBufferRenderElement::from_buffer(
                renderer,
                pos,
                &deco.border,
                alpha.into(),
                None,
                None,
                Kind::Unspecified,
            ) {
                elements.push(AquaRenderElement::Memory(elem));
            }
        }
    }

    if let Some(elem) = state.shell.panel_element(renderer) {
        elements.push(AquaRenderElement::Memory(elem));
    }
    if let Some(elem) = state
        .shell
        .dock_element(renderer, geometry.size.w, geometry.size.h)
    {
        elements.push(AquaRenderElement::Memory(elem));
    }

    // Dropdown menu (if open)
    if let Some(ref active) = state.menu.active_menu {
        let menu_pos = active.1;
        let pointer_pos = state.seat.get_pointer().map(|p| {
            let loc = p.current_location();
            (loc.x, loc.y)
        });
        if let Some(buf) = crate::spotlight::render_menu(&state.menu, &state.shell.theme, &state.deco.text, pointer_pos) {
            let pos = Point::<f64, Physical>::from((menu_pos.x as f64, menu_pos.y as f64));
            if let Ok(elem) = MemoryRenderBufferRenderElement::from_buffer(
                renderer,
                pos,
                &buf,
                None,
                None,
                None,
                Kind::Unspecified,
            ) {
                elements.push(AquaRenderElement::Memory(elem));
            }
        }
    }

    // Spotlight search window (if open)
    if state.spotlight.open {
        if let Some(buf) = crate::spotlight::render_spotlight(&state.spotlight, &state.shell.theme, &state.deco.text) {
            let spot_x = (geometry.size.w - 620) / 2;
            let spot_y = geometry.size.h / 5;
            let pos = Point::<f64, Physical>::from((spot_x as f64, spot_y as f64));
            if let Ok(elem) = MemoryRenderBufferRenderElement::from_buffer(
                renderer,
                pos,
                &buf,
                None,
                None,
                None,
                Kind::Unspecified,
            ) {
                elements.push(AquaRenderElement::Memory(elem));
            }
        }
    }

    // Control Center (if open)
    if state.control_center.open {
        let pointer_pos = state.seat.get_pointer().map(|p| {
            let loc = p.current_location();
            (loc.x, loc.y)
        });
        if let Some(buf) = crate::control_center::render_control_center(&state.control_center, &state.shell.theme, &state.deco.text, pointer_pos) {
            let cc_x = geometry.size.w - crate::control_center::CC_W - 12;
            let cc_y = ui::PANEL_H + 8;
            let pos = Point::<f64, Physical>::from((cc_x as f64, cc_y as f64));
            if let Ok(elem) = MemoryRenderBufferRenderElement::from_buffer(
                renderer,
                pos,
                &buf,
                None,
                None,
                None,
                Kind::Unspecified,
            ) {
                elements.push(AquaRenderElement::Memory(elem));
            }
        }
    }

    // App Switcher overlay (Super + Tab)
    if state.switcher.active {
        if let Some(buf) = crate::control_center::render_app_switcher(&state.switcher, &state.shell.theme, &state.deco.text) {
            let count = state.switcher.items.len();
            let total_w = (16 * 2 + count * 72 + (count - 1) * 14) as i32;
            let sw_x = (geometry.size.w - total_w) / 2;
            let sw_y = (geometry.size.h - 104) / 2;
            let pos = Point::<f64, Physical>::from((sw_x as f64, sw_y as f64));
            if let Ok(elem) = MemoryRenderBufferRenderElement::from_buffer(
                renderer,
                pos,
                &buf,
                None,
                None,
                None,
                Kind::Unspecified,
            ) {
                elements.push(AquaRenderElement::Memory(elem));
            }
        }
    }

    // System Settings window (if open)
    if state.settings.open {
        if let Some(buf) = crate::settings::render_settings(&state.settings, &state.shell.theme, &state.deco.text) {
            let (set_x, set_y) = state.settings.loc.unwrap_or_else(|| {
                (
                    (geometry.size.w - crate::settings::SETTINGS_W) / 2,
                    (geometry.size.h - crate::settings::SETTINGS_H) / 2,
                )
            });
            let pos = Point::<f64, Physical>::from((set_x as f64, set_y as f64));
            if let Ok(elem) = MemoryRenderBufferRenderElement::from_buffer(
                renderer,
                pos,
                &buf,
                None,
                None,
                None,
                Kind::Unspecified,
            ) {
                elements.push(AquaRenderElement::Memory(elem));
            }
        }
    }

    // Stage Manager (left sidebar thumbnails)
    if state.stage_manager.enabled {
        if let Some((sm_h, buf)) = crate::desktop_menu::render_stage_manager(
            &state.stage_manager,
            &windows,
            state.focused_window.as_ref(),
            &state.shell.theme,
            &state.deco.text,
            geometry.size.h,
            pointer_pos,
        ) {
            let sm_x = 12;
            let sm_y = (geometry.size.h - sm_h) / 2;
            let pos = Point::<f64, Physical>::from((sm_x as f64, sm_y as f64));
            if let Ok(elem) = MemoryRenderBufferRenderElement::from_buffer(
                renderer,
                pos,
                &buf,
                None,
                None,
                None,
                Kind::Unspecified,
            ) {
                elements.push(AquaRenderElement::Memory(elem));
            }
        }
    }

    // Notification Center History Panel (slide-out from right)
    if state.notifications.panel_open {
        if let Some(buf) = crate::notifications::render_nc_panel(
            &state.notifications,
            &state.shell.theme,
            &state.deco.text,
        ) {
            let nc_x = geometry.size.w - crate::notifications::NC_PANEL_W - 12;
            let nc_y = ui::PANEL_H + 8;
            let pos = Point::<f64, Physical>::from((nc_x as f64, nc_y as f64));
            if let Ok(elem) = MemoryRenderBufferRenderElement::from_buffer(
                renderer,
                pos,
                &buf,
                None,
                None,
                None,
                Kind::Unspecified,
            ) {
                elements.push(AquaRenderElement::Memory(elem));
            }
        }
    }

    // Floating Notification Toasts (top-right)
    for (toast_y, buf) in crate::notifications::render_toasts(
        &state.notifications,
        &state.shell.theme,
        &state.deco.text,
        now,
    ) {
        let toast_x = geometry.size.w - crate::notifications::TOAST_W - 16;
        let pos = Point::<f64, Physical>::from((toast_x as f64, toast_y as f64));
        if let Ok(elem) = MemoryRenderBufferRenderElement::from_buffer(
            renderer,
            pos,
            &buf,
            None,
            None,
            None,
            Kind::Unspecified,
        ) {
            elements.push(AquaRenderElement::Memory(elem));
        }
    }

    // Wi-Fi Dropdown Menu (if open)
    if state.wifi_menu.open {
        let wifi_menu_x = (geometry.size.w - crate::wifi::WIFI_MENU_W - 90).max(10);
        let wifi_menu_y = ui::PANEL_H + 6;
        let menu_pos = Point::from((wifi_menu_x, wifi_menu_y));
        if let Some(buf) = crate::wifi::render_wifi_menu(
            &state.wifi_menu,
            &state.shell.theme,
            &state.deco.text,
            pointer_pos,
            menu_pos,
        ) {
            let pos = Point::<f64, Physical>::from((wifi_menu_x as f64, wifi_menu_y as f64));
            if let Ok(elem) = MemoryRenderBufferRenderElement::from_buffer(
                renderer,
                pos,
                &buf,
                None,
                None,
                None,
                Kind::Unspecified,
            ) {
                elements.push(AquaRenderElement::Memory(elem));
            }
        }
    }

    // Desktop Context Menu (right click on wallpaper)
    if state.desktop_menu.open {
        if let Some(buf) = crate::desktop_menu::render_desktop_menu(
            &state.desktop_menu,
            &state.shell.theme,
            &state.deco.text,
            pointer_pos,
        ) {
            let pos = Point::<f64, Physical>::from((
                state.desktop_menu.pos.x as f64,
                state.desktop_menu.pos.y as f64,
            ));
            if let Ok(elem) = MemoryRenderBufferRenderElement::from_buffer(
                renderer,
                pos,
                &buf,
                None,
                None,
                None,
                Kind::Unspecified,
            ) {
                elements.push(AquaRenderElement::Memory(elem));
            }
        }
    }

    // OSD HUD Overlay (Volume/Brightness centered)
    state.osd.update();
    if let Some(buf) = crate::notifications::render_osd(
        &state.osd,
        &state.shell.theme,
        &state.deco.text,
    ) {
        let osd_x = (geometry.size.w - crate::notifications::OSD_SIZE) / 2;
        let osd_y = (geometry.size.h - crate::notifications::OSD_SIZE) / 2 + 100;
        let pos = Point::<f64, Physical>::from((osd_x as f64, osd_y as f64));
        if let Ok(elem) = MemoryRenderBufferRenderElement::from_buffer(
            renderer,
            pos,
            &buf,
            None,
            None,
            None,
            Kind::Unspecified,
        ) {
            elements.push(AquaRenderElement::Memory(elem));
        }
    }

    // Hardware/Software mouse cursor for native sessions (TTY/DRM)
    // If hardware_cursor is enabled in DRM, skip rendering software cursor in framebuffer
    if let Some(pos) = pointer_pos {
        let cursor_status = state.cursor_status.lock().ok().map(|s| s.clone());
        match cursor_status {
            Some(smithay::input::pointer::CursorImageStatus::Surface(ref surface)) => {
                let loc_phys = Point::<f64, Logical>::from((pos.0, pos.1)).to_physical_precise_round(scale_s);
                let surface_elements: Vec<WaylandSurfaceRenderElement<GlesRenderer>> =
                    smithay::backend::renderer::element::surface::render_elements_from_surface_tree(
                        renderer,
                        surface,
                        loc_phys,
                        scale_s,
                        1.0,
                        Kind::Unspecified,
                    );
                elements.extend(surface_elements.into_iter().map(AquaRenderElement::Surface));
            }
            Some(smithay::input::pointer::CursorImageStatus::Hidden) => {}
            _ => {
                // If DRM hardware cursor plane is active, the GPU draws it with 0-lag.
                // Otherwise fall back to drawing it in the framebuffer.
                if !state.hardware_cursor {
                    let cursor_buf = state.deco.cursor().clone();
                    let pos_phys = Point::<f64, Logical>::from((pos.0, pos.1)).to_physical_precise_round(scale_s);
                    if let Ok(elem) = MemoryRenderBufferRenderElement::from_buffer(
                        renderer,
                        pos_phys,
                        &cursor_buf,
                        None,
                        None,
                        None,
                        Kind::Unspecified,
                    ) {
                        elements.push(AquaRenderElement::Memory(elem));
                    }
                }
            }
        }
    }

    let clear = state.shell.theme.wallpaper_bottom();
    let clear = [
        clear[0] as f32 / 255.0,
        clear[1] as f32 / 255.0,
        clear[2] as f32 / 255.0,
        1.0,
    ];

    // `OutputDamageTracker` expects the element list in top-to-bottom order
    // (it renders the list reversed), while we built it bottom-to-top.
    elements.reverse();

    damage_tracker.render_output(renderer, framebuffer, 0, &elements, clear)?;
    Ok(())
}

fn running_apps(windows: &[Window]) -> [bool; 6] {
    let mut running = [false; 6];
    for window in windows {
        let app_id = window_app_id(window).unwrap_or_default().to_lowercase();
        let title = window_title(window).unwrap_or_default().to_lowercase();
        for (i, app) in ui::APPS.iter().enumerate() {
            let id = app.id.to_lowercase();
            let name = app.name.to_lowercase();
            if app_id.contains(&id) || app_id.contains(&name) || title.contains(&id) || title.contains(&name) {
                running[i] = true;
            }
        }
    }
    running
}

fn build_deco(
    w: i32,
    h: i32,
    title: Option<&str>,
    focused: bool,
    is_csd: bool,
    theme: &Theme,
    text_renderer: &TextRenderer,
) -> Deco {
    let total_h = if is_csd { h } else { h + TITLEBAR_H };
    let pw = (w + SHADOW_PAD * 2).max(1) as u32;
    let ph = (total_h + SHADOW_PAD * 2).max(1) as u32;
    let mut pm = tiny_skia::Pixmap::new(pw, ph).expect("deco pixmap");
    let dark = theme.dark();

    // Soft drop shadow: subtle and elegant macOS style.
    let shadow = if dark {
        [0u8, 0, 0, 8]
    } else {
        [0u8, 0, 0, 5]
    };
    for i in (0..12).rev() {
        let grow = i as f32 * 1.2;
        rounded(
            &mut pm,
            SHADOW_PAD as f32 - grow,
            SHADOW_PAD as f32 - grow + 3.0,
            w as f32 + grow * 2.0,
            total_h as f32 + grow * 2.0,
            RADIUS + grow,
            shadow,
        );
    }

    // Window background (and titlebar for SSD).
    if w > 0 && total_h > 0 {
        let bg = if dark {
            [31.0, 31.0, 35.0, 255.0]
        } else {
            [250.0, 250.0, 252.0, 255.0]
        };
        let mut paint = tiny_skia::Paint::default();
        paint.set_color_rgba8(bg[0] as u8, bg[1] as u8, bg[2] as u8, bg[3] as u8);
        let path = round_rect(
            SHADOW_PAD as f32,
            SHADOW_PAD as f32,
            w as f32,
            total_h as f32,
            RADIUS,
        );
        pm.fill_path(
            &path,
            &paint,
            tiny_skia::FillRule::Winding,
            tiny_skia::Transform::identity(),
            None,
        );

        if !is_csd {
            // macOS titlebar header fill
            let titlebar_bg = if dark {
                if focused {
                    [38u8, 38, 44, 255]
                } else {
                    [32u8, 32, 36, 255]
                }
            } else {
                if focused {
                    [236u8, 236, 240, 255]
                } else {
                    [245u8, 245, 248, 255]
                }
            };

            // Draw top rounded titlebar
            let mut tpb = tiny_skia::PathBuilder::new();
            let tx = SHADOW_PAD as f32;
            let ty = SHADOW_PAD as f32;
            let tw = w as f32;
            let th = TITLEBAR_H as f32;
            let r = RADIUS.min(tw / 2.0).min(th);
            let k = 0.552_284_8 * r;
            tpb.move_to(tx + r, ty);
            tpb.line_to(tx + tw - r, ty);
            tpb.cubic_to(tx + tw - r + k, ty, tx + tw, ty + r - k, tx + tw, ty + r);
            tpb.line_to(tx + tw, ty + th);
            tpb.line_to(tx, ty + th);
            tpb.line_to(tx, ty + r);
            tpb.cubic_to(tx, ty + r - k, tx + r - k, ty, tx + r, ty);
            tpb.close();
            if let Some(tpath) = tpb.finish() {
                let mut paint = tiny_skia::Paint::default();
                paint.set_color_rgba8(titlebar_bg[0], titlebar_bg[1], titlebar_bg[2], titlebar_bg[3]);
                pm.fill_path(
                    &tpath,
                    &paint,
                    tiny_skia::FillRule::Winding,
                    tiny_skia::Transform::identity(),
                    None,
                );
            }

            // Titlebar bottom hairline separator
            let sep_color = if dark {
                [0u8, 0, 0, 80]
            } else {
                [0u8, 0, 0, 35]
            };
            let mut paint = tiny_skia::Paint::default();
            paint.set_color_rgba8(sep_color[0], sep_color[1], sep_color[2], sep_color[3]);
            if let Some(rect) = tiny_skia::Rect::from_xywh(tx, ty + th - 1.0, tw, 1.0) {
                pm.fill_rect(rect, &paint, tiny_skia::Transform::identity(), None);
            }

            // Window title in the center of the titlebar
            if let Some(t) = title {
                if !t.is_empty() {
                    let text_color = if dark {
                        if focused { [230u8, 230, 235, 255] } else { [140u8, 140, 145, 255] }
                    } else {
                        if focused { [40u8, 40, 45, 255] } else { [140u8, 140, 145, 255] }
                    };
                    let tw_text = text_renderer.width(t, 13.0);
                    let title_x = tx + (tw - tw_text) / 2.0;
                    // Only draw if it doesn't collide with the traffic lights (at tx + 14..tx + 70)
                    if title_x > tx + 80.0 {
                        let baseline = ty + 20.5;
                        text_renderer.draw(&mut pm, t, title_x, baseline, 13.0, text_color);
                    }
                }
            }
        }
    }

    // Outer border.
    stroke_round(
        &mut pm,
        SHADOW_PAD as f32,
        SHADOW_PAD as f32,
        w as f32,
        total_h as f32,
        RADIUS,
        theme.window_border(),
        1.0,
    );

    let frame = MemoryRenderBuffer::from_slice(
        pm.data(),
        Fourcc::Abgr8888,
        (pw as i32, ph as i32),
        1,
        Transform::Normal,
        None,
    );

    // Border and corner masks drawn above the client so sharp corners are rounded.
    let mut bpm = tiny_skia::Pixmap::new(w.max(1) as u32, total_h.max(1) as u32).expect("border pixmap");
    let border = theme.window_border();

    // Mask bottom sharp corners of client surface to match RADIUS
    let corner_mask_color = if dark {
        [31u8, 31, 35, 255]
    } else {
        [250u8, 250, 252, 255]
    };
    let r_f = RADIUS;
    // Bottom-left corner mask
    {
        let mut pb = tiny_skia::PathBuilder::new();
        let bx = 0.0;
        let by = total_h as f32 - r_f;
        pb.move_to(bx, by);
        pb.line_to(bx, by + r_f);
        pb.line_to(bx + r_f, by + r_f);
        pb.cubic_to(bx + r_f * (1.0 - 0.552_284_8), by + r_f, bx, by + r_f * 0.552_284_8, bx, by);
        pb.close();
        if let Some(p) = pb.finish() {
            let mut paint = tiny_skia::Paint::default();
            paint.set_color_rgba8(corner_mask_color[0], corner_mask_color[1], corner_mask_color[2], corner_mask_color[3]);
            bpm.fill_path(&p, &paint, tiny_skia::FillRule::Winding, tiny_skia::Transform::identity(), None);
        }
    }
    // Bottom-right corner mask
    {
        let mut pb = tiny_skia::PathBuilder::new();
        let bx = w as f32 - r_f;
        let by = total_h as f32 - r_f;
        pb.move_to(bx + r_f, by);
        pb.line_to(bx + r_f, by + r_f);
        pb.line_to(bx, by + r_f);
        pb.cubic_to(bx + r_f * 0.552_284_8, by + r_f, bx + r_f, by + r_f * 0.552_284_8, bx + r_f, by);
        pb.close();
        if let Some(p) = pb.finish() {
            let mut paint = tiny_skia::Paint::default();
            paint.set_color_rgba8(corner_mask_color[0], corner_mask_color[1], corner_mask_color[2], corner_mask_color[3]);
            bpm.fill_path(&p, &paint, tiny_skia::FillRule::Winding, tiny_skia::Transform::identity(), None);
        }
    }

    stroke_round(
        &mut bpm,
        0.5,
        0.5,
        w as f32 - 1.0,
        total_h as f32 - 1.0,
        RADIUS,
        [border[0], border[1], border[2], border[3].min(70)],
        1.0,
    );
    let buffer = MemoryRenderBuffer::from_slice(
        bpm.data(),
        Fourcc::Abgr8888,
        (w.max(1), total_h.max(1)),
        1,
        Transform::Normal,
        None,
    );

    Deco {
        w,
        h,
        dark,
        title: title.map(|s| s.to_string()),
        focused,
        is_csd,
        frame,
        border: buffer,
    }
}

fn build_lights(dark: bool, focused: bool, hovered: bool) -> MemoryRenderBuffer {
    let mut pm = tiny_skia::Pixmap::new(56, 14).expect("lights pixmap");
    let colors: [[u8; 4]; 3] = if focused {
        [
            [255, 95, 87, 255],
            [254, 188, 46, 255],
            [40, 200, 64, 255],
        ]
    } else {
        let gray = if dark {
            [96, 96, 102, 255]
        } else {
            [190, 190, 196, 255]
        };
        [gray, gray, gray]
    };

    for (i, color) in colors.iter().enumerate() {
        let cx = 7.0 + i as f32 * 20.0;
        let mut pb = tiny_skia::PathBuilder::new();
        pb.push_circle(cx, 7.0, 6.0);
        if let Some(path) = pb.finish() {
            let mut paint = tiny_skia::Paint::default();
            paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
            pm.fill_path(
                &path,
                &paint,
                tiny_skia::FillRule::Winding,
                tiny_skia::Transform::identity(),
                None,
            );
            let mut stroke = tiny_skia::Paint::default();
            stroke.set_color_rgba8(0, 0, 0, 40);
            let s = tiny_skia::Stroke {
                width: 0.8,
                ..Default::default()
            };
            pm.stroke_path(
                &path,
                &stroke,
                &s,
                tiny_skia::Transform::identity(),
                None,
            );

            // Draw macOS hover symbols inside circles
            if hovered {
                let symbol_color = [70u8, 0, 0, 180];
                let mut stroke = tiny_skia::Paint::default();
                stroke.set_color_rgba8(symbol_color[0], symbol_color[1], symbol_color[2], symbol_color[3]);
                let strk = tiny_skia::Stroke {
                    width: 1.1,
                    line_cap: tiny_skia::LineCap::Round,
                    ..Default::default()
                };

                if i == 0 {
                    // Close: 'x'
                    let mut pb = tiny_skia::PathBuilder::new();
                    pb.move_to(cx - 2.2, 7.0 - 2.2);
                    pb.line_to(cx + 2.2, 7.0 + 2.2);
                    pb.move_to(cx + 2.2, 7.0 - 2.2);
                    pb.line_to(cx - 2.2, 7.0 + 2.2);
                    if let Some(p) = pb.finish() {
                        pm.stroke_path(&p, &stroke, &strk, tiny_skia::Transform::identity(), None);
                    }
                } else if i == 1 {
                    // Minimize: '-'
                    let mut pb = tiny_skia::PathBuilder::new();
                    pb.move_to(cx - 2.8, 7.0);
                    pb.line_to(cx + 2.8, 7.0);
                    if let Some(p) = pb.finish() {
                        pm.stroke_path(&p, &stroke, &strk, tiny_skia::Transform::identity(), None);
                    }
                } else if i == 2 {
                    // Maximize / Fullscreen: two opposing diagonal triangles
                    let mut pb = tiny_skia::PathBuilder::new();
                    pb.move_to(cx - 2.4, 7.0 - 2.4);
                    pb.line_to(cx + 1.2, 7.0 - 2.4);
                    pb.line_to(cx - 2.4, 7.0 + 1.2);
                    pb.close();
                    pb.move_to(cx + 2.4, 7.0 + 2.4);
                    pb.line_to(cx - 1.2, 7.0 + 2.4);
                    pb.line_to(cx + 2.4, 7.0 - 1.2);
                    pb.close();
                    if let Some(p) = pb.finish() {
                        let mut paint = tiny_skia::Paint::default();
                        paint.set_color_rgba8(0, 80, 20, 200);
                        pm.fill_path(&p, &paint, tiny_skia::FillRule::Winding, tiny_skia::Transform::identity(), None);
                    }
                }
            }
        }
    }

    MemoryRenderBuffer::from_slice(
        pm.data(),
        Fourcc::Abgr8888,
        (56, 14),
        1,
        Transform::Normal,
        None,
    )
}

// --- tiny-skia helpers ----------------------------------------------------

fn round_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> tiny_skia::Path {
    let r = r.min(w / 2.0).min(h / 2.0);
    let k = 0.552_284_8 * r;
    let mut pb = tiny_skia::PathBuilder::new();
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
    pb.finish().expect("round rect")
}

fn rounded(pm: &mut tiny_skia::Pixmap, x: f32, y: f32, w: f32, h: f32, r: f32, color: [u8; 4]) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let path = round_rect(x, y, w, h, r);
    let mut paint = tiny_skia::Paint::default();
    paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
    pm.fill_path(
        &path,
        &paint,
        tiny_skia::FillRule::Winding,
        tiny_skia::Transform::identity(),
        None,
    );
}

fn stroke_round(
    pm: &mut tiny_skia::Pixmap,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    r: f32,
    color: [u8; 4],
    width: f32,
) {
    let path = round_rect(x, y, w, h, r);
    let mut paint = tiny_skia::Paint::default();
    paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
    let stroke = tiny_skia::Stroke {
        width,
        ..Default::default()
    };
    pm.stroke_path(
        &path,
        &paint,
        &stroke,
        tiny_skia::Transform::identity(),
        None,
    );
}

/// Unused rectangle helper kept for future decoration work.
#[allow(dead_code)]
fn _rect(_: Rectangle<i32, Logical>) {}

fn default_cursor_buffer() -> MemoryRenderBuffer {
    let mut pm = tiny_skia::Pixmap::new(24, 24).expect("cursor pixmap");
    // Draw macOS-like sleek black & white cursor arrow
    let mut pb = tiny_skia::PathBuilder::new();
    pb.move_to(0.0, 0.0);
    pb.line_to(0.0, 18.0);
    pb.line_to(4.5, 13.5);
    pb.line_to(8.0, 21.0);
    pb.line_to(11.0, 19.5);
    pb.line_to(7.5, 12.0);
    pb.line_to(13.5, 12.0);
    pb.close();

    if let Some(path) = pb.finish() {
        // macOS cursor: white outline with sleek black fill
        let mut stroke_paint = tiny_skia::Paint::default();
        stroke_paint.set_color_rgba8(255, 255, 255, 255);
        let stroke = tiny_skia::Stroke {
            width: 2.0,
            ..Default::default()
        };
        pm.stroke_path(&path, &stroke_paint, &stroke, tiny_skia::Transform::identity(), None);

        let mut fill_paint = tiny_skia::Paint::default();
        fill_paint.set_color_rgba8(16, 16, 20, 255);
        pm.fill_path(&path, &fill_paint, tiny_skia::FillRule::Winding, tiny_skia::Transform::identity(), None);
    }

    MemoryRenderBuffer::from_slice(
        pm.data(),
        Fourcc::Abgr8888,
        (24, 24),
        1,
        Transform::Normal,
        None,
    )
}
