use std::time::Duration;

use smithay::{
    backend::{
        renderer::damage::OutputDamageTracker,
        winit::{self, WinitEvent},
    },
    output::{Mode, Output, PhysicalProperties, Subpixel},
    reexports::calloop::EventLoop,
    utils::{Point, Rectangle, Transform},
};

use crate::{CalloopData, Aqua};

pub fn init_winit(
    event_loop: &mut EventLoop<CalloopData>,
    data: &mut CalloopData,
) -> Result<(), Box<dyn std::error::Error>> {
    let display_handle = &mut data.display_handle;
    let state = &mut data.state;

    let (mut backend, winit) = winit::init()?;

    let mode = Mode {
        size: backend.window_size(),
        refresh: 60_000,
    };

    let output = Output::new(
        "winit".to_string(),
        PhysicalProperties {
            size: (0, 0).into(),
            subpixel: Subpixel::Unknown,
            make: "Smithay".into(),
            model: "Winit".into(),
        },
    );
    let _global = output.create_global::<Aqua>(display_handle);
    output.change_current_state(Some(mode), Some(Transform::Flipped180), None, Some((0, 0).into()));
    output.set_preferred(mode);

    state.space.map_output(&output, (0, 0));

    let mut damage_tracker = OutputDamageTracker::from_output(&output);

    std::env::set_var("WAYLAND_DISPLAY", &state.socket_name);

    event_loop.handle().insert_source(winit, move |event, _, data| {
        let display = &mut data.display_handle;
        let state = &mut data.state;

        match event {
            WinitEvent::Resized { size, .. } => {
                output.change_current_state(
                    Some(Mode {
                        size,
                        refresh: 60_000,
                    }),
                    None,
                    None,
                    None,
                );
            }
            WinitEvent::Input(event) => state.process_input_event(event),
            WinitEvent::Redraw => {
                // Update window animations
                let now = std::time::Instant::now();
                let mut completed_anims = Vec::new();
                let anim_keys: Vec<u32> = state.window_animations.keys().copied().collect();

                for key in anim_keys {
                    if let Some(&(start, duration, from_loc, to_loc)) = state.window_animations.get(&key) {
                        let elapsed = now.duration_since(start).as_secs_f64();
                        let total = duration.as_secs_f64();
                        let t = (elapsed / total).clamp(0.0, 1.0);
                        // Ease-out cubic: 1 - (1 - t)^3
                        let ease = 1.0 - (1.0 - t).powi(3);

                        let cur_x = from_loc.x as f64 + (to_loc.x - from_loc.x) as f64 * ease;
                        let cur_y = from_loc.y as f64 + (to_loc.y - from_loc.y) as f64 * ease;
                        let cur_loc = Point::from((cur_x.round() as i32, cur_y.round() as i32));

                        let window_match = state.space.elements().find(|w| {
                            crate::render::window_key(w) == Some(key)
                        }).cloned();
                        if let Some(window) = window_match {
                            state.space.map_element(window, cur_loc, false);
                        }

                        if t >= 1.0 {
                            completed_anims.push(key);
                        }
                    }
                }
                for key in completed_anims {
                    state.window_animations.remove(&key);
                }

                // Cleanup finished opening animations (> 200ms)
                state.window_open_animations.retain(|_, start| {
                    now.duration_since(*start).as_millis() < 200
                });

                // Process closing animations
                let mut finished_closing = Vec::new();
                for (&key, &(start, ref window)) in &state.window_close_animations {
                    let elapsed = now.duration_since(start).as_millis();
                    if elapsed >= 160 {
                        finished_closing.push((key, window.clone()));
                    }
                }
                for (key, window) in finished_closing {
                    state.window_close_animations.remove(&key);
                    if let Some(toplevel) = window.toplevel() {
                        toplevel.send_close();
                    }
                    state.space.unmap_elem(&window);
                    state.deco.forget(key);
                }

                let size = backend.window_size();
                let damage = Rectangle::from_size(size);

                {
                    let (renderer, mut framebuffer) = backend.bind().unwrap();
                    crate::render::render(
                        state,
                        renderer,
                        &mut framebuffer,
                        &output,
                        &mut damage_tracker,
                    )
                    .unwrap();
                }
                backend.submit(Some(&[damage])).unwrap();

                state.space.elements().for_each(|window| {
                    window.send_frame(
                        &output,
                        state.start_time.elapsed(),
                        Some(Duration::ZERO),
                        |_, _| Some(output.clone()),
                    )
                });

                state.space.refresh();
                state.popups.cleanup();
                let _ = display.flush_clients();

                // Ask for redraw to schedule new frame.
                backend.window().request_redraw();
            }
            WinitEvent::CloseRequested => {
                state.loop_signal.stop();
            }
            _ => (),
        };
    })?;

    Ok(())
}
