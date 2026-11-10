#![allow(irrefutable_let_patterns)]

mod handlers;

mod grabs;
mod input;
mod render;
mod state;
mod text;
mod theme;
mod ui;
pub mod udev;
mod winit;
pub mod apps;
pub mod control_center;
pub mod desktop_menu;
pub mod hardware;
pub mod notifications;
pub mod settings;
pub mod spotlight;
pub mod wifi;

use smithay::reexports::{
    calloop::EventLoop,
    wayland_server::{Display, DisplayHandle},
};
pub use state::Aqua;

pub struct CalloopData {
    state: Aqua,
    display_handle: DisplayHandle,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var_os("XCURSOR_THEME").is_none() {
        std::env::set_var("XCURSOR_THEME", "macOS");
    }
    if std::env::var_os("XCURSOR_SIZE").is_none() {
        std::env::set_var("XCURSOR_SIZE", "24");
    }

    if let Ok(env_filter) = tracing_subscriber::EnvFilter::try_from_default_env() {
        tracing_subscriber::fmt().with_env_filter(env_filter).init();
    } else {
        tracing_subscriber::fmt().init();
    }

    let mut event_loop: EventLoop<CalloopData> = EventLoop::try_new()?;

    let display: Display<Aqua> = Display::new()?;
    let display_handle = display.handle();
    let state = Aqua::new(&mut event_loop, display);

    let mut data = CalloopData {
        state,
        display_handle,
    };

    let mut args = std::env::args().skip(1);
    let flag = args.next();
    let arg = args.next();

    let command = match (flag.as_deref(), arg) {
        (Some("-c") | Some("--command"), Some(command)) => Some(command),
        (Some("--help") | Some("-h"), _) => {
            println!("aqua — macOS-styled Wayland compositor\n\nUSAGE:\n    aqua [--command <program>]\n\nOPTIONS:\n    -c, --command <program>   program to launch on startup\n    -h, --help                print this help");
            return Ok(());
        }
        _ => std::env::var("TERMINAL").ok().or_else(|| Some("konsole".into())),
    };

    if let Some(command) = command {
        tracing::info!(%command, "launching startup client");
        std::process::Command::new(command).spawn().ok();
    }

    let has_parent_display = std::env::var_os("WAYLAND_DISPLAY").is_some()
        || std::env::var_os("DISPLAY").is_some();

    if has_parent_display {
        tracing::info!("Parent display found, running in nested winit mode");
        crate::winit::init_winit(&mut event_loop, &mut data)?;
        event_loop.run(None, &mut data, move |_| {})?;
    } else {
        tracing::info!("No parent display, running in native DRM/KMS udev mode");
        crate::udev::run_udev(event_loop, data)?;
    }

    Ok(())
}
