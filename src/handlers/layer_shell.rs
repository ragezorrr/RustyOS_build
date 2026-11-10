use smithay::{
    delegate_layer_shell,
    desktop::{layer_map_for_output, LayerSurface, PopupKind, PopupManager, WindowSurfaceType},
    reexports::wayland_server::protocol::{wl_output::WlOutput, wl_surface::WlSurface},
    wayland::{
        compositor::with_states,
        shell::{
            wlr_layer::{
                Layer, LayerSurface as WlrLayerSurface, LayerSurfaceConfigure, LayerSurfaceData,
                WlrLayerShellHandler, WlrLayerShellState,
            },
            xdg::PopupSurface,
        },
    },
};

use crate::Aqua;

impl WlrLayerShellHandler for Aqua {
    fn shell_state(&mut self) -> &mut WlrLayerShellState {
        &mut self.layer_shell_state
    }

    fn new_layer_surface(
        &mut self,
        surface: WlrLayerSurface,
        output: Option<WlOutput>,
        _layer: Layer,
        namespace: String,
    ) {
        let output = output
            .and_then(|o| smithay::output::Output::from_resource(&o))
            .or_else(|| self.current_output());

        let Some(output) = output else {
            return;
        };

        let layer_surface = LayerSurface::new(surface, namespace);
        let mut map = layer_map_for_output(&output);
        let _ = map.map_layer(&layer_surface);
    }

    fn new_popup(&mut self, _parent: WlrLayerSurface, popup: PopupSurface) {
        self.unconstrain_popup(&popup);
        let _ = self.popups.track_popup(PopupKind::Xdg(popup));
    }

    fn ack_configure(&mut self, _surface: WlSurface, _configure: LayerSurfaceConfigure) {}

    fn layer_destroyed(&mut self, surface: WlrLayerSurface) {
        if let Some(output) = self.current_output() {
            let mut map = layer_map_for_output(&output);
            if let Some(layer) = map
                .layer_for_surface(surface.wl_surface(), WindowSurfaceType::TOPLEVEL)
                .cloned()
            {
                map.unmap_layer(&layer);
            }
        }
    }
}

pub fn handle_commit(_popups: &mut PopupManager, output: &smithay::output::Output, surface: &WlSurface) {
    let mut map = layer_map_for_output(output);
    if let Some(layer) = map.layer_for_surface(surface, WindowSurfaceType::TOPLEVEL).cloned() {
        let initial_configure_sent = with_states(surface, |states| {
            states
                .data_map
                .get::<LayerSurfaceData>()
                .and_then(|data| data.lock().ok())
                .map(|attrs| attrs.initial_configure_sent)
                .unwrap_or(false)
        });

        if !initial_configure_sent {
            layer.layer_surface().send_configure();
        }

        map.arrange();
    }
}

delegate_layer_shell!(Aqua);
