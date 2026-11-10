use std::{
    cell::RefCell,
    collections::HashMap,
    os::unix::io::OwnedFd,
    path::PathBuf,
    rc::Rc,
    time::Duration,
};

use smithay::{
    backend::{
        allocator::{
            gbm::{GbmAllocator, GbmBufferFlags},
            Fourcc,
        },
        drm::{DrmDevice, DrmDeviceFd, DrmEvent, GbmBufferedSurface},
        egl::{EGLContext, EGLDisplay},
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        renderer::{
            damage::OutputDamageTracker,
            gles::GlesRenderer,
            Bind, ImportDma,
        },
        session::{
            libseat::LibSeatSession,
            Event as SessionEvent, Session,
        },
        udev::{all_gpus, primary_gpu},
    },
    output::{Mode as OutputMode, Output, PhysicalProperties, Subpixel},
    reexports::{
        calloop::{
            ping::make_ping,
            timer::{TimeoutAction, Timer},
            EventLoop,
        },
        drm::{
            buffer::Buffer,
            control::{connector, crtc, dumbbuffer::DumbBuffer, Device as ControlDevice},
        },
        gbm::Device as GbmDevice,
        input::Libinput,
        rustix::fs::OFlags,
    },
    utils::{DeviceFd, Point, Transform},
};

use crate::{CalloopData, Aqua};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DrmOutputState {
    #[default]
    Idle,
    WaitingForVBlank { redraw_needed: bool },
}

pub struct DrmOutput {
    pub surface: GbmBufferedSurface<GbmAllocator<DrmDeviceFd>, ()>,
    pub output: Output,
    pub damage_tracker: OutputDamageTracker,
    pub state: DrmOutputState,
    pub last_frame_queued: Option<std::time::Instant>,
    pub cursor_buffer: Option<DumbBuffer>,
}

struct UdevBackend {
    outputs: HashMap<crtc::Handle, DrmOutput>,
    renderer: GlesRenderer,
}

