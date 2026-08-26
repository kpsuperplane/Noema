//! Append-only developer diagnostic logging.

use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::Mutex,
};

#[cfg(test)]
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{NoemaPaths, ensure_private_dir, ensure_private_file};

const MAX_LOG_BYTES: usize = 48 * 1024 * 1024;
const MAX_EVENT_BYTES: usize = 256 * 1024;
const MAX_RAW_BYTES: usize = 64 * 1024;
static ERROR_LOG_LOCK: Mutex<()> = Mutex::new(());

/// Append-only developer diagnostic logger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemErrorLogger {
    path: PathBuf,
}

impl SystemErrorLogger {
    /// Create a logger for an explicit `errors.log` path.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Create a logger from resolved Noema paths.
    #[must_use]
    pub fn from_paths(paths: &NoemaPaths) -> Self {
        Self::new(paths.errors_log_path())
    }

    #[cfg(test)]
    fn path(&self) -> &Path {
        &self.path
    }

    fn append(&self, event: SystemErrorEvent) -> Result<(), SystemErrorWriteError> {
        self.append_with_limits(event, MAX_LOG_BYTES, MAX_EVENT_BYTES)
    }

    fn append_with_limits(
        &self,
        event: SystemErrorEvent,
        max_log_bytes: usize,
        max_event_bytes: usize,
    ) -> Result<(), SystemErrorWriteError> {
        let _guard = ERROR_LOG_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(parent) = self.path.parent() {
            ensure_private_dir(parent).map_err(SystemErrorWriteError::CreateDirectory)?;
        }
        let line = serde_json::to_string(&event).map_err(SystemErrorWriteError::Serialize)?;
        let write_bytes = line.len().saturating_add(1);
        if write_bytes > max_event_bytes {
            return Err(SystemErrorWriteError::EventTooLarge {
                actual: write_bytes,
                maximum: max_event_bytes,
            });
        }
        self.rotate_if_needed(write_bytes, max_log_bytes)?;
        let mut options = OpenOptions::new();
        options.create(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&self.path)
            .map_err(SystemErrorWriteError::Open)?;
        ensure_private_file(&self.path).map_err(SystemErrorWriteError::SetPermissions)?;
        writeln!(file, "{line}").map_err(SystemErrorWriteError::Write)
    }

    fn rotate_if_needed(
        &self,
        write_bytes: usize,
        max_log_bytes: usize,
    ) -> Result<(), SystemErrorWriteError> {
        let metadata = match fs::metadata(&self.path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(SystemErrorWriteError::Metadata(error)),
        };
        if !metadata.is_file() {
            return Ok(());
        }
        if metadata.len() > max_log_bytes as u64 {
            return fs::remove_file(&self.path).map_err(SystemErrorWriteError::RemoveOversizedLog);
        }
        if metadata.len().saturating_add(write_bytes as u64) <= max_log_bytes as u64 {
            return Ok(());
        }

        let mut backup_name = OsString::from(self.path.as_os_str());
        backup_name.push(".1");
        let backup_path = PathBuf::from(backup_name);
        match fs::remove_file(&backup_path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(SystemErrorWriteError::RemoveBackup(error)),
        }
        fs::rename(&self.path, &backup_path).map_err(SystemErrorWriteError::Rotate)?;
        ensure_private_file(&backup_path).map_err(SystemErrorWriteError::SetPermissions)
    }

    /// Best-effort append that never panics or masks the caller's original error.
    pub fn try_append(&self, event: SystemErrorEvent) {
        let _ = self.append(event);
    }
}

/// One JSONL developer diagnostic event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemErrorEvent {
    /// UTC RFC3339 timestamp at construction time.
    pub timestamp: String,
    /// Severity label. The current sink writes only `error`.
    pub severity: &'static str,
    /// Stable machine-readable category owned by the emitting subsystem.
    pub category: &'static str,
    /// Concise human-readable message.
    pub message: String,
    /// Subsystem-specific context.
    pub context: Value,
    /// Ordered error strings where source errors are available.
    pub error_chain: Vec<String>,
    /// Raw diagnostic data, replaced by size metadata when it exceeds the sink limit.
    pub raw: Value,
}

impl SystemErrorEvent {
    /// Construct an error event with empty context, error chain, and raw payload.
    #[must_use]
    pub fn new(category: &'static str, message: impl Into<String>) -> Self {
        Self {
            timestamp: OffsetDateTime::now_utc()
                .format(&Rfc3339)
                .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string()),
            severity: "error",
            category,
            message: message.into(),
            context: json!({}),
            error_chain: Vec::new(),
            raw: json!({}),
        }
    }

    /// Attach structured context.
    #[must_use]
    pub fn with_context(mut self, context: Value) -> Self {
        self.context = context;
        self
    }

    /// Attach error-chain strings.
    #[must_use]
    pub fn with_error_chain<I>(mut self, error_chain: I) -> Self
    where
        I: IntoIterator<Item = String>,
    {
        self.error_chain = error_chain.into_iter().collect();
        self
    }

    /// Attach raw diagnostic data within the sink limit.
    #[must_use]
    pub fn with_raw(mut self, raw: Value) -> Self {
        let serialized_bytes = serde_json::to_vec(&raw).map_or(usize::MAX, |bytes| bytes.len());
        self.raw = if serialized_bytes <= MAX_RAW_BYTES {
            raw
        } else {
            json!({
                "truncated": true,
                "original_bytes": serialized_bytes,
            })
        };
        self
    }
}

