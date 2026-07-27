//! Filesystem-authoritative polling schedules, leases, and checkpoints.

#[cfg(test)]
mod tests;

use crate::{
    CursorHandle,
    private_fs::{
        PrivateFsError, create_private_dir, random_hex, read_bounded_regular_file,
        require_exact_entries, require_regular_directory, sync_directory, write_new_file,
    },
};
use noema_home::NoemaPaths;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
use thiserror::Error;

const SCHEDULES_DIR: &str = "schedules";
const SCHEDULE_FILE: &str = "schedule.json";
const CHECKPOINT_FILE: &str = "checkpoint.json";
const MAX_SCHEDULE_BYTES: u64 = 64 * 1024;
const MAX_CHECKPOINT_BYTES: u64 = 64 * 1024;
const MAX_SCHEDULES: usize = 1_024;
const MAX_INTERVAL_SECONDS: u64 = 7 * 24 * 60 * 60;
const MAX_LEASE_SECONDS: u64 = 15 * 60;
const MAX_RETRY_ATTEMPTS: u32 = 8;
const MAX_EVENT_KEY_BYTES: usize = 256;

/// Filesystem-canonical desired state for one polling workflow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PollSchedule {
    /// Stable local schedule identity.
    pub schedule_id: String,
    /// Exact connection identity used by the operation.
    pub connection_id: String,
    /// Exact definition semantic digest.
    pub semantic_digest: String,
    /// Exact operation identity.
    pub operation_id: String,
    /// Reviewed account surface.
    pub account_kind: String,
    /// Minimum interval between successful polls.
    pub interval_seconds: u64,
    /// Whether the host should continue scheduling the workflow.
    pub enabled: bool,
    /// Monotonic schedule revision.
    pub revision: u64,
    /// Earliest time at which the next poll may start.
    pub next_attempt_epoch_seconds: u64,
    /// Consecutive retry count for the current attempt series.
    pub retry_attempt: u32,
    /// Current process lease, if a worker owns the schedule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lease: Option<ScheduleLease>,
}

/// Fenced process claim persisted with a schedule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScheduleLease {
    /// Opaque process owner identity, never a provider credential.
    pub owner: String,
    /// Schedule revision captured when the lease was acquired.
    pub revision: u64,
    /// Lease expiry in the same bounded epoch-second clock as the schedule.
    pub expires_at_epoch_seconds: u64,
}

/// Secret-free durable checkpoint for one polling workflow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct PollCheckpoint {
    /// Opaque cursor handle; its token remains in a private secret store.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<CursorHandle>,
    /// Last provider event key used for local duplicate suppression.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_event_key: Option<String>,
    /// Whether a bounded baseline must run before the cursor is used again.
    #[serde(default)]
    pub full_resync_required: bool,
    /// Monotonic checkpoint revision.
    pub revision: u64,
}

/// Rebuildable projection of a filesystem-owned schedule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleProjection {
    /// Stable schedule identity.
    pub schedule_id: String,
    /// Exact connection identity.
    pub connection_id: String,
    /// Exact semantic digest.
    pub semantic_digest: String,
    /// Exact operation identity.
    pub operation_id: String,
    /// Whether the schedule is enabled.
    pub enabled: bool,
    /// Safe lifecycle status.
    pub status: &'static str,
    /// Schedule revision.
    pub revision: u64,
    /// Checkpoint revision.
    pub checkpoint_revision: u64,
    /// Last event key, never event content.
    pub last_event_key: Option<String>,
    /// Whether a baseline is required.
    pub full_resync_required: bool,
}

/// One valid schedule and its secret-free checkpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleInstall {
    /// Canonical schedule data.
    pub schedule: PollSchedule,
    /// Canonical checkpoint data.
    pub checkpoint: PollCheckpoint,
    /// Rebuildable SQLite projection.
    pub projection: ScheduleProjection,
}

/// A worker lease returned after an exact claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleClaim {
    /// Stable schedule identity.
    pub schedule_id: String,
    /// Schedule revision fenced by this claim.
    pub revision: u64,
    /// Exact worker owner.
    pub owner: String,
    /// Lease expiry.
    pub expires_at_epoch_seconds: u64,
}

/// Bounded retry policy for a polling workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PollRetryPolicy {
    /// Initial delay after a retryable failure.
    pub initial_delay_seconds: u64,
    /// Maximum delay after exponential growth.
    pub max_delay_seconds: u64,
    /// Maximum consecutive attempts.
    pub max_attempts: u32,
}

