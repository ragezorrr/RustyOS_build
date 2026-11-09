use std::{ffi::OsString, sync::Arc};

use smithay::{
    desktop::{PopupManager, Space, Window, WindowSurfaceType},
    input::{Seat, SeatState},
    reexports::{
        calloop::{generic::Generic, EventLoop, Interest, LoopSignal, Mode, PostAction},
        wayland_server::{
            backend::{ClientData, ClientId, DisconnectReason},
            protocol::wl_surface::WlSurface,
            Display, DisplayHandle,
        },
    },
    utils::{Logical, Point},
    wayland::{
        compositor::{CompositorClientState, CompositorState},
        output::OutputManagerState,
        selection::data_device::DataDeviceState,
        shell::xdg::{decoration::XdgDecorationState, XdgShellState},
        shm::ShmState,
        socket::ListeningSocketSource,
    },
};

use crate::render::DecoCache;
use crate::theme::Theme;
use crate::ui::Shell;
use crate::CalloopData;

pub struct Aqua {
    pub start_time: std::time::Instant,
    pub socket_name: OsString,
    pub display_handle: DisplayHandle,

    pub space: Space<Window>,
    pub loop_signal: LoopSignal,

    // Aqua shell
    pub shell: Shell,
    pub deco: DecoCache,
    pub menu: crate::spotlight::MenuState,
    pub spotlight: crate::spotlight::SpotlightState,
    pub control_center: crate::control_center::ControlCenterState,
    pub switcher: crate::control_center::AppSwitcherState,
    pub settings: crate::settings::SettingsWindow,
    pub notifications: crate::notifications::NotificationCenter,
    pub osd: crate::notifications::OsdOverlay,
    pub desktop_menu: crate::desktop_menu::DesktopContextMenu,
    pub stage_manager: crate::desktop_menu::StageManager,
    pub wifi_menu: crate::wifi::WifiMenuState,
    /// Settings window drag state: (initial_pointer_pos, initial_window_loc)
    pub settings_dragging: Option<(Point<f64, Logical>, Point<i32, Logical>)>,
    /// Minimized windows hidden from space: key -> (Window, saved_location)
    pub minimized_windows: std::collections::HashMap<u32, (Window, Point<i32, Logical>)>,
    pub focused_window: Option<Window>,
    /// Saved geometry of windows that are currently maximized.
    pub maximized: std::collections::HashMap<u32, (Point<i32, Logical>, smithay::utils::Size<i32, Logical>)>,
    /// Last titlebar click time for double-click detection.
    pub last_titlebar_click: Option<(u32, std::time::Instant)>,
    /// Window animations in progress: key -> (start_time, duration, from_loc, to_loc)
    pub window_animations: std::collections::HashMap<u32, (std::time::Instant, std::time::Duration, Point<i32, Logical>, Point<i32, Logical>)>,
    /// Window opening animations: key -> start_time
    pub window_open_animations: std::collections::HashMap<u32, std::time::Instant>,
    /// Window closing animations: key -> (start_time, Window)
    pub window_close_animations: std::collections::HashMap<u32, (std::time::Instant, Window)>,

    // Smithay State
    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub xdg_decoration_state: XdgDecorationState,
    pub shm_state: ShmState,
    pub output_manager_state: OutputManagerState,
    pub seat_state: SeatState<Aqua>,
    pub data_device_state: DataDeviceState,
    pub popups: PopupManager,

    pub seat: Seat<Self>,
    pub cursor_status: std::sync::Arc<std::sync::Mutex<smithay::input::pointer::CursorImageStatus>>,
}

