mod server;
mod state;
mod tray;

use state::SharedState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(SharedState::new());

    #[cfg(target_os = "macos")]
    {
        builder = builder.manage(tray::DockState::new());
    }

    builder
        .setup(|app| {
            server::spawn(app.handle().clone());
            tray::build(app.handle())?;
            #[cfg(target_os = "macos")]
            {
                // Start hidden from Dock by default — pet lives in the tray.
                let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
                if let Some(state) = app.try_state::<tray::DockState>() {
                    *state.hidden.lock().unwrap() = true;
                }
            }
            Ok(())
        })
        .on_window_event(|_window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                server::cleanup();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
