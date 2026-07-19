use std::fmt;

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use noema_tasks::TaskId;
use noema_workspaces::ProjectId;
use thiserror::Error;
use time::{PrimitiveDateTime, macros::format_description};

/// Safe cursor validation failure exposed to API adapters as `invalid_cursor`.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("invalid work cursor")]
pub struct WorkCursorError;

impl WorkCursorError {
    /// Return the stable API error code for malformed cursors.
    /// Return the validated page size as a query limit.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        "invalid_cursor"
    }
}

/// Validated connection page size. The default is 50 and the hard maximum is
/// 100 for every Work connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct WorkPageSize(u8);

impl WorkPageSize {
    /// Default page size for bounded Work connections.
    pub const DEFAULT: Self = Self(50);
    /// Default page size for detail/history views.
    pub const DETAIL_DEFAULT: Self = Self(20);
    /// Maximum page size accepted by any Work connection.
    pub const MAX: u8 = 100;

    /// Validate and construct a bounded page size.
    ///
    /// # Errors
    ///
    /// Returns [`WorkCursorError`] when `value` is zero or exceeds [`Self::MAX`].
    pub fn new(value: u32) -> Result<Self, WorkCursorError> {
        let value = u8::try_from(value).map_err(|_| WorkCursorError)?;
        if value == 0 || value > Self::MAX {
            Err(WorkCursorError)
        } else {
            Ok(Self(value))
        }
    }

    #[must_use]
    /// Return this validated page size as a collection/SQL limit.
    pub const fn get(self) -> usize {
        self.0 as usize
    }
}

impl Default for WorkPageSize {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Positive global Work-event sequence cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorkEventCursor(u64);

impl WorkEventCursor {
    /// Validate a positive SQLite-compatible global event sequence.
    ///
    /// # Errors
    ///
    /// Returns [`WorkCursorError`] when the sequence is zero or exceeds SQLite's range.
    pub fn new(sequence: u64) -> Result<Self, WorkCursorError> {
        if sequence == 0 || sequence > i64::MAX as u64 {
            Err(WorkCursorError)
        } else {
            Ok(Self(sequence))
        }
    }

    /// Decode an exact unpadded base64url event cursor.
    ///
    /// # Errors
    ///
    /// Returns [`WorkCursorError`] for malformed, noncanonical, or out-of-range input.
    pub fn decode(encoded: &str) -> Result<Self, WorkCursorError> {
        let bytes = decode_base64url(encoded)?;
        let payload = std::str::from_utf8(&bytes).map_err(|_| WorkCursorError)?;
        let sequence = payload
            .strip_prefix("work-event:v1:")
            .ok_or(WorkCursorError)?;
        Self::new(canonical_sequence(sequence)?)
    }

    /// Return the global event sequence represented by this cursor.
    #[must_use]
    pub const fn sequence(self) -> u64 {
        self.0
    }

    /// Encode this cursor as the canonical unpadded base64url value.
    #[must_use]
    pub fn encode(self) -> String {
        encode_base64url(format!("work-event:v1:{}", self.0).as_bytes())
    }
}

impl fmt::Display for WorkEventCursor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.encode())
    }
}

/// Keyset cursor for active/all or terminal task connections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkTaskCursor {
    /// Cursor for an active task ordered by update timestamp and id.
    Active {
        /// Query fingerprint that makes the cursor filter-specific.
        query_hash: String,
        /// Last task update timestamp in the page.
        updated_at: String,
        /// Last task identity in the page.
        task_id: TaskId,
    },
    /// Cursor for terminal history ordered by terminal timestamp and id.
    Terminal {
        /// Query fingerprint that makes the cursor filter-specific.
        query_hash: String,
        /// Last terminal timestamp in the page.
        terminal_at: String,
        /// Last task identity in the page.
        task_id: TaskId,
    },
}

impl WorkTaskCursor {
    pub(crate) fn active(
        query_hash: String,
        updated_at: String,
        task_id: TaskId,
    ) -> Result<Self, WorkCursorError> {
        Ok(Self::Active {
            query_hash: canonical_hash(&query_hash)?,
            updated_at: canonical_sqlite_timestamp(&updated_at)?,
            task_id,
        })
    }

    pub(crate) fn terminal(
        query_hash: String,
        terminal_at: String,
        task_id: TaskId,
    ) -> Result<Self, WorkCursorError> {
        Ok(Self::Terminal {
            query_hash: canonical_hash(&query_hash)?,
            terminal_at: canonical_sqlite_timestamp(&terminal_at)?,
            task_id,
        })
    }

    /// Decode and validate a task connection cursor.
    ///
    /// # Errors
    ///
    /// Returns [`WorkCursorError`] for malformed input or an unknown cursor family.
    pub fn decode(encoded: &str) -> Result<Self, WorkCursorError> {
        let [family, query_hash, timestamp, id] = decode_nul_fields(encoded)?;
        let query_hash = canonical_hash(&query_hash)?;
        let timestamp = canonical_sqlite_timestamp(&timestamp)?;
        let task_id = TaskId::new(canonical_text(&id)?).map_err(|_| WorkCursorError)?;
        match family.as_str() {
            "work-task-active:v1" => Ok(Self::Active {
                query_hash,
                updated_at: timestamp,
                task_id,
            }),
            "work-task-terminal:v1" => Ok(Self::Terminal {
                query_hash,
                terminal_at: timestamp,
                task_id,
            }),
            _ => Err(WorkCursorError),
        }
    }