impl Aqua {
    pub fn new(event_loop: &mut EventLoop<CalloopData>, display: Display<Self>) -> Self {
        ensure_editor_configs();
        let start_time = std::time::Instant::now();

        let dh = display.handle();

        let compositor_state = CompositorState::new::<Self>(&dh);
        let xdg_shell_state = XdgShellState::new::<Self>(&dh);
        let xdg_decoration_state = XdgDecorationState::new::<Self>(&dh);
        let shm_state = ShmState::new::<Self>(&dh, vec![]);
        let output_manager_state = OutputManagerState::new_with_xdg_output::<Self>(&dh);
        let mut seat_state = SeatState::new();
        let data_device_state = DataDeviceState::new::<Self>(&dh);
        let popups = PopupManager::default();

        // A seat is a group of keyboards, pointer and touch devices.
        // A seat typically has a pointer and maintains a keyboard focus and a pointer focus.
        let mut seat: Seat<Self> = seat_state.new_wl_seat(&dh, "winit");

        // Notify clients that we have a keyboard, for the sake of the example we assume that keyboard is always present.
        // You may want to track keyboard hot-plug in real compositor.
        seat.add_keyboard(Default::default(), 200, 25).unwrap();

        // Notify clients that we have a pointer (mouse)
        // Here we assume that there is always pointer plugged in
        seat.add_pointer();

        // A space represents a two-dimensional plane. Windows and Outputs can be mapped onto it.
        //
        // Windows get a position and stacking order through mapping.
        // Outputs become views of a part of the Space and can be rendered via Space::render_output.
        let space = Space::default();

        let socket_name = Self::init_wayland_listener(display, event_loop);

        // Get the loop signal, used to stop the event loop
        let loop_signal = event_loop.get_signal();

        Self {
            start_time,
            display_handle: dh,

            space,
            loop_signal,
            shell: Shell::new(Theme::load()),
            deco: DecoCache::default(),
            menu: crate::spotlight::MenuState::default(),
            spotlight: crate::spotlight::SpotlightState::default(),
            control_center: crate::control_center::ControlCenterState::default(),
            switcher: crate::control_center::AppSwitcherState::default(),
            settings: crate::settings::SettingsWindow::default(),
            notifications: crate::notifications::NotificationCenter::default(),
            osd: crate::notifications::OsdOverlay::default(),
            desktop_menu: crate::desktop_menu::DesktopContextMenu::default(),
            stage_manager: crate::desktop_menu::StageManager::default(),
            wifi_menu: crate::wifi::WifiMenuState::default(),
            settings_dragging: None,
            minimized_windows: std::collections::HashMap::new(),
            focused_window: None,
            maximized: std::collections::HashMap::new(),
            last_titlebar_click: None,
            window_animations: std::collections::HashMap::new(),
            window_open_animations: std::collections::HashMap::new(),
            window_close_animations: std::collections::HashMap::new(),
            socket_name,

            compositor_state,
            xdg_shell_state,
            xdg_decoration_state,
            shm_state,
            output_manager_state,
            seat_state,
            data_device_state,
            popups,
            seat,
            cursor_status: std::sync::Arc::new(std::sync::Mutex::new(smithay::input::pointer::CursorImageStatus::default_named())),
        }
    }

    fn init_wayland_listener(
        display: Display<Aqua>,
        event_loop: &mut EventLoop<CalloopData>,
    ) -> OsString {
        // Creates a new listening socket, automatically choosing the next available `wayland` socket name.
        let listening_socket = ListeningSocketSource::new_auto().unwrap();

        // Get the name of the listening socket.
        // Clients will connect to this socket.
        let socket_name = listening_socket.socket_name().to_os_string();

        let loop_handle = event_loop.handle();

        loop_handle
            .insert_source(listening_socket, move |client_stream, _, state| {
                // Inside the callback, you should insert the client into the display.
                //
                // You may also associate some data with the client when inserting the client.
                state
                    .display_handle
                    .insert_client(client_stream, Arc::new(ClientState::default()))
                    .unwrap();
            })
            .expect("Failed to init the wayland event source.");

        // You also need to add the display itself to the event loop, so that client events will be processed by wayland-server.
        loop_handle
            .insert_source(
                Generic::new(display, Interest::READ, Mode::Level),
                |_, display, state| {
                    // Safety: we don't drop the display
                    unsafe {
                        display.get_mut().dispatch_clients(&mut state.state).unwrap();
                    }
                    Ok(PostAction::Continue)
                },
            )
            .unwrap();

        socket_name
    }

