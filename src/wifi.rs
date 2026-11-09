//! Real Network & Wi-Fi scanner and popup menu for Aqua (macOS Sequoia style).

use std::process::Command;
use smithay::backend::{
    allocator::Fourcc,
    renderer::element::memory::MemoryRenderBuffer,
};
use smithay::utils::{Logical, Point, Transform};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform as SkTransform};

use crate::text::TextRenderer;
use crate::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkKind {
    Ethernet,
    Wifi,
    Disconnected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EthernetInfo {
    pub device: String,
    pub connection: String,
    pub connected: bool,
    pub is_primary: bool,
}

#[derive(Debug, Clone)]
pub struct WifiNetwork {
    pub ssid: String,
    pub active: bool,
    pub signal: u8, // 0..100
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WifiMenuClick {
    None,
    ToggleWifi,
    ConnectWifi(String),
    OpenSettings,
    Close,
}

pub struct WifiMenuState {
    pub open: bool,
    pub ethernet: Option<EthernetInfo>,
    pub networks: Vec<WifiNetwork>,
    pub current_ssid: Option<String>,
    pub wifi_enabled: bool,
    pub last_scan: Option<std::time::Instant>,
}

impl Default for WifiMenuState {
    fn default() -> Self {
        let mut state = Self {
            open: false,
            ethernet: None,
            networks: Vec::new(),
            current_ssid: None,
            wifi_enabled: true,
            last_scan: None,
        };
        state.refresh();
        state
    }
}

impl WifiMenuState {
    pub fn refresh(&mut self) {
        // 1. Detect Ethernet connection (via nmcli and /sys/class/net fallback)
        let mut eth_info: Option<EthernetInfo> = None;
        let dev_out = Command::new("nmcli")
            .args(["-t", "-f", "TYPE,STATE,DEVICE,CONNECTION", "dev"])
            .output();

        if let Ok(out) = dev_out {
            let text = String::from_utf8_lossy(&out.stdout);
            for line in text.lines() {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() >= 3 && parts[0] == "ethernet" {
                    let state = parts[1];
                    let dev = parts[2];
                    let conn = if parts.len() >= 4 && !parts[3].is_empty() {
                        parts[3]
                    } else {
                        "Проводное подключение"
                    };
                    let connected = state.contains("connected") || state.contains("подключено");
                    if connected {
                        eth_info = Some(EthernetInfo {
                            device: dev.to_string(),
                            connection: conn.to_string(),
                            connected: true,
                            is_primary: true,
                        });
                        break;
                    }
                }
            }
        }

        // Fallback: check /sys/class/net for wired interfaces
        if eth_info.is_none() {
            if let Ok(entries) = std::fs::read_dir("/sys/class/net") {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with("en") || name.starts_with("eth") {
                        let carrier = std::fs::read_to_string(entry.path().join("carrier"))
                            .map(|s| s.trim() == "1")
                            .unwrap_or(false);
                        let operstate = std::fs::read_to_string(entry.path().join("operstate"))
                            .map(|s| s.trim() == "up")
                            .unwrap_or(false);
                        if carrier && operstate {
                            eth_info = Some(EthernetInfo {
                                device: name,
                                connection: "Проводное подключение".to_string(),
                                connected: true,
                                is_primary: true,
                            });
                            break;
                        }
                    }
                }
            }
        }

        // 2. Read active SSID & scan networks via nmcli
        let output = Command::new("nmcli")
            .args(["-t", "-f", "active,ssid,signal", "dev", "wifi"])
            .output();

        let mut networks = Vec::new();
        let mut active_ssid = None;

        if let Ok(out) = output {
            let text = String::from_utf8_lossy(&out.stdout);
            for line in text.lines() {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() >= 3 {
                    let active = parts[0] == "yes" || parts[0] == "да";
                    let ssid = parts[1].trim();
                    let signal = parts[2].trim().parse::<u8>().unwrap_or(50);
                    if !ssid.is_empty() && !networks.iter().any(|n: &WifiNetwork| n.ssid == ssid) {
                        if active {
                            active_ssid = Some(ssid.to_string());
                        }
                        networks.push(WifiNetwork {
                            ssid: ssid.to_string(),
                            active,
                            signal,
                        });
                    }
                }
            }
        }

        // 3. Check if Wi-Fi device is connected/enabled
        let dev_status = Command::new("nmcli")
            .args(["dev", "status"])
            .output();
        if let Ok(out) = dev_status {
            let text = String::from_utf8_lossy(&out.stdout);
            self.wifi_enabled = text.lines().any(|l| l.contains("wifi") && (l.contains("подключено") || l.contains("connected") || l.contains("disconnected") || l.contains("отключено")));
        }

        // 4. Check default route to determine if Ethernet or Wi-Fi is primary
        let route_out = Command::new("ip")
            .args(["route", "show", "default"])
            .output();
        if let Ok(out) = route_out {
            let text = String::from_utf8_lossy(&out.stdout);
            if let Some(first_line) = text.lines().next() {
                if let Some(ref mut eth) = eth_info {
                    eth.is_primary = first_line.contains(&eth.device);
                }
            }
        }

        self.ethernet = eth_info;
        self.current_ssid = active_ssid;
        self.networks = networks;
        self.last_scan = Some(std::time::Instant::now());
    }

    pub fn network_kind(&self) -> NetworkKind {
        if let Some(ref eth) = self.ethernet {
            if eth.connected && eth.is_primary {
                return NetworkKind::Ethernet;
            }
        }
        if self.current_ssid.is_some() {
            return NetworkKind::Wifi;
        }
        if let Some(ref eth) = self.ethernet {
            if eth.connected {
                return NetworkKind::Ethernet;
            }
        }
        if self.wifi_enabled && !self.networks.is_empty() {
            return NetworkKind::Wifi;
        }
        NetworkKind::Disconnected
    }

    pub fn has_ethernet(&self) -> bool {
        self.ethernet.as_ref().map_or(false, |e| e.connected)
    }

    pub fn total_height(&self) -> i32 {
        let mut h = 12; // top padding
        if self.has_ethernet() {
            h += 46 + 9; // ethernet row + separator
        }
        h += 38; // wifi title & switch row
        if self.wifi_enabled {
            let count = self.networks.len().min(8) as i32;
            if count > 0 {
                h += 24 + count * 28; // section title + rows
            }
        }
        h += 9 + 34; // separator + settings link
        h + 8 // bottom padding
    }

    pub fn handle_click(&self, wx: i32, wy: i32) -> WifiMenuClick {
        if wx < 0 || wx >= WIFI_MENU_W {
            return WifiMenuClick::Close;
        }
        let mut cur_y = 12;
        if self.has_ethernet() {
            if wy >= cur_y && wy < cur_y + 46 {
                return WifiMenuClick::OpenSettings;
            }
            cur_y += 46 + 9;
        }

        // Wi-Fi header row (switch)
        if wy >= cur_y && wy < cur_y + 38 {
            if wx >= WIFI_MENU_W - 54 && wx <= WIFI_MENU_W - 10 {
                return WifiMenuClick::ToggleWifi;
            }
        }
        cur_y += 38;

        if self.wifi_enabled {
            let count = self.networks.len().min(8) as i32;
            if count > 0 {
                cur_y += 24; // skip section title
                if wy >= cur_y && wy < cur_y + count * 28 {
                    let idx = ((wy - cur_y) / 28) as usize;
                    if let Some(net) = self.networks.get(idx) {
                        return WifiMenuClick::ConnectWifi(net.ssid.clone());
                    }
                }
                cur_y += count * 28;
            }
        }

        // Settings link
        cur_y += 9;
        if wy >= cur_y && wy < cur_y + 34 {
            return WifiMenuClick::OpenSettings;
        }

        WifiMenuClick::None
    }

    pub fn toggle(&mut self) {
        self.open = !self.open;
        if self.open {
            self.refresh();
        }
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn connect(&mut self, ssid: &str) {
        let _ = Command::new("nmcli")
            .args(["dev", "wifi", "connect", ssid])
            .spawn();
        self.current_ssid = Some(ssid.to_string());
    }

    pub fn toggle_wifi(&mut self) {
        self.wifi_enabled = !self.wifi_enabled;
        let arg = if self.wifi_enabled { "on" } else { "off" };
        let _ = Command::new("nmcli")
            .args(["radio", "wifi", arg])
            .spawn();
    }
}

pub const WIFI_MENU_W: i32 = 280;

pub fn render_wifi_menu(
    menu: &WifiMenuState,
    theme: &Theme,
    text: &TextRenderer,
    pointer_pos: Option<(f64, f64)>,
    menu_pos: Point<i32, Logical>,
) -> Option<MemoryRenderBuffer> {
    if !menu.open {
        return None;
    }

    let total_h = menu.total_height();
    let mut pm = Pixmap::new(WIFI_MENU_W as u32, total_h as u32).expect("wifi menu pixmap");
    let dark = theme.dark();

    let bg = if dark {
        [28u8, 28, 36, 238]
    } else {
        [248u8, 248, 252, 240]
    };
    let border = if dark {
        [255u8, 255, 255, 36]
    } else {
        [0u8, 0, 0, 25]
    };

    // Card glass plate
    rounded_rect(&mut pm, 0.5, 0.5, WIFI_MENU_W as f32 - 1.0, total_h as f32 - 1.0, 14.0, bg);
    stroke_rounded(&mut pm, 0.5, 0.5, WIFI_MENU_W as f32 - 1.0, total_h as f32 - 1.0, 14.0, border, 1.0);

    let mut cur_y = 10.0f32;

    // 1. Ethernet Section (if connected)
    if let Some(ref eth) = menu.ethernet {
        if eth.connected {
            let is_hover = pointer_pos.map_or(false, |(px, py)| {
                let rx = px as i32 - menu_pos.x;
                let ry = py as i32 - menu_pos.y;
                rx >= 8 && rx < WIFI_MENU_W - 8 && ry >= cur_y as i32 && ry < (cur_y + 44.0) as i32
            });

            if is_hover {
                rounded_rect(&mut pm, 8.0, cur_y, WIFI_MENU_W as f32 - 16.0, 44.0, 8.0, if dark { [255, 255, 255, 22] } else { [0, 0, 0, 16] });
            }

            // Mini Ethernet Icon
            draw_mini_ethernet(&mut pm, 26.0, cur_y + 20.0, theme.menubar_fg());

            // Ethernet title & connection name
            text.draw_bold(&mut pm, "Ethernet", 44.0, cur_y + 17.0, 13.0, theme.menubar_fg());
            let conn_label = format!("{} ({})", eth.connection, eth.device);
            let trunc_conn: String = conn_label.chars().take(24).collect();
            text.draw(&mut pm, &trunc_conn, 44.0, cur_y + 33.0, 11.0, theme.menubar_fg_dim());

            // Green connected dot on right
            fill_circle(&mut pm, WIFI_MENU_W as f32 - 24.0, cur_y + 20.0, 3.5, [52, 199, 89, 255]);

            cur_y += 46.0;

            // Separator
            fill_rect(&mut pm, 12.0, cur_y + 4.0, WIFI_MENU_W as f32 - 24.0, 1.0, if dark { [255, 255, 255, 20] } else { [0, 0, 0, 25] });
            cur_y += 9.0;
        }
    }

    // 2. Wi-Fi Header: title & toggle switch
    draw_mini_wifi(&mut pm, 26.0, cur_y + 17.0, theme.menubar_fg());
    text.draw_bold(&mut pm, "Wi-Fi", 44.0, cur_y + 22.0, 13.5, theme.menubar_fg());

    let sw_w = 34.0;
    let sw_h = 20.0;
    let sw_x = WIFI_MENU_W as f32 - 46.0;
    let sw_y = cur_y + 8.0;
    let sw_bg = if menu.wifi_enabled { [0, 122, 255, 255] } else { if dark { [60, 60, 70, 255] } else { [210, 210, 220, 255] } };
    rounded_rect(&mut pm, sw_x, sw_y, sw_w, sw_h, 10.0, sw_bg);
    let knob_x = if menu.wifi_enabled { sw_x + 24.0 } else { sw_x + 10.0 };
    fill_circle(&mut pm, knob_x, sw_y + 10.0, 8.0, [255, 255, 255, 255]);

    cur_y += 38.0;

    // 3. Wi-Fi Networks List
    if menu.wifi_enabled {
        let net_count = menu.networks.len().min(8);
        if net_count > 0 {
            // Section title
            text.draw_bold(&mut pm, "Известные сети", 16.0, cur_y + 16.0, 11.0, theme.menubar_fg_dim());
            cur_y += 24.0;

            for net in menu.networks.iter().take(8) {
                let is_hover = pointer_pos.map_or(false, |(px, py)| {
                    let rx = px as i32 - menu_pos.x;
                    let ry = py as i32 - menu_pos.y;
                    rx >= 8 && rx < WIFI_MENU_W - 8 && ry >= cur_y as i32 && ry < (cur_y + 26.0) as i32
                });

                if is_hover {
                    rounded_rect(&mut pm, 8.0, cur_y, WIFI_MENU_W as f32 - 16.0, 26.0, 6.0, [0, 122, 255, 255]);
                }

                // Checkmark for connected Wi-Fi
                if net.active {
                    let col = if is_hover { [255, 255, 255, 255] } else { [0, 122, 255, 255] };
                    let mut pb = PathBuilder::new();
                    pb.move_to(16.0, cur_y + 13.0);
                    pb.line_to(19.0, cur_y + 17.0);
                    pb.line_to(25.0, cur_y + 9.0);
                    if let Some(p) = pb.finish() {
                        let mut paint = Paint::default();
                        paint.set_color_rgba8(col[0], col[1], col[2], col[3]);
                        let stroke = Stroke { width: 1.8, line_cap: tiny_skia::LineCap::Round, ..Default::default() };
                        pm.stroke_path(&p, &paint, &stroke, SkTransform::identity(), None);
                    }
                }

                let label_col = if is_hover { [255, 255, 255, 255] } else { theme.menubar_fg() };
                let ssid_trunc: String = net.ssid.chars().take(22).collect();
                text.draw(&mut pm, &ssid_trunc, 32.0, cur_y + 17.5, 12.5, label_col);

                // Wi-Fi signal icon on right
                let sig_x = WIFI_MENU_W as f32 - 24.0;
                let sig_col = if is_hover { [255, 255, 255, 255] } else { theme.menubar_fg_dim() };
                draw_mini_wifi(&mut pm, sig_x, cur_y + 13.0, sig_col);

                cur_y += 28.0;
            }
        }
    }

    // 4. Bottom link: Настройки сети...
    fill_rect(&mut pm, 12.0, cur_y + 4.0, WIFI_MENU_W as f32 - 24.0, 1.0, if dark { [255, 255, 255, 20] } else { [0, 0, 0, 25] });
    let is_bot_hover = pointer_pos.map_or(false, |(px, py)| {
        let rx = px as i32 - menu_pos.x;
        let ry = py as i32 - menu_pos.y;
        rx >= 8 && rx < WIFI_MENU_W - 8 && ry >= (cur_y + 8.0) as i32 && ry < (cur_y + 32.0) as i32
    });
    if is_bot_hover {
        rounded_rect(&mut pm, 8.0, cur_y + 8.0, WIFI_MENU_W as f32 - 16.0, 24.0, 6.0, [0, 122, 255, 255]);
        text.draw(&mut pm, "Настройки сети...", 16.0, cur_y + 24.0, 12.0, [255, 255, 255, 255]);
    } else {
        text.draw(&mut pm, "Настройки сети...", 16.0, cur_y + 24.0, 12.0, theme.menubar_fg());
    }

    let buf = MemoryRenderBuffer::from_slice(
        pm.data(),
        Fourcc::Abgr8888,
        (WIFI_MENU_W, total_h),
        1,
        Transform::Normal,
        None,
    );
    Some(buf)
}

fn draw_mini_ethernet(pm: &mut Pixmap, cx: f32, cy: f32, color: [u8; 4]) {
    let mut paint = Paint::default();
    paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
    let stroke = Stroke {
        width: 1.4,
        line_cap: tiny_skia::LineCap::Round,
        line_join: tiny_skia::LineJoin::Round,
        ..Default::default()
    };

    let mut pb_l = PathBuilder::new();
    pb_l.move_to(cx - 4.5, cy - 3.0);
    pb_l.line_to(cx - 7.2, cy);
    pb_l.line_to(cx - 4.5, cy + 3.0);
    if let Some(p) = pb_l.finish() {
        pm.stroke_path(&p, &paint, &stroke, SkTransform::identity(), None);
    }

    fill_circle(pm, cx - 2.2, cy, 0.9, color);
    fill_circle(pm, cx, cy, 0.9, color);
    fill_circle(pm, cx + 2.2, cy, 0.9, color);

    let mut pb_r = PathBuilder::new();
    pb_r.move_to(cx + 4.5, cy - 3.0);
    pb_r.line_to(cx + 7.2, cy);
    pb_r.line_to(cx + 4.5, cy + 3.0);
    if let Some(p) = pb_r.finish() {
        pm.stroke_path(&p, &paint, &stroke, SkTransform::identity(), None);
    }
}

fn draw_mini_wifi(pm: &mut Pixmap, cx: f32, cy: f32, color: [u8; 4]) {
    let dot_y = cy + 2.5;
    fill_circle(pm, cx, dot_y, 1.2, color);
    for r in [3.8f32, 7.0] {
        let mut pb = PathBuilder::new();
        let steps = 12;
        let start_rad = (-132.0f32).to_radians();
        let end_rad = (-48.0f32).to_radians();
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let rad = start_rad + (end_rad - start_rad) * t;
            let x = cx + r * rad.cos();
            let y = dot_y + r * rad.sin();
            if i == 0 {
                pb.move_to(x, y);
            } else {
                pb.line_to(x, y);
            }
        }
        if let Some(p) = pb.finish() {
            let mut paint = Paint::default();
            paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
            let stroke = Stroke {
                width: 1.3,
                line_cap: tiny_skia::LineCap::Round,
                ..Default::default()
            };
            pm.stroke_path(&p, &paint, &stroke, SkTransform::identity(), None);
        }
    }
}

fn rounded_rect(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, r: f32, color: [u8; 4]) {
    let r = r.min(w / 2.0).min(h / 2.0);
    let k = 0.552_284_8 * r;
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.cubic_to(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
        pm.fill_path(&path, &paint, FillRule::Winding, SkTransform::identity(), None);
    }
}

fn stroke_rounded(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, r: f32, color: [u8; 4], width: f32) {
    let r = r.min(w / 2.0).min(h / 2.0);
    let k = 0.552_284_8 * r;
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.cubic_to(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
        let stroke = Stroke {
            width,
            ..Default::default()
        };
        pm.stroke_path(&path, &paint, &stroke, SkTransform::identity(), None);
    }
}

fn fill_circle(pm: &mut Pixmap, cx: f32, cy: f32, r: f32, color: [u8; 4]) {
    let mut pb = PathBuilder::new();
    pb.push_circle(cx, cy, r);
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
        pm.fill_path(&path, &paint, FillRule::Winding, SkTransform::identity(), None);
    }
}

fn fill_rect(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, color: [u8; 4]) {
    let mut paint = Paint::default();
    paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
    if let Some(rect) = tiny_skia::Rect::from_xywh(x, y, w, h) {
        pm.fill_rect(rect, &paint, SkTransform::identity(), None);
    }
}
