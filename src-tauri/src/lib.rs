mod permission;
mod prefs;
mod server;
mod state;
mod tray;

use permission::PendingPermissions;
use state::SharedState;
use tauri::{LogicalPosition, Manager, Position};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(SharedState::new())
        .manage(PendingPermissions::new())
        .invoke_handler(tauri::generate_handler![permission::resolve_permission]);

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
            if let (Some(x), Some(y)) = (saved.x, saved.y) {
                if let Some(win) = app.get_webview_window("pet") {
                    let _ = win.set_position(Position::Logical(LogicalPosition::new(x as f64, y as f64)));
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
                        prefs::save(&prefs::WindowPrefs { x: Some(x), y: Some(y) });
                    }
                }
                _ => {}
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
