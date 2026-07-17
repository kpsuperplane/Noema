use serde_json::{Value, json};
use std::{
    fs::OpenOptions,
    io::Write,
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

const DEFAULT_TIMING_PATH: &str = "/tmp/noema-turn-timings.jsonl";
const TURN_TIMING_ENV: &str = "NOEMA_TURN_TIMING";

#[derive(Debug, Clone)]
pub(in crate::daemon) struct TurnTiming {
    inner: Arc<TurnTimingInner>,
}

#[derive(Debug)]
struct TurnTimingInner {
    started_at: Instant,
    conversation_id: String,
    turn_id: String,
    turn_index: u64,
    client_message_id: Option<String>,
}

impl TurnTiming {
    pub(in crate::daemon) fn new(
        conversation_id: impl Into<String>,
        turn_id: impl Into<String>,
        turn_index: u64,
        client_message_id: Option<String>,
    ) -> Self {
        Self {
            inner: Arc::new(TurnTimingInner {
                started_at: Instant::now(),
                conversation_id: conversation_id.into(),
                turn_id: turn_id.into(),
                turn_index,
                client_message_id,
            }),
        }
    }

    pub(in crate::daemon) fn mark(&self, event: &str, fields: Value) {
        if !turn_timing_enabled() {
            return;
        }
        let mut payload = base_payload(event);
        payload["elapsed_ms"] = json!(self.inner.started_at.elapsed().as_millis());
        payload["conversation_id"] = json!(self.inner.conversation_id);
        payload["turn_id"] = json!(self.inner.turn_id);
        payload["turn_index"] = json!(self.inner.turn_index);
        if let Some(client_message_id) = &self.inner.client_message_id {
            payload["client_message_id"] = json!(client_message_id);
        }
        merge_fields(&mut payload, fields);
        emit(payload);
    }
}

pub(crate) fn mark_turn_timing_event(
    event: &str,
    conversation_id: &str,
    client_message_id: Option<&str>,
    fields: Value,
) {
    if !turn_timing_enabled() {
        return;
    }
    let mut payload = base_payload(event);
    payload["conversation_id"] = json!(conversation_id);
    if let Some(client_message_id) = client_message_id {
        payload["client_message_id"] = json!(client_message_id);
    }
    merge_fields(&mut payload, fields);
    emit(payload);
}

fn base_payload(event: &str) -> Value {
    json!({
        "category": "turn_timing",
        "event": event,
        "unix_ms": unix_ms(),
    })
}

fn merge_fields(payload: &mut Value, fields: Value) {
    let Some(payload) = payload.as_object_mut() else {
        return;
    };
    let Value::Object(fields) = fields else {
        return;
    };
    for (key, value) in fields {
        payload.insert(key, value);
    }
}

fn emit(payload: Value) {
    if !turn_timing_enabled() {
        return;
    }
    let _guard = emit_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let line = payload.to_string();
    eprintln!("[noema_turn_timing] {line}");
    let path = std::env::var_os("NOEMA_TURN_TIMING_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_TIMING_PATH));
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) else {
        return;
    };
    let _ = writeln!(file, "{line}");
}

fn emit_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn turn_timing_enabled() -> bool {
    timing_enabled_from_env(std::env::var(TURN_TIMING_ENV).ok().as_deref())
}

fn timing_enabled_from_env(value: Option<&str>) -> bool {
    let Some(value) = value else {
        return false;
    };
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timing_is_disabled_when_flag_is_missing_or_false() {
        assert!(!timing_enabled_from_env(None));
        assert!(!timing_enabled_from_env(Some("")));
        assert!(!timing_enabled_from_env(Some("0")));
        assert!(!timing_enabled_from_env(Some("false")));
    }

    #[test]
    fn timing_is_enabled_for_truthy_flag_values() {
        assert!(timing_enabled_from_env(Some("1")));
        assert!(timing_enabled_from_env(Some("true")));
        assert!(timing_enabled_from_env(Some("yes")));
    }
}