    pub fn surface_under(&self, pos: Point<f64, Logical>) -> Option<(WlSurface, Point<f64, Logical>)> {
        self.space.element_under(pos).and_then(|(window, location)| {
            let tb_h = crate::render::window_titlebar_h(&window);
            let geom_loc = window.geometry().loc;
            let client_pos = Point::from((location.x - geom_loc.x, location.y + tb_h - geom_loc.y));
            window
                .surface_under(pos - client_pos.to_f64(), WindowSurfaceType::ALL)
                .map(|(s, p)| (s, (p + client_pos).to_f64()))
        })
    }

    fn prepare_command(&self, cmd: &mut std::process::Command) {
        cmd.env("WAYLAND_DISPLAY", &self.socket_name);
        cmd.env("GDK_BACKEND", "wayland,x11");
        cmd.env("QT_QPA_PLATFORM", "wayland;xcb");
        cmd.env("CLUTTER_BACKEND", "wayland");
        cmd.env("SDL_VIDEODRIVER", "wayland");
        cmd.env("MOZ_ENABLE_WAYLAND", "1");
        cmd.env("ELECTRON_OZONE_PLATFORM_HINT", "auto");
        // Remove host DISPLAY so clients don't mistakenly open on the host's outer X11 session
        cmd.env_remove("DISPLAY");
    }

    pub fn spawn(&self, command: &str) {
        tracing::info!(%command, "spawning client");
        let mut cmd = std::process::Command::new(command);
        self.prepare_command(&mut cmd);
        if let Err(err) = cmd.spawn() {
            tracing::warn!(%command, %err, "failed to spawn client");
        }
    }

    pub fn spawn_cmd(&self, cmd_line: &str) {
        tracing::info!(%cmd_line, "spawning client via shell");
        // Automatically ensure Chromium/Chrome/Cursor/VSCode launches with native Wayland support
        let final_cmd = if (cmd_line.contains("chrome")
            || cmd_line.contains("chromium")
            || cmd_line.contains("brave")
            || cmd_line.contains("cursor")
            || cmd_line.contains("code"))
            && !cmd_line.contains("--ozone-platform")
        {
            format!("{} --ozone-platform-hint=auto", cmd_line)
        } else {
            cmd_line.to_string()
        };

        let mut cmd = std::process::Command::new("sh");
        cmd.arg("-c").arg(&final_cmd);
        self.prepare_command(&mut cmd);
        if let Err(err) = cmd.spawn() {
            tracing::warn!(%cmd_line, %err, "failed to spawn client command");
        }
    }

    pub fn current_output(&self) -> Option<smithay::output::Output> {
        self.space.outputs().next().cloned()
    }

    pub fn output_geometry(&self) -> smithay::utils::Rectangle<i32, Logical> {
        self.current_output()
            .and_then(|o| self.space.output_geometry(&o))
            .unwrap_or_else(|| smithay::utils::Rectangle::new((0, 0).into(), (1920, 1080).into()))
    }

    pub fn work_area(&self) -> Option<smithay::utils::Rectangle<i32, Logical>> {
        let output = self.current_output()?;
        let geometry = self.space.output_geometry(&output)?;
        Some(self.shell.work_area(geometry.size.w, geometry.size.h))
    }

