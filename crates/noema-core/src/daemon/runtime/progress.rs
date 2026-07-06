#![allow(dead_code)]

use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, VecDeque};

use super::local_tools::LocalToolResult;

pub(super) const PROGRESS_AUDIT_INTERVAL: usize = 20;
pub(super) const MAX_PROVIDER_TOOL_CONTINUATIONS: usize = 80;
const RECENT_EVENT_LIMIT: usize = 5;
const RECENT_EVENT_CHAR_LIMIT: usize = 240;
const USER_GOAL_CHAR_LIMIT: usize = 240;
const CURRENT_GOAL_CHAR_LIMIT: usize = 240;
const REPEATED_ARGUMENT_THRESHOLD: usize = 4;
const FAILURE_STREAK_THRESHOLD: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DeterministicProgressStop {
    RepeatedArguments,
    FailureStreak,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(super) struct ContinuationProgressDigest {
    pub user_goal: String,
    pub current_goal: Option<String>,
    pub step: usize,
    pub window: ProgressWindowDigest,
    pub whole_turn: ProgressTurnDigest,
    pub recent_events: Vec<ProgressEvent>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub(super) struct ProgressWindowDigest {
    pub tool_counts: BTreeMap<String, usize>,
    pub success_count: usize,
    pub failure_count: usize,
    pub failure_streak: usize,
    pub repeated_argument_count: usize,
    pub novel_result_count: usize,
    pub side_effect_count: usize,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub(super) struct ProgressTurnDigest {
    pub continuation_count: usize,
    pub tool_counts: BTreeMap<String, usize>,
    pub success_count: usize,
    pub failure_count: usize,
    pub failure_streak: usize,
    pub repeated_argument_count: usize,
    pub novel_result_count: usize,
    pub side_effect_count: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub(super) struct ProgressEvent {
    pub tool_name: String,
    pub success: bool,
    pub summary: String,
}

#[derive(Debug, Clone)]
pub(super) struct ContinuationProgressTracker {
    user_goal: String,
    current_goal: Option<String>,
    window: ProgressWindowDigest,
    whole_turn: ProgressTurnDigest,
    argument_counts: BTreeMap<String, usize>,
    recent_events: VecDeque<ProgressEvent>,
}

impl ContinuationProgressTracker {
    pub(super) fn new(user_goal: &str) -> Self {
        Self {
            user_goal: truncate_chars(user_goal, USER_GOAL_CHAR_LIMIT),
            current_goal: None,
            window: ProgressWindowDigest::default(),
            whole_turn: ProgressTurnDigest::default(),
            argument_counts: BTreeMap::new(),
            recent_events: VecDeque::new(),
        }
    }

    pub(super) fn observe_results(&mut self, results: &[LocalToolResult]) {
        for result in results {
            let tool_name = result.name().to_string();
            let success = result.success();
            *self
                .window
                .tool_counts
                .entry(tool_name.clone())
                .or_default() += 1;
            *self
                .whole_turn
                .tool_counts
                .entry(tool_name.clone())
                .or_default() += 1;
            if success {
                self.window.success_count += 1;
                self.whole_turn.success_count += 1;
                self.window.failure_streak = 0;
                self.whole_turn.failure_streak = 0;
            } else {
                self.window.failure_count += 1;
                self.whole_turn.failure_count += 1;
                self.window.failure_streak += 1;
                self.whole_turn.failure_streak += 1;
            }
            let fingerprint = argument_fingerprint(result.name(), result_arguments(result));
            let count = self.argument_counts.entry(fingerprint).or_insert(0);
            *count += 1;
            if *count > 1 {
                self.window.repeated_argument_count += 1;
                self.whole_turn.repeated_argument_count += 1;
            }
            if result_novel(result_payload(result)) {
                self.window.novel_result_count += 1;
                self.whole_turn.novel_result_count += 1;
            }
            if result_side_effect(result.name(), result.success()) {
                self.window.side_effect_count += 1;
                self.whole_turn.side_effect_count += 1;
            }
            self.push_event(ProgressEvent {
                tool_name,
                success,
                summary: summarize_result(result),
            });
        }
    }

    pub(super) fn mark_continuation_step(&mut self, step: usize) {
        self.whole_turn.continuation_count = step;
    }

    pub(super) fn should_audit(step: usize) -> bool {
        step > 0 && step.is_multiple_of(PROGRESS_AUDIT_INTERVAL)
    }

    pub(super) fn deterministic_stop(&self) -> Option<DeterministicProgressStop> {
        if self.window.failure_streak >= FAILURE_STREAK_THRESHOLD {
            return Some(DeterministicProgressStop::FailureStreak);
        }
        if self.window.repeated_argument_count >= REPEATED_ARGUMENT_THRESHOLD {
            return Some(DeterministicProgressStop::RepeatedArguments);
        }
        None
    }

    pub(super) fn digest(&self, step: usize) -> ContinuationProgressDigest {
        ContinuationProgressDigest {
            user_goal: self.user_goal.clone(),
            current_goal: self.current_goal.clone(),
            step,
            window: self.window.clone(),
            whole_turn: self.whole_turn.clone(),
            recent_events: self.recent_events.iter().cloned().collect(),
        }
    }

    pub(super) fn reset_window(&mut self) {
        self.window = ProgressWindowDigest::default();
    }

    pub(super) fn update_current_goal(&mut self, goal: Option<String>) {
        self.current_goal = goal.map(|goal| truncate_chars(&goal, CURRENT_GOAL_CHAR_LIMIT));
    }

    fn push_event(&mut self, mut event: ProgressEvent) {
        event.summary = truncate_chars(&event.summary, RECENT_EVENT_CHAR_LIMIT);
        self.recent_events.push_back(event);
        while self.recent_events.len() > RECENT_EVENT_LIMIT {
            self.recent_events.pop_front();
        }
    }
}

fn argument_fingerprint(name: &str, arguments: &Value) -> String {
    format!(
        "{name}:{}",
        serde_json::to_string(arguments).unwrap_or_default()
    )
}

fn result_arguments(result: &LocalToolResult) -> &Value {
    match result {
        LocalToolResult::Memory { arguments, .. }
        | LocalToolResult::AgentName { arguments, .. }
        | LocalToolResult::WebSearch { arguments, .. }
        | LocalToolResult::WebFetch { arguments, .. }
        | LocalToolResult::Gateway { arguments, .. } => arguments,
    }
}

fn result_payload(result: &LocalToolResult) -> &Value {
    match result {
        LocalToolResult::Memory { result, .. } => &result.payload,
        LocalToolResult::AgentName { result, .. } => &result.payload,
        LocalToolResult::WebSearch { result, .. } => &result.payload,
        LocalToolResult::WebFetch { result, .. } => &result.payload,
        LocalToolResult::Gateway { result, .. } => &result.payload,
    }
}

fn result_novel(payload: &Value) -> bool {
    payload
        .get("results")
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty())
        || payload.get("url").and_then(Value::as_str).is_some()
        || payload.get("object_id").and_then(Value::as_str).is_some()
        || payload.get("page_id").and_then(Value::as_str).is_some()
}

fn result_side_effect(name: &str, success: bool) -> bool {
    success && (name.contains("create") || name.contains("update") || name.contains("delete"))
}

fn summarize_result(result: &LocalToolResult) -> String {
    let payload = result_payload(result);
    payload
        .get("summary")
        .and_then(Value::as_str)
        .or_else(|| payload.get("error").and_then(Value::as_str))
        .map(str::to_string)
        .unwrap_or_else(|| {
            let status = if result.success() {
                "succeeded"
            } else {
                "failed"
            };
            format!("{} {status}", result.name())
        })
}

fn truncate_chars(value: &str, limit: usize) -> String {
    let mut output = String::new();
    for character in value.chars().take(limit) {
        output.push(character);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::runtime::local_tools::LocalToolResult;
    use crate::search::tool::WebSearchToolResult;
    use serde_json::json;

    fn web_search_result(call_id: &str, success: bool, payload: Value) -> LocalToolResult {
        LocalToolResult::WebSearch {
            call_id: Some(call_id.to_string()),
            provider_call_id: Some(call_id.to_string()),
            provider_name: Some("web.search".to_string()),
            arguments: json!({ "query": "healthy restaurants" }),
            result: WebSearchToolResult {
                call_id: Some(call_id.to_string()),
                name: "web.search".to_string(),
                success,
                payload,
            },
        }
    }

    #[test]
    fn audit_triggers_every_twenty_steps() {
        assert!(!ContinuationProgressTracker::should_audit(0));
        assert!(!ContinuationProgressTracker::should_audit(19));
        assert!(ContinuationProgressTracker::should_audit(20));
        assert!(!ContinuationProgressTracker::should_audit(21));
        assert!(ContinuationProgressTracker::should_audit(40));
    }

    #[test]
    fn digest_keeps_recent_events_bounded() {
        let mut tracker = ContinuationProgressTracker::new("find restaurants");
        for index in 0..8 {
            tracker.observe_results(&[web_search_result(
                &format!("call_{index}"),
                true,
                json!({ "summary": format!("event {index}"), "results": [{ "title": "Place" }] }),
            )]);
        }

        let digest = tracker.digest(20);
        assert_eq!(digest.recent_events.len(), RECENT_EVENT_LIMIT);
        assert_eq!(digest.window.success_count, 8);
        assert_eq!(digest.whole_turn.novel_result_count, 8);
    }

    #[test]
    fn repeated_arguments_trigger_deterministic_stop() {
        let mut tracker = ContinuationProgressTracker::new("find restaurants");
        for index in 0..5 {
            tracker.observe_results(&[web_search_result(
                &format!("call_{index}"),
                true,
                json!({ "summary": "same query", "results": [] }),
            )]);
        }

        assert_eq!(
            tracker.deterministic_stop(),
            Some(DeterministicProgressStop::RepeatedArguments)
        );
    }
}
