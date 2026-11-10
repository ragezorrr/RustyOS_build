use smithay::{
    delegate_xdg_decoration, delegate_xdg_shell,
    desktop::{find_popup_root_surface, get_popup_toplevel_coords, PopupKind, PopupManager, Space, Window},
    input::{
        pointer::{Focus, GrabStartData as PointerGrabStartData},
        Seat,
    },
    reexports::{
        wayland_protocols::xdg::{
            decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode as DecorationMode,
            shell::server::xdg_toplevel,
        },
        wayland_server::{
            protocol::{wl_seat, wl_surface::WlSurface},
            Resource,
        },
    },
    utils::{Logical, Point, Rectangle, Serial},
    wayland::{
        compositor::with_states,
        shell::xdg::{
            decoration::XdgDecorationHandler, PopupSurface, PositionerState, ToplevelSurface,
            XdgShellHandler, XdgShellState, XdgToplevelSurfaceData,
        },
    },
};

use crate::{
    grabs::{MoveSurfaceGrab, ResizeSurfaceGrab},
    Aqua,
};

impl XdgShellHandler for Aqua {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        let window = Window::new_wayland_window(surface);

        // Cascade placement inside the shell work area.
        let work = self.work_area();
        let count = self.space.elements().count() as i32;
        let step = (count % 8) * 32;
        let loc = work
            .map(|w| {
                Point::<i32, Logical>::from((
                    w.loc.x + 48 + step,
                    w.loc.y + 40 + step,
                ))
            })
            .unwrap_or_else(|| Point::from((48, 48)));

        let window_for_focus = window.clone();
        if let Some(key) = crate::render::window_key(&window) {
            self.window_open_animations.insert(key, std::time::Instant::now());
        }
        self.space.map_element(window, loc, true);
        self.focus_window(&window_for_focus);
    }

    fn new_popup(&mut self, surface: PopupSurface, _positioner: PositionerState) {
        self.unconstrain_popup(&surface);
        let _ = self.popups.track_popup(PopupKind::Xdg(surface));
    }

    fn reposition_request(&mut self, surface: PopupSurface, positioner: PositionerState, token: u32) {
        surface.with_pending_state(|state| {
            let geometry = positioner.get_geometry();
            state.geometry = geometry;
            state.positioner = positioner;
        });
        self.unconstrain_popup(&surface);
        surface.send_repositioned(token);
    }

    fn move_request(&mut self, surface: ToplevelSurface, seat: wl_seat::WlSeat, serial: Serial) {
        let seat = match Seat::from_resource(&seat) {
            Some(s) => s,
            None => return,
        };

        let wl_surface = surface.wl_surface();

        if let Some(start_data) = check_grab(&seat, wl_surface, serial) {
            let pointer = match seat.get_pointer() {
                Some(p) => p,
                None => return,
            };

            let window = match self
                .space
                .elements()
                .find(|w| w.toplevel().map(|t| t.wl_surface() == wl_surface).unwrap_or(false))
                .cloned()
            {
                Some(w) => w,
                None => return,
            };
            let initial_window_location = match self.space.element_location(&window) {
                Some(loc) => loc,
                None => return,
            };

            let grab = MoveSurfaceGrab {
                start_data,
                window,
                initial_window_location,
            };

            pointer.set_grab(self, grab, serial, Focus::Clear);
        }
    }

    fn resize_request(
        &mut self,
        surface: ToplevelSurface,
        seat: wl_seat::WlSeat,
        serial: Serial,
        edges: xdg_toplevel::ResizeEdge,
    ) {
        let seat = match Seat::from_resource(&seat) {
            Some(s) => s,
            None => return,
        };

        let wl_surface = surface.wl_surface();

        if let Some(start_data) = check_grab(&seat, wl_surface, serial) {
            let pointer = match seat.get_pointer() {
                Some(p) => p,
                None => return,
            };

            let window = match self
                .space
                .elements()
                .find(|w| w.toplevel().map(|t| t.wl_surface() == wl_surface).unwrap_or(false))
                .cloned()
            {
                Some(w) => w,
                None => return,
            };
            let initial_window_location = match self.space.element_location(&window) {
                Some(loc) => loc,
                None => return,
            };
            let initial_window_size = window.geometry().size;

            surface.with_pending_state(|state| {
                state.states.set(xdg_toplevel::State::Resizing);
            });

            surface.send_pending_configure();

            let grab = ResizeSurfaceGrab::start(
                start_data,
                window,
                edges.into(),
                Rectangle::new(initial_window_location, initial_window_size),
            );

            pointer.set_grab(self, grab, serial, Focus::Clear);
        }
    }

    fn grab(&mut self, _surface: PopupSurface, _seat: wl_seat::WlSeat, _serial: Serial) {
        // TODO popup grabs
    }

    fn maximize_request(&mut self, surface: ToplevelSurface) {
        let wl_surface = surface.wl_surface();
        let window = self
            .space
            .elements()
            .find(|w| w.toplevel().map(|t| t.wl_surface() == wl_surface).unwrap_or(false))
            .cloned();

        if let Some(window) = window {
            self.toggle_maximized(&window);
        }
    }

    fn unmaximize_request(&mut self, surface: ToplevelSurface) {
        let wl_surface = surface.wl_surface();
        let window = self
            .space
            .elements()
            .find(|w| w.toplevel().map(|t| t.wl_surface() == wl_surface).unwrap_or(false))
            .cloned();

        if let Some(window) = window {
            if let Some(key) = crate::render::window_key(&window) {
                if self.maximized.contains_key(&key) {
                    self.toggle_maximized(&window);
                }
            }
        }
    }

    fn minimize_request(&mut self, surface: ToplevelSurface) {
        let wl_surface = surface.wl_surface();
        let window = self
            .space
            .elements()
            .find(|w| w.toplevel().map(|t| t.wl_surface() == wl_surface).unwrap_or(false))
            .cloned();

        if let Some(window) = window {
            self.minimize_window(&window);
        }
    }

    fn fullscreen_request(
        &mut self,
        surface: ToplevelSurface,
        _output: Option<smithay::reexports::wayland_server::protocol::wl_output::WlOutput>,
    ) {
        let wl_surface = surface.wl_surface();
        let window = self
            .space
            .elements()
            .find(|w| w.toplevel().map(|t| t.wl_surface() == wl_surface).unwrap_or(false))
            .cloned();

        if let Some(window) = window {
            self.set_fullscreen(&window, true);
        }
    }

    fn unfullscreen_request(&mut self, surface: ToplevelSurface) {
        let wl_surface = surface.wl_surface();
        let window = self
            .space
            .elements()
            .find(|w| w.toplevel().map(|t| t.wl_surface() == wl_surface).unwrap_or(false))
            .cloned();

        if let Some(window) = window {
            self.set_fullscreen(&window, false);
        }
    }

    fn show_window_menu(
        &mut self,
        surface: ToplevelSurface,
        _seat: wl_seat::WlSeat,
        _serial: Serial,
        location: Point<i32, Logical>,
    ) {
        let wl_surface = surface.wl_surface();
        let window = self
            .space
            .elements()
            .find(|w| w.toplevel().map(|t| t.wl_surface() == wl_surface).unwrap_or(false))
            .cloned();

        if let Some(window) = window {
            if let Some(window_loc) = self.space.element_location(&window) {
                let menu_pos = Point::from((window_loc.x + location.x, window_loc.y + location.y));
                self.desktop_menu.open_at(menu_pos);
            }
        }
    }
}

