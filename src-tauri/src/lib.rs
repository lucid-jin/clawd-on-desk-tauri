mod mini;
mod permission;
mod prefs;
mod server;
mod state;
mod tray;

use mini::MiniState;
use permission::PendingPermissions;
use state::SharedState;
use tauri::{AppHandle, Emitter, LogicalPosition, Manager, Position, Runtime};

#[tauri::command]
fn get_prefs() -> prefs::Prefs {
    prefs::load()
}

#[tauri::command]
fn toggle_dnd_cmd<R: Runtime>(app: AppHandle<R>) -> bool {
    let new = app
        .try_state::<SharedState>()
        .map(|s| {
            let v = s.toggle_dnd();
            s.notify_resolve(&app);
            v
        })
        .unwrap_or(false);
    let mut p = prefs::load();
    p.dnd = new;
    prefs::save(&p);
    let _ = app.emit("dnd-changed", new);
    new
}

#[tauri::command]
fn toggle_mini_cmd<R: Runtime>(app: AppHandle<R>) -> bool {
    let active = app
        .try_state::<MiniState>()
        .map(|s| s.is_active())
        .unwrap_or(false);
    if active {
        mini::exit_mini(&app);
    } else {
        // Pull pet near right edge first, then snap.
        if let Some(win) = app.get_webview_window("pet") {
            let scale = win.scale_factor().unwrap_or(1.0);
            if let (Ok(outer_pos), Ok(outer_size)) = (win.outer_position(), win.outer_size()) {
                if let Ok(Some(m)) = app.primary_monitor() {
                    let mp = m.position();
                    let ms = m.size();
                    let right_logical = mp.x as f64 / scale + ms.width as f64 / scale;
                    let win_w_logical = outer_size.width as f64 / scale;
                    let outer_y_logical = outer_pos.y as f64 / scale;
                    let target_x = right_logical - win_w_logical - 5.0;
                    let _ = win.set_position(Position::Logical(LogicalPosition::new(target_x, outer_y_logical)));
                }
            }
        }
        mini::maybe_snap_right(&app);
    }
    let new_active = !active;
    let mut p = prefs::load();
    p.mini_mode = new_active;
    prefs::save(&p);
    new_active
}

#[cfg(target_os = "macos")]
#[tauri::command]
fn toggle_dock_cmd<R: Runtime>(app: AppHandle<R>) -> bool {
    let hidden = app
        .try_state::<tray::DockState>()
        .map(|s| *s.hidden.lock().unwrap())
        .unwrap_or(true);
    // Flip
    let new_hidden = !hidden;
    let policy = if new_hidden {
        tauri::ActivationPolicy::Accessory
    } else {
        tauri::ActivationPolicy::Regular
    };
    if let Some(s) = app.try_state::<tray::DockState>() {
        *s.hidden.lock().unwrap() = new_hidden;
    }
    let _ = app.set_activation_policy(policy);
    let mut p = prefs::load();
    p.hide_dock = new_hidden;
    prefs::save(&p);
    let _ = app.emit("dock-changed", new_hidden);
    new_hidden
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
fn toggle_dock_cmd<R: Runtime>(_app: AppHandle<R>) -> bool {
    false
}

#[tauri::command]
fn open_settings<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.show();
        let _ = win.set_focus();
        return Ok(());
    }
    tauri::WebviewWindowBuilder::new(
        &app,
        "settings",
        tauri::WebviewUrl::App("settings-tauri.html".into()),
    )
    .title("Clawd Settings")
    .inner_size(440.0, 320.0)
    .resizable(false)
    .focused(true)
    .build()
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(SharedState::new())
        .manage(PendingPermissions::new())
        .manage(MiniState::new())
        .invoke_handler(tauri::generate_handler![
            permission::resolve_permission,
            mini::maybe_snap_right_cmd,
            mini::exit_mini_cmd,
            mini::mini_active,
            get_prefs,
            toggle_dnd_cmd,
            toggle_mini_cmd,
            toggle_dock_cmd,
            open_settings,
        ]);

    #[cfg(target_os = "macos")]
    {
        builder = builder.manage(tray::DockState::new());
    }

    builder
        .setup(|app| {
            server::spawn(app.handle().clone());
            tray::build(app.handle())?;

            // Restore saved window position (if any).
            let saved = prefs::load();
            if let (Some(x), Some(y)) = (saved.window.x, saved.window.y) {
                if let Some(win) = app.get_webview_window("pet") {
                    let _ = win.set_position(Position::Logical(LogicalPosition::new(x as f64, y as f64)));
                }
            }

            // Hydrate DND / mini / dock from prefs.
            if saved.dnd {
                if let Some(s) = app.try_state::<SharedState>() {
                    *s.dnd.lock().unwrap() = true;
                }
            }

            #[cfg(target_os = "macos")]
            {
                let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
                if let Some(state) = app.try_state::<tray::DockState>() {
                    *state.hidden.lock().unwrap() = true;
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            match event {
                tauri::WindowEvent::Destroyed => server::cleanup(),
                tauri::WindowEvent::Moved(_) => {
                    // Persist position on every move. Cheap enough for a pet window.
                    if let Ok(pos) = window.outer_position() {
                        let scale = window.scale_factor().unwrap_or(1.0);
                        let x = (pos.x as f64 / scale) as i32;
                        let y = (pos.y as f64 / scale) as i32;
                        prefs::save_window(prefs::WindowPrefs { x: Some(x), y: Some(y) });
                    }
                }
                _ => {}
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
