//! Durable queue and lease records for task executor/reviewer runs.

#![allow(clippy::missing_errors_doc)]

mod events;
mod lifecycle;
mod records;
mod recovery;

const RUN_COLUMNS: &str = "run_id, task_id, run_kind, agent_id, attempt_index, revision_index, parent_run_id, triggering_submission_id, triggering_review_id, resume_message, provider_kind, provider_account_id, provider_instance_key, selection_mode, model_profile, reasoning_effort, selection_source, actual_provider_kind, actual_model_profile, max_provider_continuations, max_tool_calls, max_active_minutes, progress_audit_interval, status, priority, queued_at, lease_owner, lease_token, lease_expires_at, heartbeat_at, started_at, ended_at, cancellation_requested, retry_count, error_code, error_message, provider_call_count, tool_call_count, input_tokens, cached_input_tokens, output_tokens, active_milliseconds, created_at, updated_at";
