use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Runtime};

pub const PRIORITY: &[(&str, u8)] = &[
    ("error", 8),
    ("notification", 7),
    ("sweeping", 6),
    ("attention", 5),
    ("carrying", 4),
    ("juggling", 4),
    ("working", 3),
    ("thinking", 2),
    ("idle", 1),
    ("sleeping", 0),
];

pub const ONESHOT_STATES: &[&str] = &["attention", "error", "sweeping", "notification", "carrying"];

pub fn priority_of(state: &str) -> u8 {
    PRIORITY
        .iter()
        .find(|(s, _)| *s == state)
        .map(|(_, p)| *p)
        .unwrap_or(0)
}

pub fn is_oneshot(state: &str) -> bool {
    ONESHOT_STATES.contains(&state)
}

fn min_display_ms(state: &str) -> u64 {
    match state {
        "error" => 5000,
        "attention" | "notification" => 4000,
        "carrying" => 3000,
        "sweeping" => 2000,
        "working" | "thinking" => 1000,
        _ => 0,
    }
}

fn auto_return_ms(state: &str) -> Option<u64> {
    match state {
        "error" => Some(5000),
        "notification" | "attention" => Some(4000),
        "carrying" => Some(3000),
        "sweeping" => Some(2000),
        _ => None,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DisplayState {
    pub state: String,
    pub subagent_count: usize,
    pub session_count: usize,
}

#[derive(Debug, Clone)]
struct Session {
    state: String,
    agent_id: String,
    updated_at: Instant,
    subagent_count: usize,
    headless: bool,
}

pub struct StateMachine {
    sessions: HashMap<String, Session>,
    current_display: String,
    display_started_at: Instant,
}

impl StateMachine {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            current_display: "idle".to_string(),
            display_started_at: Instant::now(),
        }
    }

    fn session_key(agent_id: &str, session_id: &str) -> String {
        format!("{agent_id}:{session_id}")
    }

    /// Core entry point: incoming state event from an agent hook.
    /// Returns the new display state if it changed.
    pub fn apply_event(&mut self, incoming: IncomingEvent) -> Option<DisplayState> {
        let agent = incoming.agent_id.as_deref().unwrap_or("claude-code");
        let session = incoming.session_id.as_deref().unwrap_or("default");
        let key = Self::session_key(agent, session);

        if incoming.state == "sleeping" || incoming.event.as_deref() == Some("SessionEnd") {
            self.sessions.remove(&key);
        } else if is_oneshot(&incoming.state) {
            // Oneshot states don't update session state; they flash and go back.
            // Store a temporary session anyway so resolve() picks the priority.
            self.sessions.insert(
                key,
                Session {
                    state: incoming.state.clone(),
                    agent_id: agent.to_string(),
                    updated_at: Instant::now(),
                    subagent_count: incoming.subagent_count.unwrap_or(0),
                    headless: incoming.headless.unwrap_or(false),
                },
            );
        } else {
            self.sessions.insert(
                key,
                Session {
                    state: incoming.state.clone(),
                    agent_id: agent.to_string(),
                    updated_at: Instant::now(),
                    subagent_count: incoming.subagent_count.unwrap_or(0),
                    headless: incoming.headless.unwrap_or(false),
                },
            );
        }

        self.maybe_change_display()
    }

    /// Remove the oneshot session (called by auto-return timer).
    pub fn clear_oneshot(&mut self, agent: &str, session: &str) -> Option<DisplayState> {
        let key = Self::session_key(agent, session);
        if let Some(s) = self.sessions.get(&key) {
            if is_oneshot(&s.state) {
                self.sessions.remove(&key);
            }
        }
        self.maybe_change_display()
    }

    fn resolve(&self) -> DisplayState {
        let mut best = "sleeping";
        let mut has_non_headless = false;
        let mut session_count = 0usize;
        let mut subagent_count = 0usize;

        for s in self.sessions.values() {
            if s.headless {
                continue;
            }
            has_non_headless = true;
            session_count += 1;
            subagent_count += s.subagent_count;
            if priority_of(&s.state) > priority_of(best) {
                best = &s.state;
            }
        }

        if self.sessions.is_empty() || !has_non_headless {
            best = "idle";
        }

        // working sub-state derivation — only applies when best==working
        let resolved = match best {
            "working" => match session_count {
                0 | 1 => "typing",
                2 => "juggling",
                _ => "building",
            },
            "juggling" => match subagent_count {
                0 | 1 => "juggling",
                _ => "conducting",
            },
            other => other,
        };

        DisplayState {
            state: resolved.to_string(),
            subagent_count,
            session_count,
        }
    }

    fn maybe_change_display(&mut self) -> Option<DisplayState> {
        let next = self.resolve();

        if next.state == self.current_display {
            return None;
        }

        let now = Instant::now();
        let min_ms = min_display_ms(&self.current_display);
        if min_ms > 0 && now.duration_since(self.display_started_at) < Duration::from_millis(min_ms)
        {
            let lower_priority = priority_of(&next.state) < priority_of(&self.current_display);
            if lower_priority {
                // Hold current display for min duration; resolve() will be called
                // again by the auto-return timer or next event.
                return None;
            }
        }

        self.current_display = next.state.clone();
        self.display_started_at = now;
        Some(next)
    }

    pub fn current(&self) -> DisplayState {
        DisplayState {
            state: self.current_display.clone(),
            session_count: self.sessions.len(),
            subagent_count: 0,
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct IncomingEvent {
    pub state: String,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub event: Option<String>,
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub subagent_count: Option<usize>,
    #[serde(default)]
    pub headless: Option<bool>,
}

/// Shared state machine + app handle for auto-return timer bookkeeping.
pub struct SharedState {
    pub sm: Mutex<StateMachine>,
    pub dnd: Mutex<bool>,
}

impl SharedState {
    pub fn new() -> Self {
        Self {
            sm: Mutex::new(StateMachine::new()),
            dnd: Mutex::new(false),
        }
    }

    pub fn toggle_dnd(&self) -> bool {
        let mut g = self.dnd.lock().unwrap();
        *g = !*g;
        *g
    }

    pub fn is_dnd(&self) -> bool {
        *self.dnd.lock().unwrap()
    }

    /// Force-emit current display without changing state (used after DND toggle).
    pub fn notify_resolve<R: Runtime>(&self, app: &AppHandle<R>) {
        let disp = if self.is_dnd() {
            DisplayState {
                state: "sleeping".into(),
                subagent_count: 0,
                session_count: 0,
            }
        } else {
            self.sm.lock().unwrap().current()
        };
        let _ = app.emit("display-state", &disp);
    }

    pub fn handle_incoming(&self, app: &AppHandle, incoming: IncomingEvent) {
        if self.is_dnd() {
            // In DND mode we silently drop incoming events — pet stays asleep.
            return;
        }

        let (change, scheduled) = {
            let mut sm = self.sm.lock().unwrap();
            let change = sm.apply_event(incoming.clone());
            let scheduled = auto_return_ms(&incoming.state).map(|ms| {
                (
                    ms,
                    incoming.agent_id.clone().unwrap_or_else(|| "claude-code".into()),
                    incoming.session_id.clone().unwrap_or_else(|| "default".into()),
                )
            });
            (change, scheduled)
        };

        if let Some(disp) = change {
            let _ = app.emit("display-state", &disp);
        }

        if let Some((ms, agent, session)) = scheduled {
            let app_clone = app.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(Duration::from_millis(ms)).await;
                // Use the managed SharedState to clear oneshot.
                use tauri::Manager;
                if let Some(state) = app_clone.try_state::<SharedState>() {
                    let change = {
                        let mut sm = state.sm.lock().unwrap();
                        sm.clear_oneshot(&agent, &session)
                    };
                    if let Some(disp) = change {
                        let _ = app_clone.emit("display-state", &disp);
                    }
                }
            });
        }
    }
}

// ──── Unit tests ────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    fn ev(state: &str, session: &str) -> IncomingEvent {
        IncomingEvent {
            state: state.into(),
            session_id: Some(session.into()),
            event: None,
            agent_id: Some("claude-code".into()),
            subagent_count: None,
            headless: None,
        }
    }

    #[test]
    fn empty_resolves_to_idle() {
        let sm = StateMachine::new();
        assert_eq!(sm.resolve().state, "idle");
    }

    #[test]
    fn priority_error_beats_working() {
        let mut sm = StateMachine::new();
        sm.apply_event(ev("working", "s1"));
        let d = sm.apply_event(ev("error", "s2")).unwrap();
        assert_eq!(d.state, "error");
    }

    #[test]
    fn single_working_session_becomes_typing() {
        let mut sm = StateMachine::new();
        let d = sm.apply_event(ev("working", "s1")).unwrap();
        assert_eq!(d.state, "typing");
    }

    #[test]
    fn two_working_sessions_becomes_juggling() {
        let mut sm = StateMachine::new();
        sm.apply_event(ev("working", "s1"));
        let d = sm.apply_event(ev("working", "s2")).unwrap();
        assert_eq!(d.state, "juggling");
    }

    #[test]
    fn three_working_sessions_becomes_building() {
        let mut sm = StateMachine::new();
        sm.apply_event(ev("working", "s1"));
        sm.apply_event(ev("working", "s2"));
        let d = sm.apply_event(ev("working", "s3")).unwrap();
        assert_eq!(d.state, "building");
    }

    #[test]
    fn session_end_removes_session() {
        let mut sm = StateMachine::new();
        sm.apply_event(ev("working", "s1"));
        let mut end = ev("sleeping", "s1");
        end.event = Some("SessionEnd".into());
        let d = sm.apply_event(end).unwrap();
        assert_eq!(d.state, "idle");
    }

    #[test]
    fn priority_lookup() {
        assert_eq!(priority_of("error"), 8);
        assert_eq!(priority_of("idle"), 1);
        assert_eq!(priority_of("unknown"), 0);
    }

    #[test]
    fn oneshot_detection() {
        assert!(is_oneshot("error"));
        assert!(is_oneshot("notification"));
        assert!(!is_oneshot("working"));
        assert!(!is_oneshot("idle"));
    }
}
