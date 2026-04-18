use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Prefs {
    #[serde(default)]
    pub window: WindowPrefs,
    #[serde(default)]
    pub dnd: bool,
    #[serde(default)]
    pub mini_mode: bool,
    #[serde(default)]
    pub hide_dock: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WindowPrefs {
    pub x: Option<i32>,
    pub y: Option<i32>,
}

fn path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".clawd").join("prefs.json"))
}

pub fn load() -> Prefs {
    let Some(p) = path() else { return Prefs::default() };
    let Ok(data) = std::fs::read(&p) else { return migrate_legacy() };
    serde_json::from_slice(&data).unwrap_or_else(|_| migrate_legacy())
}

pub fn save(prefs: &Prefs) {
    let Some(p) = path() else { return };
    if let Some(parent) = p.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(data) = serde_json::to_vec_pretty(prefs) {
        let _ = std::fs::write(&p, data);
    }
}

/// Migrate from the old window.json (M7) to the new prefs.json format.
fn migrate_legacy() -> Prefs {
    let Some(home) = dirs::home_dir() else { return Prefs::default() };
    let legacy = home.join(".clawd").join("window.json");
    if let Ok(data) = std::fs::read(&legacy) {
        if let Ok(wp) = serde_json::from_slice::<WindowPrefs>(&data) {
            let prefs = Prefs { window: wp, ..Default::default() };
            save(&prefs);
            let _ = std::fs::remove_file(&legacy);
            return prefs;
        }
    }
    Prefs::default()
}

/// Convenience: merge a window-only update without touching other fields.
pub fn save_window(wp: WindowPrefs) {
    let mut prefs = load();
    prefs.window = wp;
    save(&prefs);
}
