use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::store::StoreError;

static ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Encode `id` as a `prefix`-tagged lowercase-hex SurrealDB record fragment.
pub(super) fn hex_record_fragment(prefix: &str, id: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    let mut fragment = String::with_capacity(prefix.len() + id.len() * 2);
    fragment.push_str(prefix);
    for byte in id.bytes() {
        fragment.push(HEX[(byte >> 4) as usize] as char);
        fragment.push(HEX[(byte & 0x0f) as usize] as char);
    }
    fragment
}

/// Build a [`StoreError::InvalidEnum`] for an unrecognized stored enum label.
pub(super) fn invalid_enum<T>(kind: &'static str, value: &str) -> Result<T, StoreError> {
    Err(StoreError::InvalidEnum {
        kind,
        value: value.to_string(),
    })
}

pub(super) fn allocate_id(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let counter = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}:{nanos:x}{counter:x}")
}

pub(super) fn record_fragment(id: &str) -> String {
    id.chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
}

pub(super) fn now_string() -> String {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or_else(
        |_| "0".to_string(),
        |duration| duration.as_secs().to_string(),
    )
}
