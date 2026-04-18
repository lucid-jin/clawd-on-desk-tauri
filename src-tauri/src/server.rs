use std::net::{SocketAddr, TcpListener as StdTcpListener};
use std::path::PathBuf;

use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};

use crate::permission;
use crate::state::{IncomingEvent, SharedState};

const PORT_RANGE: std::ops::Range<u16> = 23333..23338;
const SERVER_ID: &str = "clawd-on-desk-tauri";

#[derive(Clone)]
struct ServerState {
    app: AppHandle,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StateEvent {
    pub state: String,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub event: Option<String>,
    #[serde(default)]
    pub source_pid: Option<i64>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

async fn health() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(header::HeaderName::from_static("x-clawd-server"), SERVER_ID)],
        Json(serde_json::json!({ "ok": true, "server": SERVER_ID })),
    )
}

async fn post_state(
    State(state): State<ServerState>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    // Relay raw event for debugging visibility in M1 console logs
    let _ = state.app.emit("state-change", &payload);

    // Parse into typed incoming event and route through state machine
    match serde_json::from_value::<IncomingEvent>(payload) {
        Ok(incoming) => {
            if let Some(sm_state) = state.app.try_state::<SharedState>() {
                sm_state.handle_incoming(&state.app, incoming);
            }
        }
        Err(err) => {
            eprintln!("[server] state payload parse error: {err}");
        }
    }

    (
        StatusCode::OK,
        [(header::HeaderName::from_static("x-clawd-server"), SERVER_ID)],
        Json(serde_json::json!({ "ok": true })),
    )
}

async fn post_permission(
    State(state): State<ServerState>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    let _ = state.app.emit("permission-request", &payload);
    let decision = permission::request(state.app.clone(), payload).await;
    (
        StatusCode::OK,
        [(header::HeaderName::from_static("x-clawd-server"), SERVER_ID)],
        Json(serde_json::json!({ "behavior": decision.behavior })),
    )
}

fn bind_first_available() -> Option<(StdTcpListener, u16)> {
    for port in PORT_RANGE {
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        match StdTcpListener::bind(addr) {
            Ok(l) => {
                let _ = l.set_nonblocking(true);
                return Some((l, port));
            }
            Err(_) => continue,
        }
    }
    None
}

fn runtime_json_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".clawd").join("runtime.json"))
}

fn write_runtime_json(port: u16) {
    let Some(path) = runtime_json_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let payload = serde_json::json!({
        "port": port,
        "server": SERVER_ID,
        "pid": std::process::id(),
    });
    let _ = std::fs::write(&path, serde_json::to_vec_pretty(&payload).unwrap_or_default());
}

fn remove_runtime_json() {
    if let Some(path) = runtime_json_path() {
        let _ = std::fs::remove_file(path);
    }
}

pub fn spawn(app: AppHandle) {
    let Some((std_listener, port)) = bind_first_available() else {
        eprintln!("[server] no free port in {PORT_RANGE:?} — hook endpoints disabled");
        return;
    };
    eprintln!("[server] bound 127.0.0.1:{port}");
    write_runtime_json(port);

    tauri::async_runtime::spawn(async move {
        let listener = match tokio::net::TcpListener::from_std(std_listener) {
            Ok(l) => l,
            Err(err) => {
                eprintln!("[server] tokio adopt failed: {err}");
                return;
            }
        };

        let router = Router::new()
            .route("/", get(health))
            .route("/state", get(health).post(post_state))
            .route("/permission", post(post_permission))
            .with_state(ServerState { app });

        if let Err(err) = axum::serve(listener, router).await {
            eprintln!("[server] axum exited: {err}");
        }
    });

    // best-effort cleanup on app shutdown — registered from lib.rs exit hook
    let _ = remove_runtime_json;
}

pub fn cleanup() {
    remove_runtime_json();
}

#[allow(dead_code)]
fn _unused(_h: &HeaderMap) {}
