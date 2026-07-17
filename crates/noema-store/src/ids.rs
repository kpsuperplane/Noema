use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::StoreError;

static ID_COUNTER: AtomicU64 = AtomicU64::new(1);

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

pub(super) fn now_string() -> String {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or_else(
        |_| "0".to_string(),
        |duration| duration.as_secs().to_string(),
    )
}
