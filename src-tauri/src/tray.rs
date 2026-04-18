use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, Runtime,
};

use crate::mini;
use crate::state::SharedState;

pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let sleep_item = MenuItem::with_id(app, "sleep", "Sleep / Wake (DND)", true, None::<&str>)?;
    let mini_item = MenuItem::with_id(app, "mini_toggle", "Toggle Mini Mode", true, None::<&str>)?;
    let settings_item = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let show_item = MenuItem::with_id(app, "show", "Show Pet", true, None::<&str>)?;
    let hide_item = MenuItem::with_id(app, "hide", "Hide Pet", true, None::<&str>)?;
    let hide_dock = CheckMenuItem::with_id(app, "hide_dock", "Hide Dock Icon", true, false, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let sep3 = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Clawd", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &sleep_item, &mini_item,
            &sep1,
            &settings_item,
            &sep2,
            &show_item, &hide_item, &hide_dock,
            &sep3,
            &quit,
        ],
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
        "settings" => {
            if let Some(win) = app.get_webview_window("settings") {
                let _ = win.show();
                let _ = win.set_focus();
            } else {
                let _ = tauri::WebviewWindowBuilder::new(
                    app,
                    "settings",
                    tauri::WebviewUrl::App("settings-tauri.html".into()),
                )
                .title("Clawd Settings")
                .inner_size(440.0, 320.0)
                .resizable(false)
                .focused(true)
                .build();
            }
        }
        "mini_toggle" => {
            let active = app
                .try_state::<mini::MiniState>()
                .map(|s| s.is_active())
                .unwrap_or(false);
            if active {
                mini::exit_mini(app);
            } else {
                // Force-snap: temporarily move pet near right edge and snap.
                if let Some(win) = app.get_webview_window("pet") {
                    let scale = win.scale_factor().unwrap_or(1.0);
                    if let (Ok(outer_pos), Ok(outer_size)) = (win.outer_position(), win.outer_size()) {
                        if let Ok(Some(m)) = app.primary_monitor().map(|m| m) {
                            let mp = m.position();
                            let ms = m.size();
                            let right_logical = mp.x as f64 / scale + ms.width as f64 / scale;
                            let win_w_logical = outer_size.width as f64 / scale;
                            let outer_y_logical = outer_pos.y as f64 / scale;
                            let target_x = right_logical - win_w_logical - 5.0;
                            let _ = win.set_position(tauri::Position::Logical(tauri::LogicalPosition::new(
                                target_x,
                                outer_y_logical,
                            )));
                        }
                    }
                }
                mini::maybe_snap_right(app);
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
