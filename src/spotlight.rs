//! Spotlight search launcher and Menu bar dropdown menus for Aqua.

use smithay::backend::{
    allocator::Fourcc,
    renderer::element::memory::MemoryRenderBuffer,
};
use smithay::utils::{Logical, Point, Transform};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Transform as SkTransform};

use crate::apps::{scan_apps, AppInfo};
use crate::text::TextRenderer;
use crate::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppCategory {
    Finder,
    Editor,     // Cursor / VS Code
    Browser,    // Google Chrome / Chromium / Firefox
    Terminal,   // Konsole / Kitty
    Generic,
}

pub fn detect_app_category(app_id: Option<&str>, title: Option<&str>) -> (String, AppCategory) {
    let id = app_id.unwrap_or("").to_lowercase();
    let t = title.unwrap_or("").to_lowercase();

    if id.contains("cursor") || t.contains("cursor") || id.contains("code") || t.contains("visual studio code") {
        let name = if let Some(t_orig) = title {
            if t_orig.contains("Cursor") {
                t_orig.to_string()
            } else {
                "Cursor".to_string()
            }
        } else {
            "Cursor".to_string()
        };
        (name, AppCategory::Editor)
    } else if id.contains("chrome") || id.contains("chromium") || id.contains("brave") || id.contains("firefox") || t.contains("chrome") {
        ("Google Chrome".to_string(), AppCategory::Browser)
    } else if id.contains("konsole") || id.contains("terminal") || id.contains("kitty") || id.contains("alacritty") {
        ("Terminal".to_string(), AppCategory::Terminal)
    } else if id.contains("dolphin") || id.contains("nautilus") || id.contains("files") {
        ("Finder".to_string(), AppCategory::Finder)
    } else if let Some(t_orig) = title {
        (t_orig.to_string(), AppCategory::Generic)
    } else {
        ("Finder".to_string(), AppCategory::Finder)
    }
}