    /// Encode this task cursor as the canonical opaque value.
    #[must_use]
    pub fn encode(&self) -> String {
        match self {
            Self::Active {
                query_hash,
                updated_at,
                task_id,
            } => encode_nul_fields(&[
                "work-task-active:v1",
                query_hash,
                updated_at,
                task_id.as_str(),
            ]),
            Self::Terminal {
                query_hash,
                terminal_at,
                task_id,
            } => encode_nul_fields(&[
                "work-task-terminal:v1",
                query_hash,
                terminal_at,
                task_id.as_str(),
            ]),
        }
    }
}

/// Keyset cursor for project connections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectCursor {
    pub(crate) query_hash: String,
    pub(crate) updated_at: String,
    pub(crate) project_id: ProjectId,
}

impl ProjectCursor {
    /// Validate and construct a project connection cursor.
    ///
    /// # Errors
    ///
    /// Returns [`WorkCursorError`] when the query hash or timestamp is noncanonical.
    pub fn new(
        query_hash: String,
        updated_at: String,
        project_id: ProjectId,
    ) -> Result<Self, WorkCursorError> {
        Ok(Self {
            query_hash: canonical_hash(&query_hash)?,
            updated_at: canonical_sqlite_timestamp(&updated_at)?,
            project_id,
        })
    }

    /// Decode and validate a project connection cursor.
    ///
    /// # Errors
    ///
    /// Returns [`WorkCursorError`] for malformed input or invalid project identity.
    pub fn decode(encoded: &str) -> Result<Self, WorkCursorError> {
        let [family, query_hash, updated_at, project_id] = decode_nul_fields(encoded)?;
        if family != "work-project:v1" {
            return Err(WorkCursorError);
        }
        Self::new(
            query_hash,
            updated_at,
            ProjectId::new(project_id).map_err(|_| WorkCursorError)?,
        )
    }

    /// Encode this project cursor as the canonical opaque value.
    #[must_use]
    pub fn encode(&self) -> String {
        encode_nul_fields(&[
            "work-project:v1",
            &self.query_hash,
            &self.updated_at,
            self.project_id.as_str(),
        ])
    }
}

pub(super) fn encode_nul_fields(fields: &[&str]) -> String {
    let mut payload = Vec::new();
    for (index, field) in fields.iter().enumerate() {
        if index > 0 {
            payload.push(0);
        }
        payload.extend_from_slice(field.as_bytes());
    }
    encode_base64url(&payload)
}

pub(super) fn decode_nul_fields<const N: usize>(
    encoded: &str,
) -> Result<[String; N], WorkCursorError> {
    let bytes = decode_base64url(encoded)?;
    bytes
        .split(|byte| *byte == 0)
        .map(|field| utf8(field).map(str::to_string))
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| WorkCursorError)
}

pub(super) fn canonical_sequence(value: &str) -> Result<u64, WorkCursorError> {
    let value = canonical_text(value)?;
    if (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(WorkCursorError);
    }
    let value = value.parse::<u64>().map_err(|_| WorkCursorError)?;
    (value > 0 && value <= i64::MAX as u64)
        .then_some(value)
        .ok_or(WorkCursorError)
}

pub(super) fn canonical_hash(value: &str) -> Result<String, WorkCursorError> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(value.to_string())
    } else {
        Err(WorkCursorError)
    }
}

pub(super) fn canonical_text(value: &str) -> Result<String, WorkCursorError> {
    if value.is_empty() || value.chars().any(char::is_control) {
        Err(WorkCursorError)
    } else {
        Ok(value.to_string())
    }
}

pub(super) fn canonical_sqlite_timestamp(value: &str) -> Result<String, WorkCursorError> {
    const SQLITE_TIMESTAMP: &[time::format_description::FormatItem<'static>] =
        format_description!("[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z");
    let parsed = PrimitiveDateTime::parse(value, SQLITE_TIMESTAMP).map_err(|_| WorkCursorError)?;
    let canonical = parsed
        .format(SQLITE_TIMESTAMP)
        .map_err(|_| WorkCursorError)?;
    if canonical == value {
        Ok(canonical)
    } else {
        Err(WorkCursorError)
    }
}

fn utf8(bytes: &[u8]) -> Result<&str, WorkCursorError> {
    std::str::from_utf8(bytes).map_err(|_| WorkCursorError)
}

fn encode_base64url(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

fn decode_base64url(value: &str) -> Result<Vec<u8>, WorkCursorError> {
    if value.is_empty() || value.contains('=') {
        return Err(WorkCursorError);
    }
    let output = URL_SAFE_NO_PAD.decode(value).map_err(|_| WorkCursorError)?;
    if encode_base64url(&output) == value {
        Ok(output)
    } else {
        Err(WorkCursorError)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_cursor_is_exact_unpadded_base64url_and_fail_closed() {
        let cursor = WorkEventCursor::new(42).unwrap();
        assert_eq!(cursor.encode(), "d29yay1ldmVudDp2MTo0Mg");
        assert_eq!(WorkEventCursor::decode(&cursor.encode()), Ok(cursor));
        for invalid in [
            "",
            "d29yay1ldmVudDp2MTow",
            "d29yay1ldmVudDp2MTowMQ",
            "d29yay1ldmVudDp2MTo0Mg==",
            "bm90LXRoZS1mYW1pbHk",
        ] {
            assert!(WorkEventCursor::decode(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn page_size_defaults_to_fifty_and_caps_at_one_hundred() {
        assert_eq!(WorkPageSize::default().get(), 50);
        assert_eq!(WorkPageSize::new(100).unwrap().get(), 100);
        assert!(WorkPageSize::new(0).is_err());
        assert!(WorkPageSize::new(101).is_err());
    }
}
