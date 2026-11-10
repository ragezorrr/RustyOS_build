mod compositor;
mod xdg_shell;

use crate::Aqua;

//
// Wl Seat
//

use smithay::input::{Seat, SeatHandler, SeatState};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::Resource;
use smithay::wayland::fractional_scale::{with_fractional_scale, FractionalScaleHandler};
use smithay::wayland::output::OutputHandler;
use smithay::wayland::selection::data_device::{
    set_data_device_focus, ClientDndGrabHandler, DataDeviceHandler, DataDeviceState, ServerDndGrabHandler,
};
use smithay::wayland::selection::primary_selection::{
    set_primary_focus, PrimarySelectionHandler, PrimarySelectionState,
};
use smithay::wayland::selection::SelectionHandler;
use smithay::wayland::xdg_activation::{
    XdgActivationHandler, XdgActivationState, XdgActivationToken, XdgActivationTokenData,
};
use smithay::{
    delegate_data_device, delegate_fractional_scale, delegate_output, delegate_primary_selection,
    delegate_seat, delegate_viewporter, delegate_xdg_activation,
};

impl SeatHandler for Aqua {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<Aqua> {
        &mut self.seat_state
    }

    fn cursor_image(&mut self, _seat: &Seat<Self>, image: smithay::input::pointer::CursorImageStatus) {
        if let Ok(mut status) = self.cursor_status.lock() {
            *status = image;
        }
    }

    fn focus_changed(&mut self, seat: &Seat<Self>, focused: Option<&WlSurface>) {
        let dh = &self.display_handle;
        let client = focused.and_then(|s| dh.get_client(s.id()).ok());
        set_data_device_focus(dh, seat, client.clone());
        set_primary_focus(dh, seat, client);
    }
}

delegate_seat!(Aqua);

//
// Wl Data Device
//

impl SelectionHandler for Aqua {
    type SelectionUserData = ();
}

impl DataDeviceHandler for Aqua {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.data_device_state
    }
}

impl ClientDndGrabHandler for Aqua {}
impl ServerDndGrabHandler for Aqua {}

delegate_data_device!(Aqua);

//
// Wl Output & Xdg Output
//

impl OutputHandler for Aqua {}
delegate_output!(Aqua);

//
// Primary Selection (Middle-click paste)
//

impl PrimarySelectionHandler for Aqua {
    fn primary_selection_state(&self) -> &PrimarySelectionState {
        &self.primary_selection_state
    }
}
delegate_primary_selection!(Aqua);

//
// Viewporter
//

delegate_viewporter!(Aqua);

//
// Fractional Scale
//

impl FractionalScaleHandler for Aqua {
    fn new_fractional_scale(&mut self, surface: WlSurface) {
        // Preferred fractional scale for crisp rendering without blur (default 1.0)
        let scale = self.space.outputs().next().map_or(1.0, |o| o.current_scale().fractional_scale());
        smithay::wayland::compositor::with_states(&surface, |states| {
            with_fractional_scale(states, |fs| {
                fs.set_preferred_scale(scale.max(1.0));
            });
        });
    }
}
delegate_fractional_scale!(Aqua);

//
// XDG Activation
//

impl XdgActivationHandler for Aqua {
    fn activation_state(&mut self) -> &mut XdgActivationState {
        &mut self.xdg_activation_state
    }

    fn request_activation(
        &mut self,
        _token: XdgActivationToken,
        _token_data: XdgActivationTokenData,
        surface: WlSurface,
    ) {
        // Focus and raise window belonging to activated surface
        let window = self
            .space
            .elements()
            .find(|w| w.toplevel().map_or(false, |t| t.wl_surface() == &surface))
            .cloned();
        if let Some(window) = window {
            self.space.raise_element(&window, true);
            self.focus_window(&window);
        }
    }
}
delegate_xdg_activation!(Aqua);
