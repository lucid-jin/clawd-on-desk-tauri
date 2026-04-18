use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, Runtime,
};

use crate::state::SharedState;

pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let sleep_item = MenuItem::with_id(app, "sleep", "Sleep / Wake (DND)", true, None::<&str>)?;
    let show_item = MenuItem::with_id(app, "show", "Show Pet", true, None::<&str>)?;
    let hide_item = MenuItem::with_id(app, "hide", "Hide Pet", true, None::<&str>)?;
    let hide_dock = CheckMenuItem::with_id(app, "hide_dock", "Hide Dock Icon", true, false, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Clawd", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[&sleep_item, &sep1, &show_item, &hide_item, &hide_dock, &sep2, &quit],
    )?;

    let icon = tauri::include_image!("../src/assets/tray-iconTemplate.png");
    let _tray = TrayIconBuilder::with_id("clawd-tray")
        .icon(icon)
        .icon_as_template(true)
        .tooltip("Clawd")
        .menu(&menu)
        .on_menu_event(|app, event| handle_menu_event(app, event.id().as_ref()))
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { .. } = event {
                let _ = tray.app_handle().emit("tray-click", ());
            }
        })
        .build(app)?;

    Ok(())
}

fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, id: &str) {
    match id {
        "quit" => {
            app.exit(0);
        }
        "sleep" => {
            if let Some(state) = app.try_state::<SharedState>() {
                let new_dnd = state.toggle_dnd();
                let _ = app.emit("dnd-changed", new_dnd);
                // Nudge the state machine to re-emit display
                state.notify_resolve(app);
            }
        }
        "show" => {
            if let Some(win) = app.get_webview_window("pet") {
                let _ = win.show();
                let _ = win.set_focus();
            }
        }
        "hide" => {
            if let Some(win) = app.get_webview_window("pet") {
                let _ = win.hide();
            }
        }
        "hide_dock" => {
            #[cfg(target_os = "macos")]
            {
                // Toggle dock visibility via activation policy
                let policy = app
                    .try_state::<DockState>()
                    .map(|s| s.toggle())
                    .unwrap_or(tauri::ActivationPolicy::Accessory);
                let _ = app.set_activation_policy(policy);
            }
        }
        _ => {}
    }
}

#[cfg(target_os = "macos")]
pub struct DockState {
    pub hidden: std::sync::Mutex<bool>,
}

#[cfg(target_os = "macos")]
impl DockState {
    pub fn new() -> Self {
        Self {
            hidden: std::sync::Mutex::new(false),
        }
    }

    pub fn toggle(&self) -> tauri::ActivationPolicy {
        let mut g = self.hidden.lock().unwrap();
        *g = !*g;
        if *g {
            tauri::ActivationPolicy::Accessory
        } else {
            tauri::ActivationPolicy::Regular
        }
    }
}
