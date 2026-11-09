//! Aqua theme: light/dark appearance, colors and persistence.

use std::{fs, path::PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Appearance {
    Light,
    Dark,
}

impl Appearance {
    pub fn as_str(self) -> &'static str {
        match self {
            Appearance::Light => "light",
            Appearance::Dark => "dark",
        }
    }

    pub fn is_dark(self) -> bool {
        matches!(self, Appearance::Dark)
    }
}

impl Default for Appearance {
    fn default() -> Self {
        Appearance::Dark
    }
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub appearance: Appearance,
    /// System accent color (macOS blue by default).
    pub accent: [u8; 4],
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            appearance: Appearance::default(),
            accent: [0, 122, 255, 255],
        }
    }
}

impl Theme {
    pub fn config_path() -> PathBuf {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .unwrap_or_else(|| PathBuf::from("."));
        base.join("aqua").join("config.toml")
    }

    /// Load the theme from `~/.config/aqua/config.toml`. Missing file is fine.
    pub fn load() -> Self {
        let mut theme = Self::default();
        if let Ok(contents) = fs::read_to_string(Self::config_path()) {
            for line in contents.lines() {
                let line = line.trim();
                if let Some(value) = line.strip_prefix("appearance") {
                    let value = value.trim_start_matches([' ', '=', '"', '\'']);
                    match value.trim_end_matches(['"', '\'']) {
                        "light" => theme.appearance = Appearance::Light,
                        "dark" => theme.appearance = Appearance::Dark,
                        _ => {}
                    }
                }
            }
        }
        theme
    }

    pub fn save(&self) {
        let path = Self::config_path();
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let contents = format!(
            "# Aqua compositor configuration\nappearance = \"{}\"\n",
            self.appearance.as_str()
        );
        let _ = fs::write(path, contents);
    }

    pub fn toggle(&mut self) {
        self.appearance = match self.appearance {
            Appearance::Light => Appearance::Dark,
            Appearance::Dark => Appearance::Light,
        };
        self.save();
    }

    pub fn dark(&self) -> bool {
        self.appearance.is_dark()
    }

    // --- menu bar ---------------------------------------------------------

    pub fn menubar_bg(&self) -> [u8; 4] {
        if self.dark() {
            [22, 22, 26, 178]
        } else {
            [250, 250, 252, 178]
        }
    }

    pub fn menubar_fg(&self) -> [u8; 4] {
        if self.dark() {
            [242, 242, 247, 255]
        } else {
            [28, 28, 32, 255]
        }
    }

    pub fn menubar_fg_dim(&self) -> [u8; 4] {
        if self.dark() {
            [180, 180, 190, 255]
        } else {
            [90, 90, 100, 255]
        }
    }

    // --- dock -------------------------------------------------------------

    pub fn dock_bg(&self) -> [u8; 4] {
        if self.dark() {
            // Slightly more opaque dark glass, matches Sequoia
            [28, 28, 36, 170]
        } else {
            [255, 255, 255, 150]
        }
    }

    pub fn dock_border(&self) -> [u8; 4] {
        if self.dark() {
            [255, 255, 255, 45]
        } else {
            [0, 0, 0, 30]
        }
    }

    // --- windows ----------------------------------------------------------

    pub fn window_border(&self) -> [u8; 4] {
        if self.dark() {
            [255, 255, 255, 28]
        } else {
            [0, 0, 0, 32]
        }
    }

    pub fn window_shadow(&self) -> [u8; 4] {
        if self.dark() {
            [0, 0, 0, 90]
        } else {
            [0, 0, 0, 55]
        }
    }

    // --- wallpaper --------------------------------------------------------

    pub fn wallpaper_top(&self) -> [u8; 4] {
        if self.dark() {
            // Deep dark indigo — macOS Sequoia dark top
            [18, 14, 42, 255]
        } else {
            [255, 214, 196, 255]
        }
    }

    pub fn wallpaper_bottom(&self) -> [u8; 4] {
        if self.dark() {
            // Near-black at the bottom
            [6, 4, 14, 255]
        } else {
            [168, 196, 236, 255]
        }
    }

    pub fn wallpaper_glow(&self) -> [u8; 4] {
        if self.dark() {
            // Purple/violet aurora glow, Sequoia style
            [68, 48, 148, 255]
        } else {
            [255, 244, 214, 255]
        }
    }

    // Secondary glow color for multi-aurora effect
    pub fn wallpaper_glow2(&self) -> [u8; 4] {
        if self.dark() {
            // Deep blue-violet
            [38, 28, 100, 255]
        } else {
            [200, 220, 255, 255]
        }
    }
}
