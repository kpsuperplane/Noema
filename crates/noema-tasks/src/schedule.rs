use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{TaskId, WorkDomainError, error::invalid_input, validation::required};

pub use crate::ids::TaskRecurrenceId;

string_enum! {
/// Recovery behavior when a scheduled instant elapsed while processing was unavailable.
#[derive(Default)]
pub enum MissedRunPolicy, "task.schedule.missed_run_policy" {
    /// Record the missed instant without executing it.
    Skip => "skip",
    /// Execute one representative occurrence immediately.
    #[default] RunOnce => "run_once",
}
/// Behavior when another occurrence in the same series is still nonterminal.
#[derive(Default)]
pub enum OverlapPolicy, "task.recurrence.overlap_policy" {
    /// Record the new instant without starting another task.
    #[default] Skip => "skip",
    /// Retain one coalesced instant until the active occurrence settles.
    QueueOne => "queue_one",
    /// Materialize every due occurrence.
    Allow => "allow",
}
/// Durable lifecycle of a recurring task template.
pub enum RecurrenceLifecycle, "task.recurrence.lifecycle" {
    Active => "active",
    Paused => "paused",
    Ended => "ended",
}
/// Immutable disposition of one recurrence slot.
pub enum RecurrenceOccurrenceResolution, "task.recurrence.occurrence_resolution" {
    Materialized => "materialized",
    Skipped => "skipped",
    Coalesced => "coalesced",
}
/// What caused a materialized recurrence occurrence to exist.
pub enum RecurrenceOccurrenceTrigger, "task.recurrence.occurrence_trigger" {
    /// An exact cron slot or its missed/overlap policy resolution.
    Scheduled => "scheduled",
    /// An explicit human or agent request outside the cron cadence.
    Manual => "manual",
}
/// Fenced lifecycle operation for recurring authority.
pub enum RecurrenceCommandKind, "task.recurrence.command" {
    Pause => "pause",
    Resume => "resume",
    SkipNext => "skip_next",
    End => "end",
}
}

/// Optional future execution attached directly to an Inbox task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewTaskSchedule {
    /// Exact UTC instant represented as Unix seconds.
    pub scheduled_for: i64,
    /// IANA timezone used to author and display wall-clock time.
    pub time_zone: String,
    /// Restart/missed-window policy.
    pub missed_run_policy: MissedRunPolicy,
    /// Repeating authority, when Repeat is enabled.
    pub recurrence: Option<NewTaskRecurrence>,
}

/// Repeating timing configuration copied from a task when Repeat is enabled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewTaskRecurrence {
    /// Inclusive lower bound for cron matches, as Unix seconds.
    pub starts_at: i64,
    /// Five-field cron expression without a timezone suffix.
    pub cron_expression: String,
    /// Overlap behavior for nonterminal occurrences.
    pub overlap_policy: OverlapPolicy,
}

impl NewTaskSchedule {
    /// Normalize the timezone/cron and ensure the supplied first instant is legal.
    ///
    /// # Errors
    /// Returns an invalid-input error for invalid timing or recurrence fields.
    pub fn normalized(mut self) -> Result<Self, WorkDomainError> {
        self.time_zone = required(&self.time_zone, "task.schedule.time_zone")?;
        if let Some(recurrence) = self.recurrence.as_mut() {
            recurrence.cron_expression = normalize_cron(&recurrence.cron_expression)?;
            let first = next_recurrence_at_or_after(
                &recurrence.cron_expression,
                &self.time_zone,
                recurrence.starts_at,
            )?;
            if self.scheduled_for != first {
                return Err(invalid_input(
                    "task.schedule.scheduled_for",
                    "first occurrence must equal the first cron match at or after startsAt",
                ));
            }
        } else {
            validate_timezone(&self.time_zone)?;
        }
        Ok(self)
    }
}

/// Continuing authority for future occurrences of repeating work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct TaskRecurrenceRecord {
    pub recurrence_id: TaskRecurrenceId,
    pub workspace_id: noema_workspaces::WorkspaceId,
    pub project_id: Option<noema_workspaces::ProjectId>,
    pub title: String,
    pub description_markdown: String,
    pub authorization_context: crate::TaskAuthorizationContext,
    pub starts_at: i64,
    pub cron_expression: String,
    pub time_zone: String,
    pub missed_run_policy: MissedRunPolicy,
    pub overlap_policy: OverlapPolicy,
    pub lifecycle: RecurrenceLifecycle,
    pub revision: u64,
    pub next_run_at: Option<i64>,
    pub pending_coalesced_at: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

/// Immutable recurrence slot audit row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(missing_docs, reason = "field names are the stable domain vocabulary")]
pub struct RecurrenceOccurrenceRecord {
    pub recurrence_id: TaskRecurrenceId,
    pub recurrence_revision: u64,
    pub scheduled_for: i64,
    pub local_slot: String,
    pub trigger: RecurrenceOccurrenceTrigger,
    pub resolution: RecurrenceOccurrenceResolution,
    pub task_id: Option<TaskId>,
    pub created_at: String,
}

