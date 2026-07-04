use serde_json::{Value, json};
use std::{
    fs::OpenOptions,
    io::Write,
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

const DEFAULT_TIMING_PATH: &str = "/tmp/noema-turn-timings.jsonl";

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

pub(crate) fn mark_graphql_turn_event(
    event: &str,
    conversation_id: &str,
    client_message_id: Option<&str>,
    fields: Value,
) {
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

fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