pub fn run_udev(
    mut event_loop: EventLoop<CalloopData>,
    mut data: CalloopData,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("Initializing native DRM/udev backend...");

    let (mut session, notifier) = match LibSeatSession::new() {
        Ok(s) => s,
        Err(err) => {
            tracing::error!(?err, "Failed to initialize libseat session");
            return Err(Box::new(err));
        }
    };

    let seat_name = session.seat();
    tracing::info!(%seat_name, "Libseat session opened successfully");

    // 1. Initialize libinput on this session
    let mut libinput_context = Libinput::new_with_udev(LibinputSessionInterface::from(session.clone()));
    libinput_context.udev_assign_seat(&seat_name).map_err(|_| "Failed to assign udev seat to libinput")?;
    let libinput_backend = LibinputInputBackend::new(libinput_context);

    // 2. Discover primary GPU
    let gpu_path = primary_gpu(&seat_name)
        .ok()
        .flatten()
        .or_else(|| all_gpus(&seat_name).ok().and_then(|mut g| g.pop()))
        .unwrap_or_else(|| PathBuf::from("/dev/dri/card0"));

    tracing::info!(?gpu_path, "Selected GPU node");

    let fd: OwnedFd = session.open(&gpu_path, OFlags::RDWR | OFlags::CLOEXEC)?;
    let drm_fd = DrmDeviceFd::new(DeviceFd::from(fd));

    let (mut drm_device, drm_notifier) = DrmDevice::new(drm_fd.clone(), false)?;
    let gbm_device = GbmDevice::new(drm_fd.clone())?;

    let egl_display = unsafe { EGLDisplay::new(gbm_device.clone())? };
    let egl_context = EGLContext::new(&egl_display)?;
    let renderer = unsafe { GlesRenderer::new(egl_context)? };

    let mut outputs: HashMap<crtc::Handle, DrmOutput> = HashMap::new();

    // 3. Scan connectors and setup outputs
    let res = drm_device.resource_handles()?;
    for &conn_handle in res.connectors() {
        let conn_info = drm_device.get_connector(conn_handle, false)?;
        if conn_info.state() == connector::State::Connected {
            let preferred_mode = conn_info
                .modes()
                .iter()
                .find(|m| m.mode_type().contains(smithay::reexports::drm::control::ModeTypeFlags::PREFERRED))
                .or_else(|| {
                    // Otherwise pick the mode with the highest refresh rate
                    conn_info.modes().iter().max_by_key(|m| m.vrefresh())
                })
                .copied()
                .or_else(|| conn_info.modes().first().copied());

            if let Some(mode) = preferred_mode {
                // Find a free CRTC for this connector
                let crtc_handle = res.crtcs().iter().find(|crtc| !outputs.contains_key(crtc)).copied();
                if let Some(crtc) = crtc_handle {
                    tracing::info!(?conn_handle, ?crtc, ?mode, "Configuring connected display");
                    let drm_surface = drm_device.create_surface(crtc, mode, &[conn_handle])?;
                    let gbm_allocator = GbmAllocator::new(gbm_device.clone(), GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT);
                    let color_formats = [Fourcc::Argb8888, Fourcc::Xrgb8888];
                    let render_formats = renderer.dmabuf_formats().into_iter().collect::<Vec<_>>();

                    let gbm_surface = GbmBufferedSurface::new(
                        drm_surface,
                        gbm_allocator,
                        &color_formats,
                        render_formats,
                    )?;

                    let output_size = (mode.size().0 as i32, mode.size().1 as i32);
                    let output_mode = OutputMode {
                        size: output_size.into(),
                        refresh: (mode.vrefresh() * 1000) as i32,
                    };

                    let conn_name = format!("{}-{}", conn_info.interface().as_str(), conn_info.interface_id());
                    let display_info = crate::settings::detect_display_info();
                    let output = Output::new(
                        conn_name.clone(),
                        PhysicalProperties {
                            size: conn_info.size().map(|(w, h)| (w as i32, h as i32).into()).unwrap_or_else(|| (344, 194).into()),
                            subpixel: Subpixel::Unknown,
                            make: display_info.name.clone(),
                            model: conn_name,
                        },
                    );
                    let _global = output.create_global::<Aqua>(&mut data.display_handle);
                    output.change_current_state(Some(output_mode), Some(Transform::Normal), None, Some((0, 0).into()));
                    output.set_preferred(output_mode);

                    data.state.space.map_output(&output, (0, 0));
                    let damage_tracker = OutputDamageTracker::from_output(&output);

                    let mut cursor_buffer = None;
                    if let Ok(mut dumb) = drm_fd.create_dumb_buffer((64, 64), smithay::reexports::drm::buffer::DrmFourcc::Argb8888, 32) {
                        let pitch = dumb.pitch() as usize;
                        if let Ok(mut map) = drm_fd.map_dumb_buffer(&mut dumb) {
                            let mut pm = tiny_skia::Pixmap::new(64, 64).expect("cursor pm");
                            let hx = 1.0;
                            let hy = 1.0;

                            let mut pb = tiny_skia::PathBuilder::new();
                            pb.move_to(hx, hy);
                            pb.line_to(hx, hy + 17.5);
                            pb.line_to(hx + 4.8, hy + 14.2);
                            pb.line_to(hx + 8.2, hy + 21.5);
                            pb.line_to(hx + 10.8, hy + 20.3);
                            pb.line_to(hx + 7.5, hy + 13.0);
                            pb.line_to(hx + 13.5, hy + 13.0);
                            pb.close();

                            if let Some(path) = pb.finish() {
                                let mut shadow_paint = tiny_skia::Paint::default();
                                shadow_paint.set_color_rgba8(0, 0, 0, 48);
                                let shadow_transform = tiny_skia::Transform::from_translate(0.8, 1.6);
                                let shadow_stroke = tiny_skia::Stroke {
                                    width: 2.2,
                                    line_cap: tiny_skia::LineCap::Round,
                                    line_join: tiny_skia::LineJoin::Round,
                                    ..Default::default()
                                };
                                pm.stroke_path(&path, &shadow_paint, &shadow_stroke, shadow_transform, None);
                                pm.fill_path(&path, &shadow_paint, tiny_skia::FillRule::Winding, shadow_transform, None);

                                let mut stroke_paint = tiny_skia::Paint::default();
                                stroke_paint.set_color_rgba8(255, 255, 255, 255);
                                let stroke = tiny_skia::Stroke {
                                    width: 1.6,
                                    line_cap: tiny_skia::LineCap::Round,
                                    line_join: tiny_skia::LineJoin::Round,
                                    ..Default::default()
                                };
                                pm.stroke_path(&path, &stroke_paint, &stroke, tiny_skia::Transform::identity(), None);

                                let mut fill_paint = tiny_skia::Paint::default();
                                fill_paint.set_color_rgba8(18, 18, 22, 255);
                                pm.fill_path(&path, &fill_paint, tiny_skia::FillRule::Winding, tiny_skia::Transform::identity(), None);
                            }

                            let pm_data = pm.data();
                            for y in 0..64 {
                                for x in 0..64 {
                                    let src_idx = (y * 64 + x) * 4;
                                    let r = pm_data[src_idx];
                                    let g = pm_data[src_idx + 1];
                                    let b = pm_data[src_idx + 2];
                                    let a = pm_data[src_idx + 3];
                                    let dst_idx = (y * pitch) + (x * 4);
                                    if dst_idx + 3 < map.len() {
                                        map[dst_idx] = b;
                                        map[dst_idx + 1] = g;
                                        map[dst_idx + 2] = r;
                                        map[dst_idx + 3] = a;
                                    }
                                }
                            }
                        }

                        #[allow(deprecated)]
                        if drm_fd.set_cursor(crtc, Some(&dumb)).is_ok() {
                            tracing::info!(?crtc, "Hardware DRM cursor plane enabled successfully");
                            data.state.hardware_cursor = true;
                            cursor_buffer = Some(dumb);
                        }
                    }

                    outputs.insert(
                        crtc,
                        DrmOutput {
                            surface: gbm_surface,
                            output,
                            damage_tracker,
                            state: DrmOutputState::Idle,
                            last_frame_queued: None,
                            cursor_buffer,
                        },
                    );
                }
            }
        }
    }

    if outputs.is_empty() {
        tracing::warn!("No connected display found during initial DRM scan");
    }

    std::env::set_var("WAYLAND_DISPLAY", &data.state.socket_name);

    let backend = Rc::new(RefCell::new(UdevBackend { outputs, renderer }));

    // Register event sources
    let handle = event_loop.handle();

    // Create an event-driven redraw signal for instant cursor response
    let (redraw_ping, redraw_source) = make_ping()?;

    // Libinput events: move hardware cursor plane instantly with 0ms lag
    let drm_cursor_fd = drm_fd.clone();
    let redraw_ping_input = redraw_ping.clone();
    handle.insert_source(libinput_backend, move |event, _, data| {
        data.state.process_input_event(event);

        // Hardware cursor plane instant move (up to 1000Hz polling rate)
        if data.state.hardware_cursor {
            if let Some(pointer) = data.state.seat.get_pointer() {
                let loc = pointer.current_location();
                let x = loc.x.round() as i32;
                let y = loc.y.round() as i32;
                for &crtc in res.crtcs() {
                    #[allow(deprecated)]
                    let _ = drm_cursor_fd.move_cursor(crtc, (x, y));
                }
            }
        } else {
            // Software cursor fallback: wake up renderer immediately on mouse movement
            redraw_ping_input.ping();
        }
    })?;

    // Session events (VT switch activate/pause)
    let backend_session = backend.clone();
    handle.insert_source(notifier, move |event, _, _data| {
        match event {
            SessionEvent::PauseSession => {
                tracing::info!("Session paused (VT switched away)");
            }
            SessionEvent::ActivateSession => {
                tracing::info!("Session activated (VT switched back)");
                if let Ok(mut b) = backend_session.try_borrow_mut() {
                    for drm_out in b.outputs.values_mut() {
                        drm_out.state = DrmOutputState::Idle;
                        drm_out.surface.reset_buffers();
                    }
                }
            }
        }
    })?;

    // DRM page-flip events
    let backend_drm = backend.clone();
    handle.insert_source(drm_notifier, move |event, _, data| {
        match event {
            DrmEvent::VBlank(crtc) => {
                if let Ok(mut b) = backend_drm.try_borrow_mut() {
                    let now = std::time::Instant::now();
                    let UdevBackend { outputs, renderer } = &mut *b;
                    if let Some(drm_out) = outputs.get_mut(&crtc) {
                        let _ = drm_out.surface.frame_submitted();
                        let redraw_needed = match drm_out.state {
                            DrmOutputState::WaitingForVBlank { redraw_needed } => redraw_needed,
                            DrmOutputState::Idle => false,
                        };
                        drm_out.state = DrmOutputState::Idle;
                        drm_out.last_frame_queued = None;

                        if drm_out.output.current_mode().is_some() {
                            let time = data.state.start_time.elapsed();
                            let output = drm_out.output.clone();
                            data.state.space.elements().for_each(|window| {
                                window.send_frame(
                                    &output,
                                    time,
                                    Some(Duration::ZERO),
                                    |_, _| Some(output.clone()),
                                );
                            });
                        }

                        // If redraw was requested while waiting for VBlank, re-render immediately
                        if redraw_needed {
                            render_output(drm_out, renderer, &mut data.state, now);
                        }
                    }
                }
                data.state.space.refresh();
                data.state.popups.cleanup();
                let _ = data.display_handle.flush_clients();
            }
            DrmEvent::Error(err) => {
                tracing::error!(?err, "DRM event error");
            }
        }
    })?;

    // 60 FPS timer: handles client frame callbacks, window animations and flush
    let backend_render = backend.clone();
    let timer = Timer::immediate();
    handle.insert_source(timer, move |_deadline, _, data| {
        let now = std::time::Instant::now();

        // 1. Window animations
        let mut completed_anims = Vec::new();
        let anim_keys: Vec<u32> = data.state.window_animations.keys().copied().collect();
        for key in anim_keys {
            if let Some(&(start, duration, from_loc, to_loc)) = data.state.window_animations.get(&key) {
                let elapsed = now.duration_since(start).as_secs_f64();
                let total = duration.as_secs_f64();
                let t = (elapsed / total).clamp(0.0, 1.0);
                let ease = 1.0 - (1.0 - t).powi(3);

                let cur_x = from_loc.x as f64 + (to_loc.x - from_loc.x) as f64 * ease;
                let cur_y = from_loc.y as f64 + (to_loc.y - from_loc.y) as f64 * ease;
                let cur_loc = Point::from((cur_x.round() as i32, cur_y.round() as i32));

                let window_match = data.state.space.elements().find(|w| {
                    crate::render::window_key(w) == Some(key)
                }).cloned();
                if let Some(window) = window_match {
                    data.state.space.map_element(window, cur_loc, false);
                }

                if t >= 1.0 {
                    completed_anims.push(key);
                }
            }
        }
        for key in completed_anims {
            data.state.window_animations.remove(&key);
        }

        // 2. Open/Close window animations
        data.state.window_open_animations.retain(|_, start| {
            now.duration_since(*start).as_millis() < 200
        });

        let mut finished_closing = Vec::new();
        for (&key, &(start, ref window)) in &data.state.window_close_animations {
            let elapsed = now.duration_since(start).as_millis();
            if elapsed >= 160 {
                finished_closing.push((key, window.clone()));
            }
        }
        for (key, window) in finished_closing {
            data.state.window_close_animations.remove(&key);
            if let Some(toplevel) = window.toplevel() {
                toplevel.send_close();
            }
            data.state.space.unmap_elem(&window);
            data.state.deco.forget(key);
        }

        // Send frame callbacks to waiting clients on timer tick so client pacing stays smooth
        let time = data.state.start_time.elapsed();
        for window in data.state.space.elements() {
            for output in data.state.space.outputs_for_element(window) {
                window.send_frame(
                    &output,
                    time,
                    Some(Duration::ZERO),
                    |_, _| Some(output.clone()),
                );
            }
        }

        // Check outputs: if idle, render next frame or mark redraw
        if let Ok(mut b) = backend_render.try_borrow_mut() {
            let UdevBackend { outputs, renderer } = &mut *b;
            for drm_out in outputs.values_mut() {
                render_output(drm_out, renderer, &mut data.state, now);
            }
        }

        data.state.space.refresh();
        data.state.popups.cleanup();
        let _ = data.display_handle.flush_clients();

        TimeoutAction::ToDuration(Duration::from_millis(16))
    })?;

    // Event-driven redraw ping source (fires immediately when mouse moves on software cursor or client commits)
    let backend_ping = backend.clone();
    handle.insert_source(redraw_source, move |_, _, data| {
        if let Ok(mut b) = backend_ping.try_borrow_mut() {
            let now = std::time::Instant::now();
            let UdevBackend { outputs, renderer } = &mut *b;
            for drm_out in outputs.values_mut() {
                render_output(drm_out, renderer, &mut data.state, now);
            }
        }

        data.state.space.refresh();
        data.state.popups.cleanup();
        let _ = data.display_handle.flush_clients();
    })?;

    tracing::info!("Native DRM/udev event loop running");

    event_loop.run(None, &mut data, move |_| {})?;

    Ok(())
}

