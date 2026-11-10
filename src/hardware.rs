//! System hardware integration: audio volume and display backlight.
//!
//! Provides non-blocking synchronization with system audio (via PipeWire / PulseAudio / ALSA)
//! and display brightness (via systemd-logind or direct sysfs).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

static BACKLIGHT_DEVICE: std::sync::OnceLock<Option<BacklightDevice>> = std::sync::OnceLock::new();

#[derive(Debug, Clone)]
struct BacklightDevice {
    name: String,
    subsystem: String,
    max_brightness: u32,
    path: PathBuf,
}

fn detect_backlight() -> Option<BacklightDevice> {
    let base = Path::new("/sys/class/backlight");
    if let Ok(entries) = fs::read_dir(base) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let max_path = path.join("max_brightness");
            if let Ok(max_str) = fs::read_to_string(&max_path) {
                if let Ok(max) = max_str.trim().parse::<u32>() {
                    if max > 0 {
                        return Some(BacklightDevice {
                            name,
                            subsystem: "backlight".to_string(),
                            max_brightness: max,
                            path,
                        });
                    }
                }
            }
        }
    }
    None
}

fn get_backlight_device() -> Option<&'static BacklightDevice> {
    BACKLIGHT_DEVICE.get_or_init(detect_backlight).as_ref()
}

// Cached last sent brightness to avoid redundant process spawning during drag
static LAST_SENT_BRIGHTNESS: AtomicU32 = AtomicU32::new(u32::MAX);
static LAST_SENT_VOLUME: AtomicU32 = AtomicU32::new(u32::MAX);

/// Read current hardware brightness as a 0.0 .. 1.0 fraction.
pub fn read_hardware_brightness() -> Option<f32> {
    let dev = get_backlight_device()?;
    let cur_path = dev.path.join("actual_brightness");
    let content = fs::read_to_string(&cur_path).or_else(|_| fs::read_to_string(dev.path.join("brightness"))).ok()?;
    let cur = content.trim().parse::<u32>().ok()?;
    Some((cur as f32 / dev.max_brightness as f32).clamp(0.05, 1.0))
}

/// Read current system audio volume as a 0.0 .. 1.0 fraction.
pub fn read_hardware_volume() -> Option<f32> {
    // 1. Try wpctl get-volume @DEFAULT_AUDIO_SINK@
    if let Ok(output) = Command::new("wpctl").args(["get-volume", "@DEFAULT_AUDIO_SINK@"]).output() {
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout);
            // Example: "Volume: 0.45" or "Volume: 0.45 [MUTED]"
            for part in s.split_whitespace() {
                if let Ok(val) = part.parse::<f32>() {
                    return Some(val.clamp(0.0, 1.0));
                }
            }
        }
    }

    // 2. Try pactl get-sink-volume @DEFAULT_SINK@
    if let Ok(output) = Command::new("pactl").args(["get-sink-volume", "@DEFAULT_SINK@"]).output() {
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout);
            for part in s.split_whitespace() {
                if part.ends_with('%') {
                    if let Ok(pct) = part.trim_end_matches('%').parse::<f32>() {
                        return Some((pct / 100.0).clamp(0.0, 1.0));
                    }
                }
            }
        }
    }

    None
}

/// Apply brightness (0.05 .. 1.0) to hardware display.
/// Runs in background thread so the UI never blocks.
pub fn apply_hardware_brightness(val: f32) {
    let clamped = val.clamp(0.05, 1.0);
    let dev = match get_backlight_device() {
        Some(d) => d.clone(),
        None => return,
    };

    let target_raw = ((dev.max_brightness as f32 * clamped).round() as u32).clamp(1, dev.max_brightness);
    let prev = LAST_SENT_BRIGHTNESS.swap(target_raw, Ordering::Relaxed);
    if prev == target_raw {
        return;
    }

    std::thread::spawn(move || {
        // First try writing directly to sysfs
        let brightness_path = dev.path.join("brightness");
        if fs::write(&brightness_path, target_raw.to_string()).is_ok() {
            return;
        }

        // If direct sysfs write fails (unprivileged user), use logind via busctl
        let _ = Command::new("busctl")
            .args([
                "--system",
                "call",
                "org.freedesktop.login1",
                "/org/freedesktop/login1/session/auto",
                "org.freedesktop.login1.Session",
                "SetBrightness",
                "ssu",
                &dev.subsystem,
                &dev.name,
                &target_raw.to_string(),
            ])
            .output();
    });
}

/// Apply volume (0.0 .. 1.0) to hardware audio.
/// Runs in background thread so the UI never blocks.
pub fn apply_hardware_volume(val: f32) {
    let clamped = val.clamp(0.0, 1.0);
    let pct = (clamped * 100.0).round() as u32;
    let prev = LAST_SENT_VOLUME.swap(pct, Ordering::Relaxed);
    if prev == pct {
        return;
    }

    let val_str = format!("{:.2}", clamped);
    let pct_str = format!("{}%", pct);

    std::thread::spawn(move || {
        // 1. Try wpctl set-volume
        if let Ok(status) = Command::new("wpctl")
            .args(["set-volume", "@DEFAULT_AUDIO_SINK@", &val_str])
            .status()
        {
            if status.success() {
                return;
            }
        }

        // 2. Try pactl set-sink-volume
        if let Ok(status) = Command::new("pactl")
            .args(["set-sink-volume", "@DEFAULT_SINK@", &pct_str])
            .status()
        {
            if status.success() {
                return;
            }
        }

        // 3. Fallback to ALSA amixer
        let _ = Command::new("amixer")
            .args(["sset", "Master", &pct_str])
            .status();
    });
}