#[derive(Debug, Error)]
enum SystemErrorWriteError {
    #[error("failed to create system error log directory: {0}")]
    CreateDirectory(#[source] std::io::Error),
    #[error("failed to open system error log: {0}")]
    Open(#[source] std::io::Error),
    #[error("failed to inspect system error log: {0}")]
    Metadata(#[source] std::io::Error),
    #[error("failed to remove oversized system error log: {0}")]
    RemoveOversizedLog(#[source] std::io::Error),
    #[error("failed to remove rotated system error log: {0}")]
    RemoveBackup(#[source] std::io::Error),
    #[error("failed to rotate system error log: {0}")]
    Rotate(#[source] std::io::Error),
    #[error("failed to set system error log permissions: {0}")]
    SetPermissions(#[source] std::io::Error),
    #[error("failed to serialize system error event: {0}")]
    Serialize(#[source] serde_json::Error),
    #[error("system error event is {actual} bytes; maximum is {maximum}")]
    EventTooLarge { actual: usize, maximum: usize },
    #[error("failed to write system error event: {0}")]
    Write(#[source] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    fn read_events(path: &Path) -> Vec<Value> {
        fs::read_to_string(path)
            .expect("error log")
            .lines()
            .map(|line| serde_json::from_str(line).expect("event"))
            .collect()
    }

    #[test]
    fn appends_jsonl_events_without_overwriting() {
        let dir = tempfile::tempdir().expect("temp dir");
        let logger = SystemErrorLogger::new(dir.path().join("errors.log"));

        logger
            .append(
                SystemErrorEvent::new("first_failure", "first failure")
                    .with_context(json!({"provider_kind": "test"}))
                    .with_raw(json!({"provider_text": "line one\nline two"})),
            )
            .expect("first append");
        logger
            .append(
                SystemErrorEvent::new("second_failure", "second failure")
                    .with_error_chain(["outer".to_string(), "inner".to_string()]),
            )
            .expect("second append");

        let events = read_events(logger.path());
        assert_eq!(events.len(), 2);
        assert_eq!(events[0]["category"], "first_failure");
        assert_eq!(events[0]["severity"], "error");
        assert_eq!(events[0]["context"]["provider_kind"], "test");
        assert_eq!(events[0]["raw"]["provider_text"], "line one\nline two");
        assert_eq!(events[1]["category"], "second_failure");
        assert_eq!(events[1]["error_chain"], json!(["outer", "inner"]));
        #[cfg(unix)]
        assert_eq!(
            fs::metadata(logger.path())
                .expect("error log metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );

        SystemErrorLogger::new(dir.path()).try_append(SystemErrorEvent::new(
            "expected_test_failure",
            "cannot append to a directory",
        ));
    }

    #[test]
    fn rotates_one_bounded_backup() {
        let dir = tempfile::tempdir().expect("temp dir");
        let logger = SystemErrorLogger::new(dir.path().join("errors.log"));
        for sequence in 0..20 {
            logger
                .append_with_limits(
                    SystemErrorEvent::new("bounded_failure", "x".repeat(80))
                        .with_context(json!({"sequence": sequence})),
                    512,
                    256,
                )
                .expect("bounded append");
        }

        let current = fs::metadata(logger.path()).expect("current log");
        let backup = fs::metadata(dir.path().join("errors.log.1")).expect("rotated log");
        assert!(current.len() <= 512);
        assert!(backup.len() <= 512);
    }

    #[test]
    fn replaces_oversized_raw_data_and_preserves_small_raw_data() {
        let small = SystemErrorEvent::new("small_failure", "small")
            .with_raw(json!({"ordinary_value": "preserved"}));
        assert_eq!(small.raw["ordinary_value"], "preserved");

        let large = SystemErrorEvent::new("large_failure", "large")
            .with_raw(json!({"content": "x".repeat(MAX_RAW_BYTES)}));
        assert_eq!(large.raw["truncated"], true);
        assert!(large.raw["original_bytes"].as_u64().expect("byte count") > MAX_RAW_BYTES as u64);
    }

    #[test]
    fn rejects_an_event_larger_than_the_file_limit() {
        let dir = tempfile::tempdir().expect("temp dir");
        let logger = SystemErrorLogger::new(dir.path().join("errors.log"));
        let result = logger.append_with_limits(
            SystemErrorEvent::new("large_failure", "x".repeat(512)),
            512,
            256,
        );

        assert!(matches!(
            result,
            Err(SystemErrorWriteError::EventTooLarge { .. })
        ));
        assert!(!logger.path().exists());
    }
}