fn render_output(
    drm_out: &mut DrmOutput,
    renderer: &mut GlesRenderer,
    state: &mut Aqua,
    now: std::time::Instant,
) {
    if let DrmOutputState::WaitingForVBlank { ref mut redraw_needed } = drm_out.state {
        // Safety check: if VBlank was dropped or delayed for >100ms, reset to Idle
        if let Some(queued) = drm_out.last_frame_queued {
            if now.duration_since(queued).as_millis() > 100 {
                drm_out.state = DrmOutputState::Idle;
                drm_out.last_frame_queued = None;
            } else {
                // Currently waiting for VBlank: mark that redraw is needed when VBlank arrives
                *redraw_needed = true;
                return;
            }
        } else {
            *redraw_needed = true;
            return;
        }
    }

    if let Ok((mut dmabuf, _age)) = drm_out.surface.next_buffer() {
        let t_render = std::time::Instant::now();
        let render_res = if let Ok(mut target) = renderer.bind(&mut dmabuf) {
            crate::render::render(
                state,
                renderer,
                &mut target,
                &drm_out.output,
                &mut drm_out.damage_tracker,
            )
        } else {
            return;
        };
        let d_render = t_render.elapsed();
        if d_render.as_millis() >= 4 {
            tracing::warn!(?d_render, "Slow crate::render::render stage");
        }

        match render_res {
            Ok(has_damage) => {
                if has_damage {
                    let t_queue = std::time::Instant::now();
                    let queue_res = drm_out.surface.queue_buffer(None, None, ());
                    let d_queue = t_queue.elapsed();
                    if d_queue.as_millis() >= 4 {
                        tracing::warn!(?d_queue, "Slow queue_buffer stage");
                    }
                    if queue_res.is_ok() {
                        drm_out.state = DrmOutputState::WaitingForVBlank { redraw_needed: false };
                        drm_out.last_frame_queued = Some(now);
                    }
                } else {
                    // No damage: do not queue buffer to DRM plane
                    drm_out.state = DrmOutputState::Idle;
                }
            }
            Err(_) => {
                drm_out.state = DrmOutputState::Idle;
            }
        }
    }
}
