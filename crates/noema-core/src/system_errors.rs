//! Developer diagnostic system error logging.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
use thiserror::Error;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

/// Provider output or transport body was malformed.
pub const SYSTEM_ERROR_PROVIDER_MALFORMED_RESPONSE: &str = "provider_malformed_response";
/// MCP metadata or tool-call payload was malformed.
pub const SYSTEM_ERROR_MCP_MALFORMED_RESPONSE: &str = "mcp_malformed_response";
/// Runtime state reached an invariant violation.
pub const SYSTEM_ERROR_RUNTIME_INVARIANT: &str = "runtime_invariant_violation";
/// Store-backed state violated a closed Noema schema assumption.
pub const SYSTEM_ERROR_STORE_INVARIANT: &str = "store_invariant_violation";

/// Append-only developer diagnostic system error logger.
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
    pub fn from_paths(paths: &crate::NoemaPaths) -> Self {
        Self::new(paths.errors_log_path())
    }

    /// Return the target log path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append one diagnostic event and report filesystem/serialization errors.
    ///
    /// # Errors
    ///
    /// Returns [`SystemErrorWriteError`] when the parent directory cannot be
    /// created, the event cannot be serialized, or the log line cannot be
    /// appended.
    pub fn append(&self, event: SystemErrorEvent) -> Result<(), SystemErrorWriteError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(SystemErrorWriteError::CreateDirectory)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(SystemErrorWriteError::Open)?;
        let line = serde_json::to_string(&event).map_err(SystemErrorWriteError::Serialize)?;
        writeln!(file, "{line}").map_err(SystemErrorWriteError::Write)
    }

    /// Best-effort append that never panics or masks the caller's original error.
    pub fn try_append(&self, event: SystemErrorEvent) {
        let _ = self.append(event);
    }
}

/// One JSONL system error event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemErrorEvent {
    /// UTC RFC3339 timestamp at write construction time.
    pub timestamp: String,
    /// Severity label. The first slice writes only `error`.
    pub severity: &'static str,
    /// Stable machine-readable category.
    pub category: &'static str,
    /// Concise human-readable message.
    pub message: String,
    /// Subsystem-specific context.
    pub context: Value,
    /// Ordered error strings where source errors are available.
    pub error_chain: Vec<String>,
    /// Uncapped, unredacted raw payloads.
    pub raw: Value,
}

impl SystemErrorEvent {
    /// Construct a system error event with empty context, error chain, and raw payload.
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

    /// Attach uncapped raw diagnostic payloads.
    #[must_use]
    pub fn with_raw(mut self, raw: Value) -> Self {
        self.raw = raw;
        self
    }
}

/// Errors produced when writing developer diagnostics.
#[derive(Debug, Error)]
pub enum SystemErrorWriteError {
    /// Parent directory could not be created.
    #[error("failed to create system error log directory: {0}")]
    CreateDirectory(std::io::Error),
    /// Log file could not be opened.
    #[error("failed to open system error log: {0}")]
    Open(std::io::Error),
    /// Event could not be serialized.
    #[error("failed to serialize system error event: {0}")]
    Serialize(serde_json::Error),
    /// Log line could not be written.
    #[error("failed to write system error event: {0}")]
    Write(std::io::Error),
}

#[cfg(test)]
pub(crate) fn read_system_error_events(
    path: impl AsRef<Path>,
) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    let text = fs::read_to_string(path)?;
    text.lines()
        .map(|line| serde_json::from_str::<Value>(line).map_err(Into::into))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn appends_jsonl_events_without_overwriting() {
        let dir = tempfile::tempdir().expect("temp dir");
        let logger = SystemErrorLogger::new(dir.path().join("errors.log"));

        logger
            .append(
                SystemErrorEvent::new(SYSTEM_ERROR_PROVIDER_MALFORMED_RESPONSE, "first failure")
                    .with_context(json!({"provider_kind": "codex"}))
                    .with_raw(json!({"provider_text": "line one\nline two"})),
            )
            .expect("first append");
        logger
            .append(
                SystemErrorEvent::new(SYSTEM_ERROR_MCP_MALFORMED_RESPONSE, "second failure")
                    .with_context(json!({"mcp_server_id": "mcp:test"}))
                    .with_error_chain(["outer".to_string(), "inner".to_string()])
                    .with_raw(json!({"result": {"tools": "bad"}})),
            )
            .expect("second append");

        let events = read_system_error_events(dir.path().join("errors.log")).expect("events");
        assert_eq!(events.len(), 2);
        assert_eq!(
            events[0]["category"],
            SYSTEM_ERROR_PROVIDER_MALFORMED_RESPONSE
        );
        assert_eq!(events[0]["severity"], "error");
        assert_eq!(events[0]["context"]["provider_kind"], "codex");
        assert_eq!(events[0]["raw"]["provider_text"], "line one\nline two");
        assert_eq!(events[1]["category"], SYSTEM_ERROR_MCP_MALFORMED_RESPONSE);
        assert_eq!(events[1]["error_chain"], json!(["outer", "inner"]));
        assert_eq!(events[1]["raw"]["result"]["tools"], "bad");
    }

    #[test]
    fn try_append_swallows_write_failures() {
        let dir = tempfile::tempdir().expect("temp dir");
        let logger = SystemErrorLogger::new(dir.path());

        logger.try_append(SystemErrorEvent::new(
            SYSTEM_ERROR_RUNTIME_INVARIANT,
            "cannot append to a directory",
        ));
    }
}
