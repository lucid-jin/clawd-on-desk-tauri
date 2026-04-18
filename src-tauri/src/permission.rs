use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Manager, Runtime, WebviewUrl, WebviewWindowBuilder};
use tokio::sync::oneshot;

const REQUEST_TIMEOUT_SECS: u64 = 600;

#[derive(Debug, Clone, Serialize)]
pub struct PermissionDecision {
    pub behavior: String,
}

pub struct PendingPermissions {
    next_id: AtomicU64,
    map: Mutex<HashMap<String, oneshot::Sender<PermissionDecision>>>,
}

impl PendingPermissions {
    pub fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
            map: Mutex::new(HashMap::new()),
        }
    }

    fn alloc_id(&self) -> String {
        let n = self.next_id.fetch_add(1, Ordering::SeqCst);
        format!("perm-{n}")
    }

    pub fn resolve(&self, id: &str, decision: PermissionDecision) -> bool {
        let mut map = self.map.lock().unwrap();
        if let Some(tx) = map.remove(id) {
            let _ = tx.send(decision);
            true
        } else {
            false
        }
    }
}

pub async fn request<R: Runtime>(app: AppHandle<R>, payload: Value) -> PermissionDecision {
    let Some(pending) = app.try_state::<PendingPermissions>() else {
        return PermissionDecision { behavior: "deny".into() };
    };

    let id = pending.alloc_id();
    let (tx, rx) = oneshot::channel();
    pending.map.lock().unwrap().insert(id.clone(), tx);

    // Spawn bubble window
    let encoded = urlencoding::encode(&payload.to_string()).into_owned();
    let url = format!("bubble-tauri.html?id={}&payload={}", id, encoded);
    let label = format!("bubble-{}", id);

    let build_result = WebviewWindowBuilder::new(&app, label.clone(), WebviewUrl::App(url.into()))
        .title("Clawd Permission")
        .inner_size(420.0, 220.0)
        .transparent(true)
        .decorations(false)
        .always_on_top(true)
        .resizable(false)
        .skip_taskbar(true)
        .shadow(false)
        .accept_first_mouse(true)
        .focused(true)
        .build();

    if let Err(err) = build_result {
        eprintln!("[permission] bubble build failed: {err}");
        pending.map.lock().unwrap().remove(&id);
        return PermissionDecision { behavior: "deny".into() };
    }

    let close_bubble = || {
        if let Some(win) = app.get_webview_window(&label) {
            let _ = win.close();
        }
    };

    let result = match tokio::time::timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS), rx).await {
        Ok(Ok(decision)) => decision,
        _ => {
            pending.map.lock().unwrap().remove(&id);
            PermissionDecision { behavior: "deny".into() }
        }
    };

    // Always close the bubble window when we return — belt & suspenders,
    // even if the JS-side w.close() succeeded.
    close_bubble();
    result
}

#[tauri::command]
pub fn resolve_permission<R: Runtime>(
    app: AppHandle<R>,
    id: String,
    decision: String,
) -> Result<(), String> {
    let Some(pending) = app.try_state::<PendingPermissions>() else {
        return Err("pending permissions state not managed".into());
    };
    pending.resolve(&id, PermissionDecision { behavior: decision });
    Ok(())
}