pub fn app_menu_titles(category: AppCategory) -> &'static [&'static str] {
    match category {
        AppCategory::Editor => &["File", "Edit", "Selection", "View", "Go", "Run", "Terminal", "Window", "Help"],
        AppCategory::Browser => &["File", "Edit", "View", "History", "Bookmarks", "Profiles", "Window", "Help"],
        AppCategory::Terminal => &["Shell", "Edit", "View", "Window", "Help"],
        AppCategory::Finder => &["Файл", "Правка", "Вид", "Переход", "Окно", "Справка"],
        AppCategory::Generic => &["File", "Edit", "View", "Window", "Help"],
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuKind {
    Apple,
    App {
        category: AppCategory,
        title: &'static str,
    },
}

#[derive(Debug, Clone)]
pub struct MenuItem {
    pub label: String,
    pub shortcut: Option<String>,
    pub action: MenuAction,
    pub is_separator: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuAction {
    None,
    Spawn(&'static str),
    ToggleTheme,
    ToggleSettings,
    CloseWindow,
    MinimizeWindow,
    ToggleMaximize,
    About,
    Restart,
    Shutdown,
}

pub struct MenuState {
    pub active_menu: Option<(MenuKind, Point<i32, Logical>)>,
    pub hovered_item: Option<usize>,
    pub items: Vec<MenuItem>,
}

impl Default for MenuState {
    fn default() -> Self {
        Self {
            active_menu: None,
            hovered_item: None,
            items: Vec::new(),
        }
    }
}

impl MenuState {
    pub fn open(&mut self, kind: MenuKind, pos: Point<i32, Logical>) {
        self.active_menu = Some((kind.clone(), pos));
        self.hovered_item = None;
        self.items = match kind {
            MenuKind::Apple => vec![
                MenuItem { label: "Об этом компьютере".into(), shortcut: None, action: MenuAction::About, is_separator: false },
                MenuItem { label: "Системные настройки...".into(), shortcut: Some("⌘,".into()), action: MenuAction::ToggleSettings, is_separator: false },
                MenuItem { label: "".into(), shortcut: None, action: MenuAction::None, is_separator: true },
                MenuItem { label: "Переключить тему".into(), shortcut: Some("⇧⌘D".into()), action: MenuAction::ToggleTheme, is_separator: false },
                MenuItem { label: "Терминал".into(), shortcut: Some("⌘↵".into()), action: MenuAction::Spawn("konsole"), is_separator: false },
                MenuItem { label: "Файловый менеджер".into(), shortcut: None, action: MenuAction::Spawn("dolphin"), is_separator: false },
                MenuItem { label: "".into(), shortcut: None, action: MenuAction::None, is_separator: true },
                MenuItem { label: "Перезагрузка...".into(), shortcut: None, action: MenuAction::Restart, is_separator: false },
                MenuItem { label: "Выключить...".into(), shortcut: None, action: MenuAction::Shutdown, is_separator: false },
            ],
            MenuKind::App { category, title } => {
                match category {
                    AppCategory::Editor => match title {
                        "File" => vec![
                            MenuItem { label: "New Text File".into(), shortcut: Some("⌘N".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "New Window".into(), shortcut: Some("⇧⌘N".into()), action: MenuAction::Spawn("cursor"), is_separator: false },
                            MenuItem { label: "Open File...".into(), shortcut: Some("⌘O".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Open Folder...".into(), shortcut: Some("⌘K ⌘O".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "".into(), shortcut: None, action: MenuAction::None, is_separator: true },
                            MenuItem { label: "Save".into(), shortcut: Some("⌘S".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Save As...".into(), shortcut: Some("⇧⌘S".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Save All".into(), shortcut: Some("⌥⌘S".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "".into(), shortcut: None, action: MenuAction::None, is_separator: true },
                            MenuItem { label: "Close Editor".into(), shortcut: Some("⌘W".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Close Window".into(), shortcut: Some("⇧⌘W".into()), action: MenuAction::CloseWindow, is_separator: false },
                        ],
                        "Edit" => vec![
                            MenuItem { label: "Undo".into(), shortcut: Some("⌘Z".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Redo".into(), shortcut: Some("⇧⌘Z".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "".into(), shortcut: None, action: MenuAction::None, is_separator: true },
                            MenuItem { label: "Cut".into(), shortcut: Some("⌘X".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Copy".into(), shortcut: Some("⌘C".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Paste".into(), shortcut: Some("⌘V".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "".into(), shortcut: None, action: MenuAction::None, is_separator: true },
                            MenuItem { label: "Find".into(), shortcut: Some("⌘F".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Replace".into(), shortcut: Some("⌥⌘F".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Find in Files".into(), shortcut: Some("⇧⌘F".into()), action: MenuAction::None, is_separator: false },
                        ],
                        "Selection" => vec![
                            MenuItem { label: "Select All".into(), shortcut: Some("⌘A".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Expand Selection".into(), shortcut: Some("⌃⇧⌘→".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Shrink Selection".into(), shortcut: Some("⌃⇧⌘←".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "".into(), shortcut: None, action: MenuAction::None, is_separator: true },
                            MenuItem { label: "Copy Line Down".into(), shortcut: Some("⌥⇧↓".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Move Line Down".into(), shortcut: Some("⌥↓".into()), action: MenuAction::None, is_separator: false },
                        ],
                        "View" => vec![
                            MenuItem { label: "Command Palette...".into(), shortcut: Some("⇧⌘P".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Open View...".into(), shortcut: Some("⌘Q".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "".into(), shortcut: None, action: MenuAction::None, is_separator: true },
                            MenuItem { label: "Explorer".into(), shortcut: Some("⇧⌘E".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Search".into(), shortcut: Some("⇧⌘F".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Source Control".into(), shortcut: Some("⌃⇧G".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Run and Debug".into(), shortcut: Some("⇧⌘D".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Extensions".into(), shortcut: Some("⇧⌘X".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "".into(), shortcut: None, action: MenuAction::None, is_separator: true },
                            MenuItem { label: "Terminal".into(), shortcut: Some("⌃`".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Problems".into(), shortcut: Some("⇧⌘M".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "".into(), shortcut: None, action: MenuAction::None, is_separator: true },
                            MenuItem { label: "Toggle Full Screen".into(), shortcut: Some("⌃⌘F".into()), action: MenuAction::ToggleMaximize, is_separator: false },
                        ],
                        "Go" => vec![
                            MenuItem { label: "Back".into(), shortcut: Some("⌃-".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Forward".into(), shortcut: Some("⌃⇧-".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "".into(), shortcut: None, action: MenuAction::None, is_separator: true },
                            MenuItem { label: "Go to File...".into(), shortcut: Some("⌘P".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Go to Symbol...".into(), shortcut: Some("⌘T".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Go to Line...".into(), shortcut: Some("⌃G".into()), action: MenuAction::None, is_separator: false },
                        ],
                        "Run" => vec![
                            MenuItem { label: "Start Debugging".into(), shortcut: Some("F5".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Run Without Debugging".into(), shortcut: Some("⌃F5".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Stop Debugging".into(), shortcut: Some("⇧F5".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Restart Debugging".into(), shortcut: Some("⇧⌘F5".into()), action: MenuAction::None, is_separator: false },
                        ],
                        "Terminal" => vec![
                            MenuItem { label: "New Terminal".into(), shortcut: Some("⌃⇧`".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Split Terminal".into(), shortcut: Some("⌘\\".into()), action: MenuAction::None, is_separator: false },
                        ],
                        "Window" => vec![
                            MenuItem { label: "Minimize".into(), shortcut: Some("⌘M".into()), action: MenuAction::MinimizeWindow, is_separator: false },
                            MenuItem { label: "Zoom".into(), shortcut: None, action: MenuAction::ToggleMaximize, is_separator: false },
                            MenuItem { label: "".into(), shortcut: None, action: MenuAction::None, is_separator: true },
                            MenuItem { label: "Close Window".into(), shortcut: Some("⌘W".into()), action: MenuAction::CloseWindow, is_separator: false },
                        ],
                        "Help" | _ => vec![
                            MenuItem { label: "Welcome".into(), shortcut: None, action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Documentation".into(), shortcut: None, action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Keyboard Shortcuts Reference".into(), shortcut: Some("⌘K ⌘S".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "".into(), shortcut: None, action: MenuAction::None, is_separator: true },
                            MenuItem { label: "About Cursor".into(), shortcut: None, action: MenuAction::About, is_separator: false },
                        ],
                    },
                    AppCategory::Browser => match title {
                        "File" => vec![
                            MenuItem { label: "New Tab".into(), shortcut: Some("⌘T".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "New Window".into(), shortcut: Some("⌘N".into()), action: MenuAction::Spawn("google-chrome-stable"), is_separator: false },
                            MenuItem { label: "New Incognito Window".into(), shortcut: Some("⇧⌘N".into()), action: MenuAction::Spawn("google-chrome-stable --incognito"), is_separator: false },
                            MenuItem { label: "Open File...".into(), shortcut: Some("⌘O".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Open Location...".into(), shortcut: Some("⌘L".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "".into(), shortcut: None, action: MenuAction::None, is_separator: true },
                            MenuItem { label: "Close Tab".into(), shortcut: Some("⌘W".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Close Window".into(), shortcut: Some("⇧⌘W".into()), action: MenuAction::CloseWindow, is_separator: false },
                            MenuItem { label: "Save Page As...".into(), shortcut: Some("⌘S".into()), action: MenuAction::None, is_separator: false },
                        ],
                        "Edit" => vec![
                            MenuItem { label: "Undo".into(), shortcut: Some("⌘Z".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Redo".into(), shortcut: Some("⇧⌘Z".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "".into(), shortcut: None, action: MenuAction::None, is_separator: true },
                            MenuItem { label: "Cut".into(), shortcut: Some("⌘X".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Copy".into(), shortcut: Some("⌘C".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Paste".into(), shortcut: Some("⌘V".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Select All".into(), shortcut: Some("⌘A".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "".into(), shortcut: None, action: MenuAction::None, is_separator: true },
                            MenuItem { label: "Find...".into(), shortcut: Some("⌘F".into()), action: MenuAction::None, is_separator: false },
                        ],
                        "View" => vec![
                            MenuItem { label: "Reload This Page".into(), shortcut: Some("⌘R".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Force Reload".into(), shortcut: Some("⇧⌘R".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Actual Size".into(), shortcut: Some("⌘0".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Zoom In".into(), shortcut: Some("⌘+".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Zoom Out".into(), shortcut: Some("⌘-".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "".into(), shortcut: None, action: MenuAction::None, is_separator: true },
                            MenuItem { label: "Full Screen".into(), shortcut: Some("⌃⌘F".into()), action: MenuAction::ToggleMaximize, is_separator: false },
                            MenuItem { label: "Developer Tools".into(), shortcut: Some("⌥⌘I".into()), action: MenuAction::None, is_separator: false },
                        ],
                        "History" => vec![
                            MenuItem { label: "Back".into(), shortcut: Some("⌘[".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Forward".into(), shortcut: Some("⌘]".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Home".into(), shortcut: Some("⇧⌘H".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "".into(), shortcut: None, action: MenuAction::None, is_separator: true },
                            MenuItem { label: "Show Full History".into(), shortcut: Some("⌘Y".into()), action: MenuAction::None, is_separator: false },
                        ],
                        "Bookmarks" => vec![
                            MenuItem { label: "Bookmark This Tab".into(), shortcut: Some("⌘D".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Bookmark All Tabs...".into(), shortcut: Some("⇧⌘D".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Bookmark Manager".into(), shortcut: Some("⌥⌘B".into()), action: MenuAction::None, is_separator: false },
                        ],
                        "Profiles" => vec![
                            MenuItem { label: "Default".into(), shortcut: None, action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Guest".into(), shortcut: None, action: MenuAction::None, is_separator: false },
                        ],
                        "Window" => vec![
                            MenuItem { label: "Minimize".into(), shortcut: Some("⌘M".into()), action: MenuAction::MinimizeWindow, is_separator: false },
                            MenuItem { label: "Zoom".into(), shortcut: None, action: MenuAction::ToggleMaximize, is_separator: false },
                            MenuItem { label: "Downloads".into(), shortcut: Some("⌥⌘L".into()), action: MenuAction::Spawn("dolphin"), is_separator: false },
                            MenuItem { label: "".into(), shortcut: None, action: MenuAction::None, is_separator: true },
                            MenuItem { label: "Close Window".into(), shortcut: Some("⌘W".into()), action: MenuAction::CloseWindow, is_separator: false },
                        ],
                        "Help" | _ => vec![
                            MenuItem { label: "Chrome Help".into(), shortcut: None, action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Report an Issue...".into(), shortcut: None, action: MenuAction::None, is_separator: false },
                            MenuItem { label: "".into(), shortcut: None, action: MenuAction::None, is_separator: true },
                            MenuItem { label: "About Google Chrome".into(), shortcut: None, action: MenuAction::About, is_separator: false },
                        ],
                    },
                    AppCategory::Terminal => match title {
                        "Shell" => vec![
                            MenuItem { label: "New Window".into(), shortcut: Some("⌘N".into()), action: MenuAction::Spawn("konsole"), is_separator: false },
                            MenuItem { label: "New Tab".into(), shortcut: Some("⌘T".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "".into(), shortcut: None, action: MenuAction::None, is_separator: true },
                            MenuItem { label: "Close Window".into(), shortcut: Some("⇧⌘W".into()), action: MenuAction::CloseWindow, is_separator: false },
                        ],
                        "Edit" => vec![
                            MenuItem { label: "Copy".into(), shortcut: Some("⌘C".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Paste".into(), shortcut: Some("⌘V".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Select All".into(), shortcut: Some("⌘A".into()), action: MenuAction::None, is_separator: false },
                        ],
                        "View" => vec![
                            MenuItem { label: "Zoom In".into(), shortcut: Some("⌘+".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Zoom Out".into(), shortcut: Some("⌘-".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Full Screen".into(), shortcut: Some("⌃⌘F".into()), action: MenuAction::ToggleMaximize, is_separator: false },
                        ],
                        "Window" => vec![
                            MenuItem { label: "Minimize".into(), shortcut: Some("⌘M".into()), action: MenuAction::MinimizeWindow, is_separator: false },
                            MenuItem { label: "Zoom".into(), shortcut: None, action: MenuAction::ToggleMaximize, is_separator: false },
                            MenuItem { label: "Close Window".into(), shortcut: Some("⌘W".into()), action: MenuAction::CloseWindow, is_separator: false },
                        ],
                        _ => vec![
                            MenuItem { label: "Terminal Help".into(), shortcut: None, action: MenuAction::About, is_separator: false },
                        ],
                    },
                    AppCategory::Finder | AppCategory::Generic => match title {
                        "File" | "Файл" => vec![
                            MenuItem { label: "Новое окно".into(), shortcut: Some("⌘N".into()), action: MenuAction::Spawn("dolphin"), is_separator: false },
                            MenuItem { label: "Новая папка".into(), shortcut: Some("⇧⌘N".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "".into(), shortcut: None, action: MenuAction::None, is_separator: true },
                            MenuItem { label: "Закрыть окно".into(), shortcut: Some("⌘W".into()), action: MenuAction::CloseWindow, is_separator: false },
                        ],
                        "Edit" | "Правка" => vec![
                            MenuItem { label: "Отменить".into(), shortcut: Some("⌘Z".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Повторить".into(), shortcut: Some("⇧⌘Z".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "".into(), shortcut: None, action: MenuAction::None, is_separator: true },
                            MenuItem { label: "Вырезать".into(), shortcut: Some("⌘X".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Копировать".into(), shortcut: Some("⌘C".into()), action: MenuAction::None, is_separator: false },
                            MenuItem { label: "Вставить".into(), shortcut: Some("⌘V".into()), action: MenuAction::None, is_separator: false },
                        ],
                        "View" | "Вид" => vec![
                            MenuItem { label: "Во весь экран".into(), shortcut: Some("⌃⌘F".into()), action: MenuAction::ToggleMaximize, is_separator: false },
                            MenuItem { label: "Переключить тему".into(), shortcut: Some("⇧⌘D".into()), action: MenuAction::ToggleTheme, is_separator: false },
                        ],
                        "Go" | "Переход" => vec![
                            MenuItem { label: "Домашняя папка".into(), shortcut: Some("⇧⌘H".into()), action: MenuAction::Spawn("dolphin"), is_separator: false },
                            MenuItem { label: "Загрузки".into(), shortcut: Some("⌥⌘L".into()), action: MenuAction::Spawn("dolphin"), is_separator: false },
                            MenuItem { label: "Документы".into(), shortcut: Some("⇧⌘O".into()), action: MenuAction::Spawn("dolphin"), is_separator: false },
                            MenuItem { label: "Компьютер".into(), shortcut: Some("⇧⌘C".into()), action: MenuAction::Spawn("dolphin"), is_separator: false },
                        ],
                        "Window" | "Окно" => vec![
                            MenuItem { label: "Свернуть".into(), shortcut: Some("⌘M".into()), action: MenuAction::MinimizeWindow, is_separator: false },
                            MenuItem { label: "Изменить масштаб".into(), shortcut: None, action: MenuAction::ToggleMaximize, is_separator: false },
                        ],
                        "Help" | "Справка" | _ => vec![
                            MenuItem { label: "Справка Aqua".into(), shortcut: None, action: MenuAction::About, is_separator: false },
                        ],
                    },
                }
            }
        };
    }

    pub fn close(&mut self) {
        self.active_menu = None;
        self.hovered_item = None;
        self.items.clear();
    }
}

pub struct SpotlightState {
    pub open: bool,
    pub query: String,
    pub selected_index: usize,
    pub all_apps: Vec<AppInfo>,
    pub filtered_apps: Vec<AppInfo>,
}

impl Default for SpotlightState {
    fn default() -> Self {
        let all_apps = scan_apps();
        let filtered_apps = all_apps.iter().take(6).cloned().collect();
        Self {
            open: false,
            query: String::new(),
            selected_index: 0,
            all_apps,
            filtered_apps,
        }
    }
}

impl SpotlightState {
    pub fn toggle(&mut self) {
        self.open = !self.open;
        if self.open {
            self.query.clear();
            self.selected_index = 0;
            self.filter();
        }
    }

    pub fn close(&mut self) {
        self.open = false;
        self.query.clear();
        self.selected_index = 0;
    }

    pub fn filter(&mut self) {
        let q = self.query.trim().to_lowercase();
        if q.is_empty() {
            self.filtered_apps = self.all_apps.iter().take(6).cloned().collect();
        } else {
            self.filtered_apps = self
                .all_apps
                .iter()
                .filter(|app| {
                    app.name.to_lowercase().contains(&q)
                        || app.comment.as_deref().unwrap_or("").to_lowercase().contains(&q)
                        || app.exec.to_lowercase().contains(&q)
                })
                .take(6)
                .cloned()
                .collect();
        }
        if self.selected_index >= self.filtered_apps.len() {
            self.selected_index = 0;
        }
    }

    pub fn input_char(&mut self, c: char) {
        if !c.is_control() {
            self.query.push(c);
            self.filter();
        }
    }

    pub fn backspace(&mut self) {
        self.query.pop();
        self.filter();
    }

    pub fn select_next(&mut self) {
        if !self.filtered_apps.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.filtered_apps.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.filtered_apps.is_empty() {
            if self.selected_index == 0 {
                self.selected_index = self.filtered_apps.len() - 1;
            } else {
                self.selected_index -= 1;
            }
        }
    }

    pub fn selected_app(&self) -> Option<&AppInfo> {
        self.filtered_apps.get(self.selected_index)
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

pub const MENU_W: i32 = 240;
pub const ITEM_H: i32 = 26;
pub const MENU_PAD: i32 = 6;

pub fn render_menu(
    menu: &MenuState,
    theme: &Theme,
    text: &TextRenderer,
    pointer_pos: Option<(f64, f64)>,
) -> Option<MemoryRenderBuffer> {
    let (_, pos) = menu.active_menu?;
    let _ = pos;

    let items_count = menu.items.len();
    if items_count == 0 {
        return None;
    }

    let mut total_h = MENU_PAD * 2;
    for item in &menu.items {
        if item.is_separator {
            total_h += 9;
        } else {
            total_h += ITEM_H;
        }
    }

    let w = MENU_W as u32;
    let h = total_h as u32;
    let mut pm = Pixmap::new(w, h)?;

    let dark = theme.dark();
    let bg = if dark {
        [34u8, 34, 40, 240]
    } else {
        [246u8, 246, 250, 242]
    };
    let border = if dark {
        [255u8, 255, 255, 30]
    } else {
        [0u8, 0, 0, 35]
    };
    let fg = theme.menubar_fg();
    let fg_dim = theme.menubar_fg_dim();

    // Drop shadow
    for i in (0..4).rev() {
        let pad = (i + 1) as f32 * 1.5;
        draw_round_rect(&mut pm, 0.5 - pad, 0.5 - pad + 2.0, w as f32 - 1.0 + pad * 2.0, h as f32 - 1.0 + pad * 2.0, 10.0, [0, 0, 0, (12 - i * 3) as u8]);
    }

    // Menu glass background
    draw_round_rect(&mut pm, 0.5, 0.5, w as f32 - 1.0, h as f32 - 1.0, 9.0, bg);
    stroke_round_rect(&mut pm, 0.5, 0.5, w as f32 - 1.0, h as f32 - 1.0, 9.0, border, 1.0);

    // Items
    let mut cur_y = MENU_PAD as f32;
    let local_pointer = pointer_pos.map(|(px, py)| {
        (px - pos.x as f64, py - pos.y as f64)
    });

    for (idx, item) in menu.items.iter().enumerate() {
        if item.is_separator {
            let sep_y = cur_y + 4.0;
            let sep_col = if dark { [255, 255, 255, 20] } else { [0, 0, 0, 25] };
            fill_rect(&mut pm, 10.0, sep_y, w as f32 - 20.0, 1.0, sep_col);
            cur_y += 9.0;
            continue;
        }

        let is_hovered = if let Some((lx, ly)) = local_pointer {
            lx >= 5.0 && lx < (w as f64 - 5.0) && ly >= cur_y as f64 && ly < (cur_y as f64 + ITEM_H as f64)
        } else {
            menu.hovered_item == Some(idx)
        };

        if is_hovered {
            // macOS blue accent highlight
            let hi_bg = theme.accent;
            draw_round_rect(&mut pm, 5.0, cur_y, w as f32 - 10.0, ITEM_H as f32, 5.0, hi_bg);
        }

        let text_col = if is_hovered { [255, 255, 255, 255] } else { fg };
        let sc_col = if is_hovered { [235, 235, 245, 220] } else { fg_dim };

        // Label
        text.draw(&mut pm, &item.label, 14.0, cur_y + 17.5, 12.5, text_col);

        // Shortcut
        if let Some(sc) = &item.shortcut {
            let sc_w = text.width(sc, 11.5);
            text.draw(&mut pm, sc, w as f32 - 14.0 - sc_w, cur_y + 17.5, 11.5, sc_col);
        }

        cur_y += ITEM_H as f32;
    }

    Some(MemoryRenderBuffer::from_slice(
        pm.data(),
        Fourcc::Abgr8888,
        (w as i32, h as i32),
        1,
        Transform::Normal,
        None,
    ))
}

const SPOT_W: i32 = 620;
const SPOT_SEARCH_H: i32 = 56;
const SPOT_ROW_H: i32 = 46;

pub fn render_spotlight(
    spot: &SpotlightState,
    theme: &Theme,
    text: &TextRenderer,
) -> Option<MemoryRenderBuffer> {
    if !spot.open {
        return None;
    }

    let rows = spot.filtered_apps.len();
    let total_h = SPOT_SEARCH_H + if rows > 0 { rows as i32 * SPOT_ROW_H + 12 } else { 0 };

    let w = SPOT_W as u32;
    let h = total_h as u32;
    let mut pm = Pixmap::new(w, h)?;

    let dark = theme.dark();
    let bg = if dark {
        [28u8, 28, 34, 245]
    } else {
        [248u8, 248, 252, 245]
    };
    let border = if dark {
        [255u8, 255, 255, 35]
    } else {
        [0u8, 0, 0, 40]
    };
    let fg = theme.menubar_fg();
    let fg_dim = theme.menubar_fg_dim();

    // Heavy drop shadow
    for i in (0..6).rev() {
        let pad = (i + 1) as f32 * 2.5;
        draw_round_rect(&mut pm, 0.5 - pad, 0.5 - pad + 4.0, w as f32 - 1.0 + pad * 2.0, h as f32 - 1.0 + pad * 2.0, 18.0, [0, 0, 0, (14 - i * 2) as u8]);
    }

    // Spotlight window background
    draw_round_rect(&mut pm, 0.5, 0.5, w as f32 - 1.0, h as f32 - 1.0, 16.0, bg);
    stroke_round_rect(&mut pm, 0.5, 0.5, w as f32 - 1.0, h as f32 - 1.0, 16.0, border, 1.0);

    // Search magnifying glass icon
    draw_search_icon(&mut pm, 24.0, 28.0, fg_dim);

    // Input text / placeholder
    let input_x = 52.0;
    let baseline = 34.0;
    if spot.query.is_empty() {
        text.draw(&mut pm, "Поиск Spotlight", input_x, baseline, 18.0, fg_dim);
    } else {
        text.draw(&mut pm, &spot.query, input_x, baseline, 18.0, fg);
        // Blinking cursor
        let tw = text.width(&spot.query, 18.0);
        let cursor_x = input_x + tw + 2.0;
        let c_color = theme.accent;
        fill_rect(&mut pm, cursor_x, 16.0, 2.0, 24.0, c_color);
    }

    if rows > 0 {
        // Separator between input and results
        let sep_col = if dark { [255, 255, 255, 18] } else { [0, 0, 0, 20] };
        fill_rect(&mut pm, 16.0, SPOT_SEARCH_H as f32, w as f32 - 32.0, 1.0, sep_col);

        let mut row_y = SPOT_SEARCH_H as f32 + 6.0;
        for (i, app) in spot.filtered_apps.iter().enumerate() {
            let is_sel = i == spot.selected_index;
            if is_sel {
                let hi_bg = theme.accent;
                draw_round_rect(&mut pm, 10.0, row_y, w as f32 - 20.0, SPOT_ROW_H as f32 - 2.0, 9.0, hi_bg);
            }

            let app_name_col = if is_sel { [255, 255, 255, 255] } else { fg };
            let app_sub_col = if is_sel { [235, 235, 245, 210] } else { fg_dim };

            // Mini App generic icon badge
            let badge_bg = if is_sel {
                [255, 255, 255, 45]
            } else if dark {
                [50, 50, 58, 255]
            } else {
                [220, 220, 228, 255]
            };
            draw_round_rect(&mut pm, 20.0, row_y + 8.0, 28.0, 28.0, 6.0, badge_bg);
            // First letter inside badge
            if let Some(ch) = app.name.chars().next() {
                let s = ch.to_uppercase().to_string();
                text.draw(&mut pm, &s, 30.0, row_y + 27.0, 13.0, app_name_col);
            }

            // Name
            text.draw(&mut pm, &app.name, 60.0, row_y + 22.0, 14.0, app_name_col);

            // Subtext (comment or path)
            let sub = app.comment.as_deref().unwrap_or(&app.exec);
            text.draw(&mut pm, sub, 60.0, row_y + 36.5, 11.0, app_sub_col);

            row_y += SPOT_ROW_H as f32;
        }
    }

    Some(MemoryRenderBuffer::from_slice(
        pm.data(),
        Fourcc::Abgr8888,
        (w as i32, h as i32),
        1,
        Transform::Normal,
        None,
    ))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn draw_search_icon(pm: &mut Pixmap, cx: f32, cy: f32, color: [u8; 4]) {
    let mut pb = PathBuilder::new();
    pb.push_circle(cx, cy, 6.5);
    pb.move_to(cx + 4.5, cy + 4.5);
    pb.line_to(cx + 9.5, cy + 9.5);
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
        let stroke = tiny_skia::Stroke {
            width: 2.0,
            line_cap: tiny_skia::LineCap::Round,
            ..Default::default()
        };
        pm.stroke_path(&path, &paint, &stroke, SkTransform::identity(), None);
    }
}

fn fill_rect(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, color: [u8; 4]) {
    if let Some(rect) = tiny_skia::Rect::from_xywh(x, y, w, h) {
        let mut paint = Paint::default();
        paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
        pm.fill_rect(rect, &paint, SkTransform::identity(), None);
    }
}

fn draw_round_rect(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, r: f32, color: [u8; 4]) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }
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
    pb.close();
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
        pm.fill_path(&path, &paint, FillRule::Winding, SkTransform::identity(), None);
    }
}

fn stroke_round_rect(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, r: f32, color: [u8; 4], width: f32) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }
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
    pb.close();
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
        let stroke = tiny_skia::Stroke {
            width,
            ..Default::default()
        };
        pm.stroke_path(&path, &paint, &stroke, SkTransform::identity(), None);
    }
}
