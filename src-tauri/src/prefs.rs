use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WindowPrefs {
    pub x: Option<i32>,
    pub y: Option<i32>,
}

fn prefs_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".clawd").join("window.json"))
}

pub fn load() -> WindowPrefs {
    let Some(path) = prefs_path() else { return WindowPrefs::default() };
    let Ok(data) = std::fs::read(&path) else { return WindowPrefs::default() };
    serde_json::from_slice(&data).unwrap_or_default()
}

pub fn save(prefs: &WindowPrefs) {
    let Some(path) = prefs_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(data) = serde_json::to_vec_pretty(prefs) {
        let _ = std::fs::write(&path, data);
    }
}