    pub fn apply_display_mode(&mut self, width: i32, height: i32, scale_percent: i32) {
        self.settings.display.resolution = (width, height);
        self.settings.display.scaling = scale_percent;
        self.settings.display.applied_feedback = Some(std::time::Instant::now());

        let scale_factor = scale_percent as f64 / 100.0;

        if let Some(output) = self.current_output() {
            let mode = smithay::output::Mode {
                size: (width, height).into(),
                refresh: self.settings.display.refresh_rate * 1000,
            };
            let scale = if scale_percent == 100 {
                smithay::output::Scale::Integer(1)
            } else {
                smithay::output::Scale::Fractional(scale_factor)
            };

            output.change_current_state(
                Some(mode),
                Some(smithay::utils::Transform::Normal),
                Some(scale),
                Some((0, 0).into()),
            );

            // Remap output so Space updates its logical dimensions
            self.space.map_output(&output, (0, 0));
        }

        // Force full recalculation of shell geometry (menu bar, dock, hitboxes, wallpaper)
        self.shell.invalidate_cache();

        // Reposition / center Settings window within new screen dimensions
        let geo = self.output_geometry();
        let set_x = ((geo.size.w - crate::settings::SETTINGS_W) / 2).max(10);
        let set_y = ((geo.size.h - crate::settings::SETTINGS_H) / 2).max(crate::ui::PANEL_H + 10);
        self.settings.loc = Some((set_x, set_y));

        // Clamp pointer within new screen dimensions if needed
        if let Some(pointer) = self.seat.get_pointer() {
            let loc = pointer.current_location();
            if loc.x >= geo.size.w as f64 || loc.y >= geo.size.h as f64 {
                let clamped_x = loc.x.clamp(0.0, (geo.size.w - 2) as f64);
                let clamped_y = loc.y.clamp(0.0, (geo.size.h - 2) as f64);
                let serial = smithay::utils::SERIAL_COUNTER.next_serial();
                pointer.motion(
                    self,
                    None,
                    &smithay::input::pointer::MotionEvent {
                        location: (clamped_x, clamped_y).into(),
                        serial,
                        time: 0,
                    },
                );
                pointer.frame(self);
            }
        }

        // Reconfigure windows to fit new work area
        if let Some(work_area) = self.work_area() {
            let windows: Vec<_> = self.space.elements().cloned().collect();
            for w in windows {
                if let Some(loc) = self.space.element_location(&w) {
                    let mut new_loc = loc;
                    if new_loc.x + 80 > work_area.loc.x + work_area.size.w {
                        new_loc.x = (work_area.loc.x + work_area.size.w - w.geometry().size.w).max(work_area.loc.x);
                    }
                    if new_loc.y + 80 > work_area.loc.y + work_area.size.h {
                        new_loc.y = (work_area.loc.y + work_area.size.h - w.geometry().size.h).max(work_area.loc.y);
                    }
                    if new_loc != loc {
                        self.space.map_element(w.clone(), new_loc, false);
                    }
                }
                if let Some(toplevel) = w.toplevel() {
                    toplevel.send_pending_configure();
                }
            }
        }

        self.notifications.post(
            "Настройки экрана",
            "Разрешение применено",
            &format!("Установлен режим: {} × {} ({}%)", width, height, scale_percent),
        );
    }

    pub fn focus_window(&mut self, window: &Window) {
        self.focused_window = Some(window.clone());
        if let Some(toplevel) = window.toplevel() {
            let serial = smithay::utils::SERIAL_COUNTER.next_serial();
            if let Some(keyboard) = self.seat.get_keyboard() {
                keyboard.set_focus(self, Some(toplevel.wl_surface().clone()), serial);
            }
        }
    }

    pub fn minimize_window(&mut self, window: &Window) {
        if let Some(key) = crate::render::window_key(window) {
            let loc = self.space.element_location(window).unwrap_or_else(|| Point::from((48, 48)));
            self.minimized_windows.insert(key, (window.clone(), loc));
            self.space.unmap_elem(window);
            if self.focused_window.as_ref() == Some(window) {
                self.focused_window = None;
            }
        }
    }

    pub fn restore_minimized_window(&mut self, key: u32) {
        if let Some((win, loc)) = self.minimized_windows.remove(&key) {
            self.space.map_element(win.clone(), loc, true);
            self.space.raise_element(&win, true);
            self.focus_window(&win);
            self.window_open_animations.insert(key, std::time::Instant::now());
        }
    }