/// Return the first cron match at or after the inclusive UTC lower bound.
///
/// # Errors
/// Returns an invalid-input error for an invalid cron, timezone, or timestamp.
pub fn next_recurrence_at_or_after(
    cron_expression: &str,
    time_zone: &str,
    starts_at: i64,
) -> Result<i64, WorkDomainError> {
    let schedule = parse_schedule(cron_expression, time_zone)?;
    let start = jiff::Timestamp::from_second(starts_at)
        .map_err(|error| invalid_input("task.schedule.starts_at", error.to_string()))?;
    if starts_at.rem_euclid(60) == 0 && schedule.matches(start).map_err(schedule_error)? {
        return Ok(starts_at);
    }
    schedule
        .find_next(start)
        .map(|value| value.timestamp().as_second())
        .map_err(schedule_error)
}

/// Resolve five future UTC recurrence instants, including a matching lower bound.
///
/// # Errors
/// Returns an invalid-input error for an invalid cron, timezone, or timestamp.
pub fn recurrence_preview(
    cron_expression: &str,
    time_zone: &str,
    starts_at: i64,
) -> Result<Vec<i64>, WorkDomainError> {
    let schedule = parse_schedule(cron_expression, time_zone)?;
    let first = next_recurrence_at_or_after(cron_expression, time_zone, starts_at)?;
    let mut values = vec![first];
    let mut cursor = first;
    for _ in 1..5 {
        let timestamp = jiff::Timestamp::from_second(cursor)
            .map_err(|error| invalid_input("task.schedule.starts_at", error.to_string()))?;
        cursor = schedule
            .find_next(timestamp)
            .map(|value| value.timestamp().as_second())
            .map_err(schedule_error)?;
        values.push(cursor);
    }
    Ok(values)
}

/// Convert an RFC3339 input into the canonical Unix-second representation.
///
/// # Errors
/// Returns an invalid-input error when `value` is not an RFC3339 instant.
pub fn parse_utc_instant(value: &str, field: &'static str) -> Result<i64, WorkDomainError> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc).timestamp())
        .map_err(|_| invalid_input(field, "expected an RFC3339 instant"))
}

/// Stable wall-clock minute identity used to deduplicate fall-back slots.
///
/// # Errors
/// Returns an invalid-input error for an invalid timezone or timestamp.
pub fn recurrence_local_slot(
    scheduled_for: i64,
    time_zone: &str,
) -> Result<String, WorkDomainError> {
    let zone = jiff::tz::TimeZone::get(time_zone)
        .map_err(|error| invalid_input("task.schedule.time_zone", error.to_string()))?;
    let timestamp = jiff::Timestamp::from_second(scheduled_for)
        .map_err(|error| invalid_input("task.schedule.scheduled_for", error.to_string()))?;
    Ok(timestamp
        .to_zoned(zone)
        .strftime("%Y-%m-%dT%H:%M")
        .to_string())
}

fn normalize_cron(value: &str) -> Result<String, WorkDomainError> {
    let value = required(value, "task.recurrence.cron_expression")?;
    if value.split_whitespace().count() != 5 {
        return Err(invalid_input(
            "task.recurrence.cron_expression",
            "expected a five-field cron expression",
        ));
    }
    Ok(value.split_whitespace().collect::<Vec<_>>().join(" "))
}

fn validate_timezone(time_zone: &str) -> Result<(), WorkDomainError> {
    parse_schedule("0 0 * * *", time_zone).map(drop)
}

fn parse_schedule(
    cron_expression: &str,
    time_zone: &str,
) -> Result<cronexpr::Crontab, WorkDomainError> {
    let cron_expression = normalize_cron(cron_expression)?;
    let time_zone = required(time_zone, "task.schedule.time_zone")?;
    cronexpr::parse_crontab(&format!("{cron_expression} {time_zone}")).map_err(schedule_error)
}

fn schedule_error(error: cronexpr::Error) -> WorkDomainError {
    invalid_input("task.schedule", error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recurrence_start_is_inclusive_and_preview_uses_iana_zone() {
        let start = parse_utc_instant("2026-08-03T12:00:00Z", "start").unwrap();
        let preview = recurrence_preview("0 8 * * *", "America/New_York", start).unwrap();
        assert_eq!(preview[0], start);
        assert_eq!(preview.len(), 5);
    }

    #[test]
    fn invalid_cron_and_timezone_fail_closed() {
        assert!(recurrence_preview("0 8 * *", "UTC", 0).is_err());
        assert!(recurrence_preview("0 8 * * *", "Not/AZone", 0).is_err());
    }

    #[test]
    fn daylight_saving_gap_is_skipped_and_repeated_minute_has_one_slot_identity() {
        let spring = parse_utc_instant("2026-03-08T00:00:00-05:00", "start").unwrap();
        let next = next_recurrence_at_or_after("30 2 * * *", "America/New_York", spring).unwrap();
        assert_eq!(
            next,
            parse_utc_instant("2026-03-09T02:30:00-04:00", "expected").unwrap()
        );

        let first = parse_utc_instant("2026-11-01T01:30:00-04:00", "first").unwrap();
        let repeated = parse_utc_instant("2026-11-01T01:30:00-05:00", "repeated").unwrap();
        assert_eq!(
            recurrence_local_slot(first, "America/New_York").unwrap(),
            recurrence_local_slot(repeated, "America/New_York").unwrap()
        );
    }
}