impl PollRetryPolicy {
    /// Calculate a bounded exponential delay for an attempt number.
    #[must_use]
    pub fn delay_seconds(self, attempt: u32) -> u64 {
        let attempt = attempt.min(self.max_attempts.saturating_sub(1));
        self.initial_delay_seconds
            .saturating_mul(1_u64.checked_shl(attempt).unwrap_or(u64::MAX))
            .min(self.max_delay_seconds)
    }

    /// Validate the retry policy before it can be stored in a definition.
    ///
    /// # Errors
    ///
    /// Returns ScheduleError::Invalid when delays or attempts are zero,
    /// contradictory, or exceed the bounded host policy.
    pub fn validate(self) -> Result<(), ScheduleError> {
        if self.initial_delay_seconds == 0
            || self.max_delay_seconds < self.initial_delay_seconds
            || self.max_delay_seconds > MAX_INTERVAL_SECONDS
            || self.max_attempts == 0
            || self.max_attempts > MAX_RETRY_ATTEMPTS
        {
            return Err(ScheduleError::Invalid("retry_policy"));
        }
        Ok(())
    }
}

/// Filesystem schedule-store failure with no secret-bearing fields.
#[derive(Debug, Error)]
pub enum ScheduleError {
    /// Filesystem operation failed.
    #[error("adapter schedule filesystem operation failed: {0}")]
    Io(#[from] std::io::Error),
    /// Private file invariant failed.
    #[error("adapter schedule filesystem invariant failed: {0}")]
    Integrity(&'static str),
    /// Canonical JSON is malformed or oversized.
    #[error("adapter schedule JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),
    /// Schedule data is outside the closed vocabulary.
    #[error("adapter schedule is invalid: {0}")]
    Invalid(&'static str),
    /// The schedule cannot be claimed by this worker.
    #[error("adapter schedule lease is fenced")]
    LeaseFenced,
    /// The schedule is disabled or not due yet.
    #[error("adapter schedule is not due")]
    NotDue,
}

impl From<PrivateFsError> for ScheduleError {
    fn from(error: PrivateFsError) -> Self {
        match error {
            PrivateFsError::Io(error) => Self::Io(error),
            PrivateFsError::Integrity(code) => Self::Integrity(code),
        }
    }
}

/// Filesystem owner for desired polling state and checkpoints.
#[derive(Debug, Clone)]
pub struct ScheduleStore {
    paths: NoemaPaths,
}

impl ScheduleStore {
    /// Create a schedule store under one Noema home.
    #[must_use]
    pub const fn new(paths: NoemaPaths) -> Self {
        Self { paths }
    }

    /// Install one schedule and an initial empty checkpoint atomically.
    ///
    /// # Errors
    ///
    /// Returns ScheduleError when identifiers, timing, or immutable
    /// filesystem state is invalid.
    pub fn install(
        &self,
        schedule: &PollSchedule,
        checkpoint: &PollCheckpoint,
    ) -> Result<ScheduleInstall, ScheduleError> {
        validate_schedule(schedule)?;
        validate_checkpoint(checkpoint)?;
        let root = self.prepare_root()?;
        let target = root.join(&schedule.schedule_id);
        if target.exists() {
            let existing = Self::read_install(&target, &schedule.schedule_id)?;
            if existing.schedule != *schedule || existing.checkpoint != *checkpoint {
                return Err(ScheduleError::Integrity("schedule_conflict"));
            }
            return Ok(existing);
        }
        let staging = root.join(format!(".staging-{}", random_hex(12)?));
        create_private_dir(&staging)?;
        let result = (|| {
            write_new_file(&staging.join(SCHEDULE_FILE), &json_bytes(schedule)?)?;
            write_new_file(&staging.join(CHECKPOINT_FILE), &json_bytes(checkpoint)?)?;
            sync_directory(&staging)?;
            fs::rename(&staging, &target)?;
            sync_directory(&root)?;
            Self::read_install(&target, &schedule.schedule_id)
        })();
        if staging.exists() {
            let _ = fs::remove_dir_all(staging);
        }
        result
    }

    /// Scan all valid schedules.
    ///
    /// # Errors
    ///
    /// Returns ScheduleError when the authoritative schedule root or an
    /// object violates its bounded shape.
    pub fn scan(&self) -> Result<Vec<ScheduleInstall>, ScheduleError> {
        let root = self.prepare_root()?;
        let mut entries = fs::read_dir(&root)?.collect::<Result<Vec<_>, _>>()?;
        if entries.len() > MAX_SCHEDULES + 1 {
            return Err(ScheduleError::Integrity("schedules_oversized"));
        }
        entries.sort_by_key(fs::DirEntry::file_name);
        let mut installs = Vec::new();
        for entry in entries {
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                return Err(ScheduleError::Integrity("schedule_name"));
            };
            if name.starts_with('.') {
                continue;
            }
            installs.push(Self::read_install(&entry.path(), name)?);
        }
        Ok(installs)
    }

    /// Claim a due schedule with a bounded, persisted lease.
    ///
    /// # Errors
    ///
    /// Returns ScheduleError::NotDue for disabled/not-yet-due work,
    /// ScheduleError::LeaseFenced for another live owner, or an invariant
    /// error for malformed state.
    pub fn claim(
        &self,
        schedule_id: &str,
        owner: &str,
        now_epoch_seconds: u64,
        lease_seconds: u64,
    ) -> Result<ScheduleClaim, ScheduleError> {
        if !valid_key(schedule_id)
            || !valid_key(owner)
            || lease_seconds == 0
            || lease_seconds > MAX_LEASE_SECONDS
        {
            return Err(ScheduleError::Invalid("claim"));
        }
        let root = self.prepare_root()?;
        let directory = root.join(schedule_id);
        let mut install = Self::read_install(&directory, schedule_id)?;
        if !install.schedule.enabled
            || install.schedule.next_attempt_epoch_seconds > now_epoch_seconds
        {
            return Err(ScheduleError::NotDue);
        }
        if install
            .schedule
            .lease
            .as_ref()
            .is_some_and(|lease| lease.expires_at_epoch_seconds > now_epoch_seconds)
        {
            return Err(ScheduleError::LeaseFenced);
        }
        let revision = install.schedule.revision.saturating_add(1);
        let expires_at_epoch_seconds = now_epoch_seconds.saturating_add(lease_seconds);
        install.schedule.revision = revision;
        install.schedule.lease = Some(ScheduleLease {
            owner: owner.to_string(),
            revision,
            expires_at_epoch_seconds,
        });
        atomic_replace(
            &directory.join(SCHEDULE_FILE),
            &json_bytes(&install.schedule)?,
        )?;
        Ok(ScheduleClaim {
            schedule_id: schedule_id.to_string(),
            revision,
            owner: owner.to_string(),
            expires_at_epoch_seconds,
        })
    }

    /// Commit a checkpoint before releasing its schedule lease.
    ///
    /// # Errors
    ///
    /// Returns ScheduleError::LeaseFenced when the claim is stale, or an
    /// invariant error when the checkpoint contains secret-bearing data.
    pub fn commit(
        &self,
        claim: &ScheduleClaim,
        checkpoint: &PollCheckpoint,
        next_attempt_epoch_seconds: u64,
        retry_attempt: u32,
    ) -> Result<ScheduleInstall, ScheduleError> {
        validate_checkpoint(checkpoint)?;
        if retry_attempt > MAX_RETRY_ATTEMPTS {
            return Err(ScheduleError::Invalid("retry_attempt"));
        }
        let root = self.prepare_root()?;
        let directory = root.join(&claim.schedule_id);
        let mut install = Self::read_install(&directory, &claim.schedule_id)?;
        let Some(lease) = install.schedule.lease.as_ref() else {
            return Err(ScheduleError::LeaseFenced);
        };
        if lease.owner != claim.owner
            || lease.revision != claim.revision
            || install.schedule.revision != claim.revision
        {
            return Err(ScheduleError::LeaseFenced);
        }
        // The checkpoint is the durable source of truth. A crash before the
        // schedule rewrite leaves the lease recoverable without losing state.
        atomic_replace(&directory.join(CHECKPOINT_FILE), &json_bytes(checkpoint)?)?;
        install.schedule.next_attempt_epoch_seconds = next_attempt_epoch_seconds;
        install.schedule.retry_attempt = retry_attempt;
        install.schedule.lease = None;
        install.schedule.revision = install.schedule.revision.saturating_add(1);
        atomic_replace(
            &directory.join(SCHEDULE_FILE),
            &json_bytes(&install.schedule)?,
        )?;
        Self::read_install(&directory, &claim.schedule_id)
    }

    /// Disable a schedule and fence any in-flight worker claim.
    ///
    /// # Errors
    ///
    /// Returns ScheduleError when the schedule is absent or its canonical
    /// file cannot be replaced safely.
    pub fn revoke(&self, schedule_id: &str) -> Result<ScheduleInstall, ScheduleError> {
        if !valid_key(schedule_id) {
            return Err(ScheduleError::Invalid("schedule_id"));
        }
        let root = self.prepare_root()?;
        let directory = root.join(schedule_id);
        let mut install = Self::read_install(&directory, schedule_id)?;
        install.schedule.enabled = false;
        install.schedule.lease = None;
        install.schedule.revision = install.schedule.revision.saturating_add(1);
        atomic_replace(
            &directory.join(SCHEDULE_FILE),
            &json_bytes(&install.schedule)?,
        )?;
        Self::read_install(&directory, schedule_id)
    }

    fn prepare_root(&self) -> Result<PathBuf, ScheduleError> {
        let adapters = self.paths.adapters_dir();
        create_private_dir(&adapters)?;
        let root = adapters.join(SCHEDULES_DIR);
        create_private_dir(&root)?;
        Ok(root)
    }

    fn read_install(directory: &Path, expected_id: &str) -> Result<ScheduleInstall, ScheduleError> {
        require_regular_directory(directory)?;
        require_exact_entries(directory, &[SCHEDULE_FILE, CHECKPOINT_FILE])?;
        let schedule_bytes =
            read_bounded_regular_file(&directory.join(SCHEDULE_FILE), MAX_SCHEDULE_BYTES)?;
        let checkpoint_bytes =
            read_bounded_regular_file(&directory.join(CHECKPOINT_FILE), MAX_CHECKPOINT_BYTES)?;
        let schedule = serde_json::from_slice::<PollSchedule>(&schedule_bytes)?;
        let checkpoint = serde_json::from_slice::<PollCheckpoint>(&checkpoint_bytes)?;
        if schedule.schedule_id != expected_id {
            return Err(ScheduleError::Integrity("schedule_identity"));
        }
        validate_schedule(&schedule)?;
        validate_checkpoint(&checkpoint)?;
        Ok(ScheduleInstall {
            projection: projection(&schedule, &checkpoint),
            schedule,
            checkpoint,
        })
    }
}

fn projection(schedule: &PollSchedule, checkpoint: &PollCheckpoint) -> ScheduleProjection {
    ScheduleProjection {
        schedule_id: schedule.schedule_id.clone(),
        connection_id: schedule.connection_id.clone(),
        semantic_digest: schedule.semantic_digest.clone(),
        operation_id: schedule.operation_id.clone(),
        enabled: schedule.enabled,
        status: if checkpoint.full_resync_required {
            "full_resync_required"
        } else if schedule.lease.is_some() {
            "claimed"
        } else if schedule.enabled {
            "ready"
        } else {
            "revoked"
        },
        revision: schedule.revision,
        checkpoint_revision: checkpoint.revision,
        last_event_key: checkpoint.last_event_key.clone(),
        full_resync_required: checkpoint.full_resync_required,
    }
}

fn validate_schedule(schedule: &PollSchedule) -> Result<(), ScheduleError> {
    for value in [
        schedule.schedule_id.as_str(),
        schedule.connection_id.as_str(),
        schedule.semantic_digest.as_str(),
        schedule.operation_id.as_str(),
        schedule.account_kind.as_str(),
    ] {
        if !valid_key(value) {
            return Err(ScheduleError::Invalid("schedule_identity"));
        }
    }
    if schedule.interval_seconds == 0 || schedule.interval_seconds > MAX_INTERVAL_SECONDS {
        return Err(ScheduleError::Invalid("interval"));
    }
    if schedule.retry_attempt > MAX_RETRY_ATTEMPTS {
        return Err(ScheduleError::Invalid("retry_attempt"));
    }
    if let Some(lease) = &schedule.lease
        && (!valid_key(&lease.owner) || lease.revision != schedule.revision)
    {
        return Err(ScheduleError::Invalid("lease"));
    }
    Ok(())
}

fn validate_checkpoint(checkpoint: &PollCheckpoint) -> Result<(), ScheduleError> {
    if checkpoint
        .last_event_key
        .as_ref()
        .is_some_and(|key| key.is_empty() || key.len() > MAX_EVENT_KEY_BYTES || !valid_key(key))
    {
        return Err(ScheduleError::Invalid("event_key"));
    }
    if checkpoint
        .cursor
        .as_ref()
        .is_some_and(|cursor| !valid_key(&cursor.secret_reference))
    {
        return Err(ScheduleError::Invalid("cursor_reference"));
    }
    Ok(())
}

fn valid_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':'))
}

fn json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, ScheduleError> {
    let bytes = serde_json::to_vec(value)?;
    if bytes.len() as u64 > MAX_SCHEDULE_BYTES {
        return Err(ScheduleError::Integrity("schedule_oversized"));
    }
    Ok(bytes)
}

fn atomic_replace(path: &Path, bytes: &[u8]) -> Result<(), ScheduleError> {
    let parent = path
        .parent()
        .ok_or(ScheduleError::Integrity("schedule_parent"))?;
    let temp = parent.join(format!(".replace-{}", random_hex(12)?));
    write_new_file(&temp, bytes)?;
    fs::rename(&temp, path)?;
    sync_directory(parent)?;
    Ok(())
}
