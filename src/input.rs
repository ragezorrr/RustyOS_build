use smithay::{
    backend::input::{
        AbsolutePositionEvent, Axis, AxisSource, ButtonState, Event, InputBackend, InputEvent,
        KeyState, KeyboardKeyEvent, PointerAxisEvent, PointerButtonEvent, PointerMotionEvent,
    },
    input::{
        keyboard::{keysyms, FilterResult},
        pointer::{AxisFrame, ButtonEvent, Focus, GrabStartData as PointerGrabStartData, MotionEvent},
    },
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Point, SERIAL_COUNTER},
};

use crate::grabs::{MoveSurfaceGrab, ResizeEdge, ResizeSurfaceGrab};
use crate::render::{traffic_light_at, window_key, Light, TITLEBAR_H};
use crate::state::Aqua;
use crate::ui::UiAction;

impl Aqua {
    pub fn process_input_event<I: InputBackend>(&mut self, event: InputEvent<I>) {
        match event {
            InputEvent::Keyboard { event, .. } => {
                let serial = SERIAL_COUNTER.next_serial();
                let time = Event::time_msec(&event);
                let pressed = event.state() == KeyState::Pressed;
                let Some(keyboard) = self.seat.get_keyboard() else {
                    return;
                };

                keyboard.input::<(), _>(
                    self,
                    event.key_code(),
                    event.state(),
                    serial,
                    time,
                    |state, modifiers, keysym| {
                        let sym = keysym.modified_sym().raw();

                        // 1. Spotlight keyboard handling
                        if state.spotlight.open {
                            if pressed {
                                match sym {
                                    keysyms::KEY_Escape => {
                                        state.spotlight.close();
                                        return FilterResult::Intercept(());
                                    }
                                    keysyms::KEY_Return | keysyms::KEY_KP_Enter => {
                                        if let Some(app) = state.spotlight.selected_app().cloned() {
                                            state.spotlight.close();
                                            state.spawn_cmd(&app.exec);
                                        }
                                        return FilterResult::Intercept(());
                                    }
                                    keysyms::KEY_Down => {
                                        state.spotlight.select_next();
                                        return FilterResult::Intercept(());
                                    }
                                    keysyms::KEY_Up => {
                                        state.spotlight.select_prev();
                                        return FilterResult::Intercept(());
                                    }
                                    keysyms::KEY_BackSpace => {
                                        state.spotlight.backspace();
                                        return FilterResult::Intercept(());
                                    }
                                    _ => {
                                        // Printable characters
                                        if let Some(c) = char::from_u32(sym) {
                                            if !c.is_control() {
                                                state.spotlight.input_char(c);
                                                return FilterResult::Intercept(());
                                            }
                                        }
                                    }
                                }
                            }
                            return FilterResult::Intercept(());
                        }

                        // 2. Global Shortcuts
                        if pressed && modifiers.logo {
                            if sym == keysyms::KEY_space {
                                state.spotlight.toggle();
                                return FilterResult::Intercept(());
                            }

                            if modifiers.shift
                                && (sym == keysyms::KEY_d || sym == keysyms::KEY_D)
                            {
                                state.toggle_theme();
                                tracing::info!("appearance toggled");
                                return FilterResult::Intercept(());
                            }

                            match sym {
                                keysyms::KEY_q | keysyms::KEY_Q => {
                                    state.close_focused();
                                    return FilterResult::Intercept(());
                                }
                                keysyms::KEY_Return | keysyms::KEY_KP_Enter => {
                                    state.spawn("konsole");
                                    return FilterResult::Intercept(());
                                }
                                keysyms::KEY_Tab => {
                                    // App Switcher (Cmd+Tab or Alt+Tab)
                                    let mut items = Vec::new();
                                    for w in state.space.elements() {
                                        let key = crate::render::window_key(w).unwrap_or(0);
                                        let title = crate::render::window_title(w).unwrap_or_else(|| "Окно".into());
                                        let app_id = crate::render::window_app_id(w);
                                        items.push(crate::control_center::SwitcherItem { key, title, app_id });
                                    }
                                    for (&key, (w, _)) in &state.minimized_windows {
                                        let title = crate::render::window_title(w).unwrap_or_else(|| "Окно".into());
                                        let app_id = crate::render::window_app_id(w);
                                        items.push(crate::control_center::SwitcherItem {
                                            key,
                                            title: format!("{} (свернуто)", title),
                                            app_id,
                                        });
                                    }
                                    state.switcher.open_or_next(items);
                                    return FilterResult::Intercept(());
                                }
                                _ => {}
                            }
                        }

                        // Also support Alt+Tab
                        if pressed && modifiers.alt && sym == keysyms::KEY_Tab {
                            let mut items = Vec::new();
                            for w in state.space.elements() {
                                let key = crate::render::window_key(w).unwrap_or(0);
                                let title = crate::render::window_title(w).unwrap_or_else(|| "Окно".into());
                                let app_id = crate::render::window_app_id(w);
                                items.push(crate::control_center::SwitcherItem { key, title, app_id });
                            }
                            for (&key, (w, _)) in &state.minimized_windows {
                                let title = crate::render::window_title(w).unwrap_or_else(|| "Окно".into());
                                let app_id = crate::render::window_app_id(w);
                                items.push(crate::control_center::SwitcherItem {
                                    key,
                                    title: format!("{} (свернуто)", title),
                                    app_id,
                                });
                            }
                            state.switcher.open_or_next(items);
                            return FilterResult::Intercept(());
                        }

                        // Escape cancels App Switcher
                        if pressed && sym == keysyms::KEY_Escape && state.switcher.active {
                            state.switcher.close();
                            return FilterResult::Intercept(());
                        }

                        // Release logo/alt key closes App Switcher and focuses selected window
                        if !pressed && (!modifiers.logo && !modifiers.alt || sym == keysyms::KEY_Super_L || sym == keysyms::KEY_Super_R || sym == keysyms::KEY_Alt_L || sym == keysyms::KEY_Alt_R) {
                            if state.switcher.active {
                                if let Some(key) = state.switcher.close() {
                                    if state.minimized_windows.contains_key(&key) {
                                        state.restore_minimized_window(key);
                                    } else {
                                        let target = state.space.elements().find(|w| crate::render::window_key(w) == Some(key)).cloned();
                                        if let Some(target) = target {
                                            state.space.raise_element(&target, true);
                                            state.focus_window(&target);
                                        }
                                    }
                                }
                                return FilterResult::Intercept(());
                            }
                        }

                        // Volume & Brightness media keys -> Trigger macOS OSD HUD
                        if pressed {
                            match sym {
                                keysyms::KEY_XF86AudioRaiseVolume => {
                                    state.control_center.volume = (state.control_center.volume + 0.05).min(1.0);
                                    let v = state.control_center.volume;
                                    state.osd.trigger(crate::notifications::OsdKind::Volume, v);
                                    return FilterResult::Intercept(());
                                }
                                keysyms::KEY_XF86AudioLowerVolume => {
                                    state.control_center.volume = (state.control_center.volume - 0.05).max(0.0);
                                    let v = state.control_center.volume;
                                    state.osd.trigger(crate::notifications::OsdKind::Volume, v);
                                    return FilterResult::Intercept(());
                                }
                                keysyms::KEY_XF86MonBrightnessUp => {
                                    state.control_center.brightness = (state.control_center.brightness + 0.05).min(1.0);
                                    let b = state.control_center.brightness;
                                    state.osd.trigger(crate::notifications::OsdKind::Brightness, b);
                                    return FilterResult::Intercept(());
                                }
                                keysyms::KEY_XF86MonBrightnessDown => {
                                    state.control_center.brightness = (state.control_center.brightness - 0.05).max(0.05);
                                    let b = state.control_center.brightness;
                                    state.osd.trigger(crate::notifications::OsdKind::Brightness, b);
                                    return FilterResult::Intercept(());
                                }
                                _ => {}
                            }
                        }

                        if pressed && sym == keysyms::KEY_Escape {
                            if state.menu.active_menu.is_some() {
                                state.menu.close();
                                return FilterResult::Intercept(());
                            }
                            if state.control_center.open {
                                state.control_center.close();
                                return FilterResult::Intercept(());
                            }
                        }

                        FilterResult::Forward
                    },
                );
            }
            InputEvent::PointerMotion { event, .. } => {
                let Some(pointer) = self.seat.get_pointer() else {
                    return;
                };
                let serial = SERIAL_COUNTER.next_serial();
                let current_pos = pointer.current_location();
                let delta = event.delta();
                let mut new_pos = current_pos + delta;

                // Clamp to output bounds
                let geo = self.output_geometry();
                new_pos.x = new_pos.x.clamp(geo.loc.x as f64, (geo.loc.x + geo.size.w) as f64);
                new_pos.y = new_pos.y.clamp(geo.loc.y as f64, (geo.loc.y + geo.size.h) as f64);

                // If settings window is being dragged, update its position
                if let Some((start_pos, start_loc)) = self.settings_dragging {
                    let dx = (new_pos.x - start_pos.x).round() as i32;
                    let dy = (new_pos.y - start_pos.y).round() as i32;
                    self.settings.loc = Some((start_loc.x + dx, start_loc.y + dy));
                    pointer.motion(
                        self,
                        None,
                        &MotionEvent {
                            location: new_pos,
                            serial,
                            time: event.time_msec(),
                        },
                    );
                    pointer.frame(self);
                    return;
                }

                // If control center slider is being dragged, update value
                if let Some(slider) = self.control_center.dragging_slider {
                    let cc_x = geo.size.w - crate::control_center::CC_W - 12;
                    let cx = (new_pos.x as i32 - cc_x).clamp(24, 296);
                    let val = ((cx - 24) as f32 / 272.0).clamp(0.0, 1.0);
                    match slider {
                        crate::control_center::ControlSlider::Brightness => {
                            let b = val.clamp(0.05, 1.0);
                            self.control_center.brightness = b;
                            self.settings.display.brightness = b;
                            self.osd.trigger(crate::notifications::OsdKind::Brightness, b);
                        }
                        crate::control_center::ControlSlider::Volume => {
                            self.control_center.volume = val;
                            self.osd.trigger(crate::notifications::OsdKind::Volume, val);
                        }
                    }
                    pointer.motion(
                        self,
                        None,
                        &MotionEvent {
                            location: new_pos,
                            serial,
                            time: event.time_msec(),
                        },
                    );
                    pointer.frame(self);
                    return;
                }

                let under = self.surface_under(new_pos);

                pointer.motion(
                    self,
                    under,
                    &MotionEvent {
                        location: new_pos,
                        serial,
                        time: event.time_msec(),
                    },
                );
                pointer.frame(self);
            }
            InputEvent::PointerMotionAbsolute { event, .. } => {
                let output_geo = self.output_geometry();
                let pos = event.position_transformed(output_geo.size) + output_geo.loc.to_f64();
                let serial = SERIAL_COUNTER.next_serial();
                let Some(pointer) = self.seat.get_pointer() else {
                    return;
                };

                // If settings window is being dragged, update its position
                if let Some((start_pos, start_loc)) = self.settings_dragging {
                    let dx = (pos.x - start_pos.x).round() as i32;
                    let dy = (pos.y - start_pos.y).round() as i32;
                    self.settings.loc = Some((start_loc.x + dx, start_loc.y + dy));
                    pointer.motion(
                        self,
                        None,
                        &MotionEvent {
                            location: pos,
                            serial,
                            time: event.time_msec(),
                        },
                    );
                    pointer.frame(self);
                    return;
                }

                // If control center slider is being dragged, update value
                if let Some(slider) = self.control_center.dragging_slider {
                    let cc_x = output_geo.size.w - crate::control_center::CC_W - 12;
                    let cx = (pos.x as i32 - cc_x).clamp(24, 296);
                    let val = ((cx - 24) as f32 / 272.0).clamp(0.0, 1.0);
                    match slider {
                        crate::control_center::ControlSlider::Brightness => {
                            let b = val.clamp(0.05, 1.0);
                            self.control_center.brightness = b;
                            self.settings.display.brightness = b;
                            self.osd.trigger(crate::notifications::OsdKind::Brightness, b);
                        }
                        crate::control_center::ControlSlider::Volume => {
                            self.control_center.volume = val;
                            self.osd.trigger(crate::notifications::OsdKind::Volume, val);
                        }
                    }
                    pointer.motion(
                        self,
                        None,
                        &MotionEvent {
                            location: pos,
                            serial,
                            time: event.time_msec(),
                        },
                    );
                    pointer.frame(self);
                    return;
                }

                let under = self.surface_under(pos);

                pointer.motion(
                    self,
                    under,
                    &MotionEvent {
                        location: pos,
                        serial,
                        time: event.time_msec(),
                    },
                );
                pointer.frame(self);
            }
            InputEvent::PointerButton { event, .. } => {
                let Some(pointer) = self.seat.get_pointer() else { return; };
                let serial = SERIAL_COUNTER.next_serial();
                let button = event.button_code();
                let button_state = event.state();

                if ButtonState::Pressed == button_state && !pointer.is_grabbed() {
                    let pos = pointer.current_location();

                    // Check menu click if open
                    if let Some((_, menu_pos)) = self.menu.active_menu.clone() {
                        let mx = pos.x as i32 - menu_pos.x;
                        let my = pos.y as i32 - menu_pos.y;
                        if mx >= 0 && mx < crate::spotlight::MENU_W && my >= 0 {
                            // Find clicked item
                            let mut cur_y = 6;
                            let mut clicked_action = None;
                            for item in &self.menu.items {
                                if item.is_separator {
                                    cur_y += 9;
                                } else {
                                    if my >= cur_y && my < cur_y + 26 {
                                        clicked_action = Some(item.action.clone());
                                        break;
                                    }
                                    cur_y += 26;
                                }
                            }
                            if let Some(action) = clicked_action {
                                self.menu.close();
                                match action {
                                    crate::spotlight::MenuAction::Spawn(cmd) => self.spawn(cmd),
                                    crate::spotlight::MenuAction::ToggleTheme => self.toggle_theme(),
                                    crate::spotlight::MenuAction::ToggleSettings => {
                                        self.settings.toggle();
                                    }
                                    crate::spotlight::MenuAction::CloseWindow => self.close_focused(),
                                    crate::spotlight::MenuAction::MinimizeWindow => {
                                        if let Some(w) = self.focused_window.clone() {
                                            self.minimize_window(&w);
                                        }
                                    }
                                    crate::spotlight::MenuAction::ToggleMaximize => {
                                        if let Some(w) = self.focused_window.clone() {
                                            self.toggle_maximized(&w);
                                        }
                                    }
                                    crate::spotlight::MenuAction::About => {
                                        self.settings.current_tab = crate::settings::SettingsTab::About;
                                        if !self.settings.open {
                                            self.settings.open = true;
                                        }
                                    }
                                    crate::spotlight::MenuAction::Restart => {
                                        std::process::Command::new("reboot").spawn().ok();
                                    }
                                    crate::spotlight::MenuAction::Shutdown => {
                                        std::process::Command::new("poweroff").spawn().ok();
                                    }
                                    crate::spotlight::MenuAction::None => {}
                                }
                                pointer.frame(self);
                                return;
                            }
                        }
                        // Click outside menu closes it
                        self.menu.close();
                    }

                    // Check Spotlight click if open
                    if self.spotlight.open {
                        let geo = self.output_geometry();
                        let spot_x = (geo.size.w - 620) / 2;
                        let spot_y = geo.size.h / 5;
                        let sx = pos.x as i32 - spot_x;
                        let sy = pos.y as i32 - spot_y;
                        let rows = self.spotlight.filtered_apps.len() as i32;
                        let spot_h = 56 + if rows > 0 { rows * 46 + 12 } else { 0 };

                        if sx >= 0 && sx < 620 && sy >= 56 && sy < spot_h {
                            let item_idx = ((sy - 56 - 6) / 46).clamp(0, rows - 1) as usize;
                            if let Some(app) = self.spotlight.filtered_apps.get(item_idx).cloned() {
                                self.spotlight.close();
                                self.spawn_cmd(&app.exec);
                                pointer.frame(self);
                                return;
                            }
                        } else if sx < 0 || sx >= 620 || sy < 0 || sy >= spot_h {
                            // Click outside closes Spotlight
                            self.spotlight.close();
                        }
                    }

                    // Check Control Center click if open
                    if self.control_center.open {
                        let geo = self.output_geometry();
                        let cc_x = geo.size.w - crate::control_center::CC_W - 12;
                        let cc_y = crate::ui::PANEL_H + 8;
                        let cx = pos.x as i32 - cc_x;
                        let cy = pos.y as i32 - cc_y;

                        if cx >= 0 && cx < crate::control_center::CC_W && cy >= 0 && cy < crate::control_center::CC_H {
                            // 1. Wi-Fi click
                            if cx >= 12 && cx < 154 && cy >= 12 && cy < 56 {
                                self.control_center.wifi_enabled = !self.control_center.wifi_enabled;
                                pointer.frame(self);
                                return;
                            }
                            // 2. Bluetooth click
                            if cx >= 12 && cx < 154 && cy >= 56 && cy < 98 {
                                self.control_center.bluetooth_enabled = !self.control_center.bluetooth_enabled;
                                pointer.frame(self);
                                return;
                            }
                            // 3. Theme click
                            if cx >= 164 && cx < 308 && cy >= 12 && cy < 76 {
                                self.toggle_theme();
                                pointer.frame(self);
                                return;
                            }
                            // 4. Brightness slider click
                            if cx >= 24 && cx < 296 && cy >= 192 && cy < 216 {
                                let val = ((cx - 24) as f32 / 272.0).clamp(0.05, 1.0);
                                self.control_center.brightness = val;
                                self.settings.display.brightness = val;
                                self.control_center.dragging_slider = Some(crate::control_center::ControlSlider::Brightness);
                                self.osd.trigger(crate::notifications::OsdKind::Brightness, val);
                                pointer.frame(self);
                                return;
                            }
                            // 5. Volume slider click
                            if cx >= 24 && cx < 296 && cy >= 272 && cy < 296 {
                                let val = ((cx - 24) as f32 / 272.0).clamp(0.0, 1.0);
                                self.control_center.volume = val;
                                self.control_center.dragging_slider = Some(crate::control_center::ControlSlider::Volume);
                                self.osd.trigger(crate::notifications::OsdKind::Volume, val);
                                pointer.frame(self);
                                return;
                            }
                            pointer.frame(self);
                            return;
                        } else {
                            // Click outside closes Control Center
                            self.control_center.close();
                        }
                    }

                    // Check Wi-Fi / Network menu click if open
                    if self.wifi_menu.open {
                        let geo = self.output_geometry();
                        let wifi_menu_x = (geo.size.w - crate::wifi::WIFI_MENU_W - 90).max(10);
                        let wifi_menu_y = crate::ui::PANEL_H + 6;
                        let wx = pos.x as i32 - wifi_menu_x;
                        let wy = pos.y as i32 - wifi_menu_y;
                        let total_h = self.wifi_menu.total_height();

                        if wx >= 0 && wx < crate::wifi::WIFI_MENU_W && wy >= 0 && wy < total_h {
                            match self.wifi_menu.handle_click(wx, wy) {
                                crate::wifi::WifiMenuClick::ToggleWifi => {
                                    self.wifi_menu.toggle_wifi();
                                }
                                crate::wifi::WifiMenuClick::ConnectWifi(ssid) => {
                                    self.wifi_menu.connect(&ssid);
                                    self.wifi_menu.close();
                                }
                                crate::wifi::WifiMenuClick::OpenSettings => {
                                    self.wifi_menu.close();
                                    self.settings.toggle();
                                }
                                crate::wifi::WifiMenuClick::Close => {
                                    self.wifi_menu.close();
                                }
                                crate::wifi::WifiMenuClick::None => {}
                            }
                            pointer.frame(self);
                            return;
                        } else {
                            self.wifi_menu.close();
                        }
                    }

                    // Check Desktop Context Menu click if open
                    if self.desktop_menu.open {
                        let mx = pos.x as i32 - self.desktop_menu.pos.x;
                        let my = pos.y as i32 - self.desktop_menu.pos.y;
                        let h = 12 + self.desktop_menu.items.iter().map(|it| if it.is_separator { 9 } else { 26 }).sum::<i32>();
                        if mx >= 0 && mx < crate::desktop_menu::DESKTOP_MENU_W && my >= 0 && my < h {
                            let mut cur_y = 6;
                            let mut chosen = None;
                            for it in &self.desktop_menu.items {
                                if it.is_separator {
                                    cur_y += 9;
                                } else {
                                    if my >= cur_y && my < cur_y + 26 {
                                        chosen = Some(it.action);
                                        break;
                                    }
                                    cur_y += 26;
                                }
                            }
                            self.desktop_menu.close();
                            if let Some(act) = chosen {
                                match act {
                                    crate::desktop_menu::DesktopMenuAction::NewFolder => self.spawn("dolphin"),
                                    crate::desktop_menu::DesktopMenuAction::ChangeWallpaper => {
                                        self.settings.current_tab = crate::settings::SettingsTab::Wallpaper;
                                        self.settings.open = true;
                                    }
                                    crate::desktop_menu::DesktopMenuAction::OpenTerminal => self.spawn("konsole"),
                                    crate::desktop_menu::DesktopMenuAction::SystemSettings => self.settings.toggle(),
                                    _ => {}
                                }
                            }
                            pointer.frame(self);
                            return;
                        } else {
                            self.desktop_menu.close();
                        }
                    }

                    // Check Notification Center Panel click if open
                    if self.notifications.panel_open {
                        let geo = self.output_geometry();
                        let nc_x = geo.size.w - crate::notifications::NC_PANEL_W - 12;
                        let nc_y = crate::ui::PANEL_H + 8;
                        let nx = pos.x as i32 - nc_x;
                        let ny = pos.y as i32 - nc_y;
                        if nx >= 0 && nx < crate::notifications::NC_PANEL_W && ny >= 0 && ny < crate::notifications::NC_PANEL_H {
                            // "Очистить всё" button at nx: 250..350, ny: 15..40
                            if nx >= 240 && nx <= 350 && ny >= 15 && ny <= 42 {
                                self.notifications.clear();
                            }
                            pointer.frame(self);
                            return;
                        } else {
                            self.notifications.close_panel();
                        }
                    }

                    // Check Stage Manager thumbnail click (left side of screen)
                    if self.stage_manager.enabled && pos.x < 90.0 {
                        let geo = self.output_geometry();
                        let windows: Vec<_> = self.space.elements().cloned().collect();
                        let inactive_windows: Vec<_> = windows.iter().filter(|w| Some(*w) != self.focused_window.as_ref()).take(4).collect();
                        let item_h = 64;
                        let gap = 14;
                        let total_h = inactive_windows.len() as i32 * (item_h + gap);
                        let stage_top = (geo.size.h - total_h) / 2;

                        if (pos.y as i32) >= stage_top && (pos.y as i32) < stage_top + total_h {
                            let idx = ((pos.y as i32 - stage_top) / (item_h + gap)) as usize;
                            if let Some(target) = inactive_windows.get(idx) {
                                self.space.raise_element(target, true);
                                self.focus_window(target);
                                pointer.frame(self);
                                return;
                            }
                        }
                    }

                    // Check Settings click if open
                    if self.settings.open {
                        let geo = self.output_geometry();
                        let (set_x, set_y) = self.settings.loc.unwrap_or_else(|| {
                            (
                                (geo.size.w - crate::settings::SETTINGS_W) / 2,
                                (geo.size.h - crate::settings::SETTINGS_H) / 2,
                            )
                        });
                        let sx = pos.x as i32 - set_x;
                        let sy = pos.y as i32 - set_y;

                        if sx >= 0 && sx < crate::settings::SETTINGS_W && sy >= 0 && sy < crate::settings::SETTINGS_H {
                            // Traffic light close button (top left of settings window: x: 10..30, y: 10..32)
                            if sx >= 10 && sx <= 30 && sy >= 10 && sy <= 32 {
                                self.settings.close();
                                self.settings_dragging = None;
                                pointer.frame(self);
                                return;
                            }
                            // Window header drag area (top 44px)
                            if sy >= 0 && sy < 44 {
                                self.settings_dragging = Some((pos, Point::from((set_x, set_y))));
                                pointer.frame(self);
                                return;
                            }
                            let action = self.settings.handle_click(sx, sy, &mut self.shell.theme);
                            match action {
                                crate::settings::SettingsAction::BrightnessChanged(val) => {
                                    self.control_center.brightness = val;
                                    self.osd.trigger(crate::notifications::OsdKind::Brightness, val);
                                }
                                crate::settings::SettingsAction::ApplyDisplay { width, height, scale } => {
                                    self.apply_display_mode(width, height, scale);
                                }
                                crate::settings::SettingsAction::CycleKeyboardLayout => {
                                    self.cycle_keyboard_layout();
                                }
                                crate::settings::SettingsAction::ThemeChanged | crate::settings::SettingsAction::WallpaperChanged => {
                                    // Triggered automatic theme persistence and re-render
                                }
                                crate::settings::SettingsAction::None => {}
                            }
                            pointer.frame(self);
                            return;
                        }
                    }

                    // 1. Shell (menu bar / dock) takes priority.
                    if let Some(action) = self.shell.hit_test(pos) {
                        match action {
                            UiAction::Launch(command, index) => {
                                self.shell.trigger_bounce(index);
                                if index == 5 {
                                    self.settings.toggle();
                                } else {
                                    let app_def = &crate::ui::APPS[index];
                                    let id = app_def.id.to_lowercase();
                                    let name = app_def.name.to_lowercase();

                                    // 1. Check if there is a minimized window for this app -> Restore it!
                                    let mut restored_key = None;
                                    for (&key, (win, _)) in &self.minimized_windows {
                                        let app_id = crate::render::window_app_id(win).unwrap_or_default().to_lowercase();
                                        let title = crate::render::window_title(win).unwrap_or_default().to_lowercase();
                                        if app_id.contains(&id) || app_id.contains(&name) || title.contains(&id) || title.contains(&name) {
                                            restored_key = Some(key);
                                            break;
                                        }
                                    }
                                    if let Some(key) = restored_key {
                                        self.restore_minimized_window(key);
                                    } else {
                                        // 2. Check if window is already open in space -> Raise and focus!
                                        let mut open_win = None;
                                        for win in self.space.elements() {
                                            let app_id = crate::render::window_app_id(win).unwrap_or_default().to_lowercase();
                                            let title = crate::render::window_title(win).unwrap_or_default().to_lowercase();
                                            if app_id.contains(&id) || app_id.contains(&name) || title.contains(&id) || title.contains(&name) {
                                                open_win = Some(win.clone());
                                                break;
                                            }
                                        }
                                        if let Some(win) = open_win {
                                            self.space.raise_element(&win, true);
                                            self.focus_window(&win);
                                        } else {
                                            // 3. Otherwise spawn new instance
                                            self.spawn(command);
                                        }
                                    }
                                }
                            }
                            UiAction::ActivateWindow(key) => {
                                if self.minimized_windows.contains_key(&key) {
                                    self.restore_minimized_window(key);
                                } else {
                                    let target = self.space.elements().find(|w| crate::render::window_key(w) == Some(key)).cloned();
                                    if let Some(target) = target {
                                        self.space.raise_element(&target, true);
                                        self.focus_window(&target);
                                    }
                                }
                            }
                            UiAction::RestoreMinimized(key) => {
                                self.restore_minimized_window(key);
                            }
                            UiAction::ToggleTheme => self.toggle_theme(),
                            UiAction::OpenMenu(kind, x, y) => {
                                self.menu.open(kind, Point::from((x, y)));
                            }
                            UiAction::ToggleSpotlight => {
                                self.spotlight.toggle();
                            }
                            UiAction::ToggleControlCenter => {
                                self.control_center.toggle();
                            }
                            UiAction::ToggleNotifications => {
                                self.notifications.toggle_panel();
                            }
                            UiAction::ToggleWifi => {
                                self.wifi_menu.toggle();
                            }
                            UiAction::CycleKeyboardLayout => {
                                self.cycle_keyboard_layout();
                            }
                        }
                        pointer.frame(self);
                        return;
                    }

                    // 2. Window interaction (traffic lights, titlebar drag & border resize).
                    // First check window borders across all windows (top to bottom)
                    const BORDER_GRAB: f64 = 8.0;
                    const BTN_LEFT: u32 = 0x110;

                    let mut resize_candidate = None;
                    for window in self.space.elements().rev() {
                        let Some(loc) = self.space.element_location(window) else {
                            continue;
                        };
                        let size = window.geometry().size;
                        let tb_h = crate::render::window_titlebar_h(window);
                        let total_h = (size.h + tb_h) as f64;
                        let w_f64 = size.w as f64;
                        let loc_x = loc.x as f64;
                        let loc_y = loc.y as f64;

                        // Check outer boundary +/- BORDER_GRAB
                        let in_outer = pos.x >= loc_x - BORDER_GRAB
                            && pos.x < loc_x + w_f64 + BORDER_GRAB
                            && pos.y >= loc_y - BORDER_GRAB
                            && pos.y < loc_y + total_h + BORDER_GRAB;

                        if in_outer {
                            let mut edge = ResizeEdge::empty();
                            if pos.x < loc_x + BORDER_GRAB {
                                edge |= ResizeEdge::LEFT;
                            }
                            if pos.x >= loc_x + w_f64 - BORDER_GRAB {
                                edge |= ResizeEdge::RIGHT;
                            }
                            if pos.y < loc_y + BORDER_GRAB {
                                edge |= ResizeEdge::TOP;
                            }
                            if pos.y >= loc_y + total_h - BORDER_GRAB {
                                edge |= ResizeEdge::BOTTOM;
                            }

                            if !edge.is_empty() {
                                resize_candidate = Some((window.clone(), loc, size, edge));
                                break;
                            }
                        }
                    }

                    if let Some((window, location, size, edge)) = resize_candidate {
                        if button == BTN_LEFT {
                            self.space.raise_element(&window, true);
                            self.focus_window(&window);
                            let start_data = PointerGrabStartData {
                                focus: None,
                                button,
                                location: pos,
                            };
                            let grab = ResizeSurfaceGrab::start(
                                start_data,
                                window,
                                edge,
                                smithay::utils::Rectangle::new(location, size),
                            );
                            pointer.set_grab(self, grab, serial, Focus::Clear);
                            pointer.frame(self);
                            return;
                        }
                    }

                    // Check traffic lights and titlebar drag (for SSD windows only)
                    let window_under_titlebar = self.space.elements().rev().find_map(|w| {
                        if crate::render::window_is_csd(w) {
                            return None;
                        }
                        let loc = self.space.element_location(w)?;
                        let size = w.geometry().size;
                        let in_titlebar = pos.x >= loc.x as f64
                            && pos.x < (loc.x + size.w) as f64
                            && pos.y >= loc.y as f64
                            && pos.y < (loc.y + TITLEBAR_H) as f64;
                        if in_titlebar {
                            Some((w.clone(), loc))
                        } else {
                            None
                        }
                    });

                    if let Some((window, location)) = window_under_titlebar {
                        if let Some(light) = traffic_light_at(location, pos) {
                            let _key = window_key(&window).unwrap_or(0);
                            match light {
                                Light::Close => {
                                    self.close_window(&window);
                                }
                                Light::Minimize => {
                                    self.minimize_window(&window);
                                }
                                Light::Maximize => {
                                    self.space.raise_element(&window, true);
                                    self.focus_window(&window);
                                    self.toggle_maximized(&window);
                                }
                            }
                            pointer.frame(self);
                            return;
                        }

                        // Titlebar click / drag
                        self.space.raise_element(&window, true);
                        self.focus_window(&window);
                        self.space.elements().for_each(|w| {
                            if let Some(toplevel) = w.toplevel() {
                                toplevel.send_pending_configure();
                            }
                        });

                        if button == BTN_LEFT {
                            let key = window_key(&window).unwrap_or(0);
                            let now = std::time::Instant::now();
                            let is_double_click = if let Some((prev_key, prev_time)) = self.last_titlebar_click {
                                prev_key == key && now.duration_since(prev_time).as_millis() < 350
                            } else {
                                false
                            };

                            if is_double_click {
                                self.last_titlebar_click = None;
                                self.toggle_maximized(&window);
                                pointer.frame(self);
                                return;
                            } else {
                                self.last_titlebar_click = Some((key, now));
                            }

                            let start_data = PointerGrabStartData {
                                focus: None,
                                button,
                                location: pos,
                            };
                            let grab = MoveSurfaceGrab {
                                start_data,
                                window,
                                initial_window_location: location,
                            };
                            pointer.set_grab(self, grab, serial, Focus::Clear);
                            pointer.frame(self);
                            return;
                        }
                    }

                    // 3. Normal client interaction.
                    if let Some((window, _loc)) = self
                        .space
                        .element_under(pos)
                        .map(|(w, l)| (w.clone(), l))
                    {
                        self.space.raise_element(&window, true);
                        self.focus_window(&window);
                        self.space.elements().for_each(|w| {
                            if let Some(toplevel) = w.toplevel() {
                                toplevel.send_pending_configure();
                            }
                        });
                    } else {
                        // Click on wallpaper (empty space)
                        // If Right click (BTN_RIGHT = 0x111): open Desktop Context Menu!
                        const BTN_RIGHT: u32 = 0x111;
                        if button == BTN_RIGHT {
                            self.desktop_menu.open_at(Point::from((pos.x as i32, pos.y as i32)));
                            pointer.frame(self);
                            return;
                        }

                        self.focused_window = None;
                        self.space.elements().for_each(|w| {
                            w.set_activated(false);
                            if let Some(toplevel) = w.toplevel() {
                                toplevel.send_pending_configure();
                            }
                        });
                        if let Some(keyboard) = self.seat.get_keyboard() {
                            keyboard.set_focus(self, Option::<WlSurface>::None, serial);
                        }
                    }
                }

                if ButtonState::Released == button_state {
                    self.settings_dragging = None;
                    self.control_center.dragging_slider = None;
                }

                pointer.button(
                    self,
                    &ButtonEvent {
                        button,
                        state: button_state,
                        serial,
                        time: event.time_msec(),
                    },
                );
                pointer.frame(self);
            }
            InputEvent::PointerAxis { event, .. } => {
                let source = event.source();

                let horizontal_amount = event
                    .amount(Axis::Horizontal)
                    .unwrap_or_else(|| event.amount_v120(Axis::Horizontal).unwrap_or(0.0) * 15.0 / 120.);
                let vertical_amount = event
                    .amount(Axis::Vertical)
                    .unwrap_or_else(|| event.amount_v120(Axis::Vertical).unwrap_or(0.0) * 15.0 / 120.);
                let horizontal_amount_discrete = event.amount_v120(Axis::Horizontal);
                let vertical_amount_discrete = event.amount_v120(Axis::Vertical);

                let mut frame = AxisFrame::new(event.time_msec()).source(source);
                if horizontal_amount != 0.0 {
                    frame = frame.value(Axis::Horizontal, horizontal_amount);
                    if let Some(discrete) = horizontal_amount_discrete {
                        frame = frame.v120(Axis::Horizontal, discrete as i32);
                    }
                }
                if vertical_amount != 0.0 {
                    frame = frame.value(Axis::Vertical, vertical_amount);
                    if let Some(discrete) = vertical_amount_discrete {
                        frame = frame.v120(Axis::Vertical, discrete as i32);
                    }
                }

                if source == AxisSource::Finger {
                    if event.amount(Axis::Horizontal) == Some(0.0) {
                        frame = frame.stop(Axis::Horizontal);
                    }
                    if event.amount(Axis::Vertical) == Some(0.0) {
                        frame = frame.stop(Axis::Vertical);
                    }
                }

                let Some(pointer) = self.seat.get_pointer() else { return; };
                pointer.axis(self, frame);
                pointer.frame(self);
            }
            _ => {}
        }
    }
}