    pub fn close_focused(&mut self) {
        let Some(window) = self.focused_window.clone() else {
            return;
        };
        self.close_window(&window);
    }

    pub fn close_window(&mut self, window: &Window) {
        if let Some(key) = crate::render::window_key(window) {
            self.window_close_animations.insert(key, (std::time::Instant::now(), window.clone()));
            if self.focused_window.as_ref() == Some(window) {
                self.focused_window = None;
            }
        } else if let Some(toplevel) = window.toplevel() {
            toplevel.send_close();
        }
    }

    pub fn toggle_theme(&mut self) {
        self.shell.theme.toggle();
    }

    pub fn toggle_maximized(&mut self, window: &Window) {
        let Some(toplevel) = window.toplevel() else {
            return;
        };
        let Some(key) = crate::render::window_key(window) else {
            return;
        };
        let Some(work) = self.work_area() else {
            return;
        };

        if let Some((loc, size)) = self.maximized.remove(&key) {
            toplevel.with_pending_state(|state| {
                state.states.unset(smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State::Maximized);
                state.size = Some(size);
            });
            let _ = toplevel.send_configure();
            let current_loc = self.space.element_location(window).unwrap_or(loc);
            self.window_animations.insert(
                key,
                (
                    std::time::Instant::now(),
                    std::time::Duration::from_millis(200),
                    current_loc,
                    loc,
                ),
            );
            self.space.map_element(window.clone(), current_loc, true);
        } else {
            let current_loc = self.space.element_location(window).unwrap_or_else(|| Point::from((0, 0)));
            let size = window.geometry().size;
            self.maximized.insert(key, (current_loc, size));
            toplevel.with_pending_state(|state| {
                state.states.set(smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State::Maximized);
                let tb_h = crate::render::window_titlebar_h(window);
                let client_h = (work.size.h - tb_h).max(100);
                state.size = Some(smithay::utils::Size::from((work.size.w, client_h)));
            });
            let _ = toplevel.send_configure();
            self.window_animations.insert(
                key,
                (
                    std::time::Instant::now(),
                    std::time::Duration::from_millis(200),
                    current_loc,
                    work.loc,
                ),
            );
            self.space.map_element(window.clone(), current_loc, true);
        }
    }
}

#[derive(Default)]
pub struct ClientState {
    pub compositor_state: CompositorClientState,
}

impl ClientData for ClientState {
    fn initialized(&self, _client_id: ClientId) {}
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}

fn ensure_editor_configs() {
    let home = std::env::var("HOME").unwrap_or_default();
    if home.is_empty() {
        return;
    }
    let paths = [
        format!("{}/.config/Cursor/User/settings.json", home),
        format!("{}/.config/Code/User/settings.json", home),
    ];
    for path in &paths {
        let p = std::path::Path::new(path);
        if let Some(parent) = p.parent() {
            if parent.exists() {
                let mut content = std::fs::read_to_string(p).unwrap_or_else(|_| "{\n}".to_string());
                let mut modified = false;
                if content.contains("\"window.menuBarVisibility\": \"hidden\"") {
                    content = content.replace("\"window.menuBarVisibility\": \"hidden\"", "\"window.menuBarVisibility\": \"compact\"");
                    modified = true;
                } else if !content.contains("\"window.menuBarVisibility\"") {
                    let trimmed = content.trim_end().trim_end_matches('}').trim_end();
                    let prefix = if trimmed.ends_with('{') { "" } else { ",\n" };
                    content = format!("{}{}{}    \"window.menuBarVisibility\": \"compact\"\n}}\n", trimmed, prefix, if prefix.is_empty() { "\n" } else { "" });
                    modified = true;
                }
                if modified {
                    let _ = std::fs::write(p, content);
                }
            }
        }
    }
}
