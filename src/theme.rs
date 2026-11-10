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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WallpaperTheme {
    Sequoia,
    Sonoma,
    Monterey,
    Midnight,
}

impl WallpaperTheme {
    pub fn name(self) -> &'static str {
        match self {
            Self::Sequoia => "macOS Sequoia",
            Self::Sonoma => "Sonoma Sunset",
            Self::Monterey => "Monterey Blue",
            Self::Midnight => "Midnight Space",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Sequoia => "Фирменная аврора индиго и фиолета",
            Self::Sonoma => "Теплые лучи золотого заката",
            Self::Monterey => "Глубокий океанический сапфир",
            Self::Midnight => "Минималистичный глубокий космос",
        }
    }
}

impl Default for WallpaperTheme {
    fn default() -> Self {
        Self::Sequoia
    }
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub appearance: Appearance,
    pub wallpaper_theme: WallpaperTheme,
    pub night_shift: bool,
    /// System accent color (macOS blue by default).
    pub accent: [u8; 4],
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            appearance: Appearance::default(),
            wallpaper_theme: WallpaperTheme::default(),
            night_shift: false,
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
                } else if let Some(value) = line.strip_prefix("wallpaper") {
                    let value = value.trim_start_matches([' ', '=', '"', '\'']);
                    match value.trim_end_matches(['"', '\'']) {
                        "sequoia" => theme.wallpaper_theme = WallpaperTheme::Sequoia,
                        "sonoma" => theme.wallpaper_theme = WallpaperTheme::Sonoma,
                        "monterey" => theme.wallpaper_theme = WallpaperTheme::Monterey,
                        "midnight" => theme.wallpaper_theme = WallpaperTheme::Midnight,
                        _ => {}
                    }
                } else if let Some(value) = line.strip_prefix("night_shift") {
                    let value = value.trim_start_matches([' ', '=', '"', '\'']);
                    theme.night_shift = value.starts_with("true");
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
        let wp_str = match self.wallpaper_theme {
            WallpaperTheme::Sequoia => "sequoia",
            WallpaperTheme::Sonoma => "sonoma",
            WallpaperTheme::Monterey => "monterey",
            WallpaperTheme::Midnight => "midnight",
        };
        let contents = format!(
            "# Aqua compositor configuration\nappearance = \"{}\"\nwallpaper = \"{}\"\nnight_shift = {}\n",
            self.appearance.as_str(),
            wp_str,
            self.night_shift,
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
            // macOS Sequoia dark translucent glass
            [26, 26, 34, 160]
        } else {
            // macOS Sequoia light translucent frosted glass
            [255, 255, 255, 145]
        }
    }

    pub fn dock_border(&self) -> [u8; 4] {
        if self.dark() {
            [255, 255, 255, 38]
        } else {
            [0, 0, 0, 24]
        }
    }

    pub fn dock_top_rim(&self) -> [u8; 4] {
        if self.dark() {
            [255, 255, 255, 52]
        } else {
            [255, 255, 255, 140]
        }
    }

    pub fn dock_indicator(&self) -> [u8; 4] {
        if self.dark() {
            [255, 255, 255, 230]
        } else {
            [30, 30, 35, 210]
        }
    }

    pub fn dock_indicator_glow(&self) -> [u8; 4] {
        if self.dark() {
            [255, 255, 255, 60]
        } else {
            [0, 0, 0, 30]
        }
    }

    pub fn dock_separator(&self) -> ([u8; 4], [u8; 4]) {
        if self.dark() {
            ([0, 0, 0, 45], [255, 255, 255, 30])
        } else {
            ([0, 0, 0, 35], [255, 255, 255, 90])
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
            match self.wallpaper_theme {
                WallpaperTheme::Sequoia => [18, 14, 42, 255],
                WallpaperTheme::Sonoma => [34, 16, 12, 255],
                WallpaperTheme::Monterey => [12, 22, 52, 255],
                WallpaperTheme::Midnight => [14, 16, 24, 255],
            }
        } else {
            match self.wallpaper_theme {
                WallpaperTheme::Sequoia => [255, 214, 196, 255],
                WallpaperTheme::Sonoma => [255, 225, 190, 255],
                WallpaperTheme::Monterey => [200, 230, 255, 255],
                WallpaperTheme::Midnight => [242, 244, 250, 255],
            }
        }
    }

    pub fn wallpaper_bottom(&self) -> [u8; 4] {
        if self.dark() {
            match self.wallpaper_theme {
                WallpaperTheme::Sequoia => [6, 4, 14, 255],
                WallpaperTheme::Sonoma => [10, 6, 8, 255],
                WallpaperTheme::Monterey => [4, 8, 22, 255],
                WallpaperTheme::Midnight => [4, 4, 8, 255],
            }
        } else {
            match self.wallpaper_theme {
                WallpaperTheme::Sequoia => [168, 196, 236, 255],
                WallpaperTheme::Sonoma => [235, 165, 130, 255],
                WallpaperTheme::Monterey => [140, 185, 240, 255],
                WallpaperTheme::Midnight => [205, 210, 222, 255],
            }
        }
    }

    pub fn wallpaper_glow(&self) -> [u8; 4] {
        if self.dark() {
            match self.wallpaper_theme {
                WallpaperTheme::Sequoia => [68, 48, 148, 255],
                WallpaperTheme::Sonoma => [175, 75, 30, 255],
                WallpaperTheme::Monterey => [20, 115, 165, 255],
                WallpaperTheme::Midnight => [55, 68, 95, 255],
            }
        } else {
            match self.wallpaper_theme {
                WallpaperTheme::Sequoia => [255, 244, 214, 255],
                WallpaperTheme::Sonoma => [255, 240, 210, 255],
                WallpaperTheme::Monterey => [230, 245, 255, 255],
                WallpaperTheme::Midnight => [255, 255, 255, 255],
            }
        }
    }

    // Secondary glow color for multi-aurora effect
    pub fn wallpaper_glow2(&self) -> [u8; 4] {
        if self.dark() {
            match self.wallpaper_theme {
                WallpaperTheme::Sequoia => [38, 28, 100, 255],
                WallpaperTheme::Sonoma => [120, 45, 20, 255],
                WallpaperTheme::Monterey => [18, 65, 135, 255],
                WallpaperTheme::Midnight => [32, 42, 65, 255],
            }
        } else {
            match self.wallpaper_theme {
                WallpaperTheme::Sequoia => [200, 220, 255, 255],
                WallpaperTheme::Sonoma => [240, 180, 150, 255],
                WallpaperTheme::Monterey => [180, 210, 250, 255],
                WallpaperTheme::Midnight => [220, 225, 235, 255],
            }
        }
    }
}
