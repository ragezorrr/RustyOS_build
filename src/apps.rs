//! Desktop application discovery from .desktop files.

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct AppInfo {
    pub name: String,
    pub exec: String,
    pub comment: Option<String>,
}

pub fn scan_apps() -> Vec<AppInfo> {
    let mut dirs = vec![PathBuf::from("/usr/share/applications")];
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(PathBuf::from(home).join(".local/share/applications"));
    }

    let mut apps = Vec::new();
    let mut seen_execs = std::collections::HashSet::new();

    for dir in dirs {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("desktop") {
                    if let Some(app) = parse_desktop_file(&path) {
                        if seen_execs.insert(app.exec.clone()) {
                            apps.push(app);
                        }
                    }
                }
            }
        }
    }

    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps
}

fn parse_desktop_file(path: &Path) -> Option<AppInfo> {
    let content = fs::read_to_string(path).ok()?;
    let mut name = None;
    let mut exec = None;
    let mut comment = None;
    let mut nodisplay = false;
    let mut in_main_section = false;

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_main_section = line == "[Desktop Entry]";
            continue;
        }
        if !in_main_section {
            continue;
        }

        if let Some(val) = line.strip_prefix("Name=") {
            if name.is_none() {
                name = Some(val.trim().to_string());
            }
        } else if let Some(val) = line.strip_prefix("Exec=") {
            if exec.is_none() {
                // Remove format specifiers like %u, %F, %U
                let raw = val.trim();
                let clean = raw.split('%').next().unwrap_or(raw).trim();
                exec = Some(clean.to_string());
            }
        } else if let Some(val) = line.strip_prefix("Comment=") {
            if comment.is_none() {
                comment = Some(val.trim().to_string());
            }
        } else if line == "NoDisplay=true" || line == "Hidden=true" {
            nodisplay = true;
        }
    }

    if nodisplay {
        return None;
    }

    let name = name?;
    let exec = exec?;
    if exec.is_empty() {
        return None;
    }

    Some(AppInfo {
        name,
        exec,
        comment,
    })
}