// Xdg Shell
delegate_xdg_shell!(Aqua);

impl XdgDecorationHandler for Aqua {
    fn new_decoration(&mut self, toplevel: ToplevelSurface) {
        let is_csd = with_states(toplevel.wl_surface(), |states| {
            states
                .data_map
                .get::<XdgToplevelSurfaceData>()
                .and_then(|data| data.lock().ok())
                .and_then(|attrs| attrs.app_id.clone())
        })
        .map(|id| {
            let lower = id.to_lowercase();
            lower.contains("cursor")
                || lower.contains("code")
                || lower.contains("chrome")
                || lower.contains("chromium")
                || lower.contains("brave")
                || lower.contains("firefox")
                || lower.contains("edge")
        })
        .unwrap_or(false);

        let mode = if is_csd {
            DecorationMode::ClientSide
        } else {
            DecorationMode::ServerSide
        };

        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(mode);
        });
        toplevel.send_configure();
    }

    fn request_mode(&mut self, toplevel: ToplevelSurface, mode: DecorationMode) {
        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(mode);
        });
        toplevel.send_configure();
    }

    fn unset_mode(&mut self, toplevel: ToplevelSurface) {
        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(DecorationMode::ServerSide);
        });
        toplevel.send_configure();
    }
}

delegate_xdg_decoration!(Aqua);

fn check_grab(
    seat: &Seat<Aqua>,
    surface: &WlSurface,
    serial: Serial,
) -> Option<PointerGrabStartData<Aqua>> {
    let pointer = seat.get_pointer()?;

    // Check that this surface has a click grab.
    if !pointer.has_grab(serial) {
        return None;
    }

    let start_data = pointer.grab_start_data()?;

    let (focus, _) = start_data.focus.as_ref()?;
    // If the focus was for a different surface, ignore the request.
    if !focus.id().same_client_as(&surface.id()) {
        return None;
    }

    Some(start_data)
}

/// Should be called on `WlSurface::commit`
pub fn handle_commit(popups: &mut PopupManager, space: &Space<Window>, surface: &WlSurface) {
    // Handle toplevel commits.
    if let Some(window) = space
        .elements()
        .find(|w| w.toplevel().map(|t| t.wl_surface() == surface).unwrap_or(false))
        .cloned()
    {
        let initial_configure_sent = with_states(surface, |states| {
            states
                .data_map
                .get::<XdgToplevelSurfaceData>()
                .and_then(|data| data.lock().ok())
                .map(|attrs| attrs.initial_configure_sent)
                .unwrap_or(false)
        });

        if !initial_configure_sent {
            if let Some(toplevel) = window.toplevel() {
                toplevel.send_configure();
            }
        }
    }

    // Handle popup commits.
    popups.commit(surface);
    if let Some(popup) = popups.find_popup(surface) {
        match popup {
            PopupKind::Xdg(ref xdg) => {
                if !xdg.is_initial_configure_sent() {
                    let _ = xdg.send_configure();
                }
            }
            PopupKind::InputMethod(ref _input_method) => {}
        }
    }
}

impl Aqua {
    fn unconstrain_popup(&self, popup: &PopupSurface) {
        let Ok(root) = find_popup_root_surface(&PopupKind::Xdg(popup.clone())) else {
            return;
        };
        let Some(window) = self
            .space
            .elements()
            .find(|w| w.toplevel().map(|t| t.wl_surface() == &root).unwrap_or(false))
        else {
            return;
        };

        let output_geo = self.output_geometry();
        let Some(window_geo) = self.space.element_geometry(window) else {
            return;
        };

        // The target geometry for the positioner should be relative to its parent's geometry, so
        // we will compute that here.
        let mut target = output_geo;
        target.loc -= get_popup_toplevel_coords(&PopupKind::Xdg(popup.clone()));
        target.loc -= window_geo.loc;

        popup.with_pending_state(|state| {
            state.geometry = state.positioner.get_unconstrained_geometry(target);
        });
    }
}
