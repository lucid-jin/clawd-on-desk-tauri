use std::sync::Mutex;

use tauri::{AppHandle, Emitter, LogicalPosition, Manager, PhysicalPosition, Position, Runtime};

const SNAP_TOLERANCE: i32 = 30; // logical px from the right work-area edge
const PEEK_REVEAL: i32 = 20; // how many logical px of the pet stay visible in mini

pub struct MiniState {
    pub active: Mutex<bool>,
}

impl MiniState {
    pub fn new() -> Self {
        Self { active: Mutex::new(false) }
    }

    pub fn is_active(&self) -> bool {
        *self.active.lock().unwrap()
    }
}

fn work_area_for_position<R: Runtime>(
    app: &AppHandle<R>,
    pos: PhysicalPosition<i32>,
) -> Option<(PhysicalPosition<i32>, tauri::PhysicalSize<u32>)> {
    // Walk monitors, return the one containing pos; fall back to primary.
    let monitors = app.available_monitors().ok()?;
    let mut hit: Option<tauri::Monitor> = None;
    for m in monitors {
        let mp = m.position();
        let ms = m.size();
        let right = mp.x + ms.width as i32;
        let bottom = mp.y + ms.height as i32;
        if pos.x >= mp.x && pos.x < right && pos.y >= mp.y && pos.y < bottom {
            hit = Some(m);
            break;
        }
    }
    let m = hit.or_else(|| app.primary_monitor().ok().flatten())?;
    Some((*m.position(), *m.size()))
}

/// Check if the pet window is near the right edge of its current monitor.
/// If so, snap it so only PEEK_REVEAL logical pixels remain on-screen and
/// enter mini mode. Returns true if the snap happened.
pub fn maybe_snap_right<R: Runtime>(app: &AppHandle<R>) -> bool {
    let Some(win) = app.get_webview_window("pet") else { return false };
    let scale = win.scale_factor().unwrap_or(1.0);
    let Ok(outer_pos) = win.outer_position() else { return false };
    let Ok(outer_size) = win.outer_size() else { return false };
    let Some((mon_pos, mon_size)) = work_area_for_position(app, outer_pos) else { return false };

    let win_right_phys = outer_pos.x + outer_size.width as i32;
    let mon_right_phys = mon_pos.x + mon_size.width as i32;
    let gap_logical = ((mon_right_phys - win_right_phys) as f64 / scale) as i32;

    if gap_logical > SNAP_TOLERANCE {
        return false;
    }

    // Snap: new logical x = monitor_right_logical - PEEK_REVEAL.
    let mon_right_logical = mon_pos.x as f64 / scale + mon_size.width as f64 / scale;
    let outer_y_logical = outer_pos.y as f64 / scale;
    let target_x = mon_right_logical - PEEK_REVEAL as f64;
    let _ = win.set_position(Position::Logical(LogicalPosition::new(target_x, outer_y_logical)));

    if let Some(state) = app.try_state::<MiniState>() {
        *state.active.lock().unwrap() = true;
    }
    let _ = app.emit("mini-state", true);
    true
}

/// Exit mini mode — slide the window back into the work area so the user can
/// grab it again, and notify the renderer.
pub fn exit_mini<R: Runtime>(app: &AppHandle<R>) {
    let Some(win) = app.get_webview_window("pet") else { return };
    let scale = win.scale_factor().unwrap_or(1.0);
    if let (Ok(outer_pos), Ok(outer_size)) = (win.outer_position(), win.outer_size()) {
        if let Some((mon_pos, mon_size)) = work_area_for_position(app, outer_pos) {
            let mon_right_logical = mon_pos.x as f64 / scale + mon_size.width as f64 / scale;
            let win_w_logical = outer_size.width as f64 / scale;
            let target_x = mon_right_logical - win_w_logical - 40.0;
            let outer_y_logical = outer_pos.y as f64 / scale;
            let _ = win.set_position(Position::Logical(LogicalPosition::new(target_x, outer_y_logical)));
        }
    }
    if let Some(state) = app.try_state::<MiniState>() {
        *state.active.lock().unwrap() = false;
    }
    let _ = app.emit("mini-state", false);
}

#[tauri::command]
pub fn maybe_snap_right_cmd<R: Runtime>(app: AppHandle<R>) -> bool {
    maybe_snap_right(&app)
}

#[tauri::command]
pub fn exit_mini_cmd<R: Runtime>(app: AppHandle<R>) {
    exit_mini(&app);
}

#[tauri::command]
pub fn mini_active<R: Runtime>(app: AppHandle<R>) -> bool {
    app.try_state::<MiniState>().map(|s| s.is_active()).unwrap_or(false)
}
