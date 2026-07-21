use async_graphql::{Enum, InputObject, SimpleObject};
use chrono::{DateTime, SecondsFormat, Utc};
use noema_store::{
    RuntimeDebugProfileRecord, RuntimeDebugScope, RuntimeDebugSpanCategory, RuntimeDebugSpanRecord,
    RuntimeDebugSpanStatus,
};

use super::{errors::graphql_error, schema::GraphqlState};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "RuntimeDebugScopeKind")]
pub(super) enum GraphqlRuntimeDebugScopeKind {
    ConversationTurn,
    TaskRun,
}

#[derive(Clone, Debug, InputObject)]
#[graphql(name = "RuntimeDebugProfileInput")]
pub(super) struct GraphqlRuntimeDebugProfileInput {
    kind: GraphqlRuntimeDebugScopeKind,
    scope_id: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "RuntimeDebugStatus")]
pub(super) enum GraphqlRuntimeDebugStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
    Interrupted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Enum)]
#[graphql(name = "RuntimeDebugSpanCategory")]
pub(super) enum GraphqlRuntimeDebugSpanCategory {
    Provider,
    Tool,
    Runtime,
    Persistence,
}

#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "RuntimeDebugSpan")]
pub(super) struct GraphqlRuntimeDebugSpan {
    id: String,
    category: GraphqlRuntimeDebugSpanCategory,
    name: String,
    status: GraphqlRuntimeDebugStatus,
    started_at: String,
    ended_at: Option<String>,
    start_offset_milliseconds: i64,
    duration_milliseconds: i64,
    provider: Option<String>,
    model: Option<String>,
    phase: Option<String>,
    response_index: Option<i64>,
    round_index: Option<i64>,
    tool_name: Option<String>,
    correlation_id: Option<String>,
    input_tokens: Option<i64>,
    cached_input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    total_tokens: Option<i64>,
}

#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "RuntimeDebugProfile")]
pub(super) struct GraphqlRuntimeDebugProfile {
    kind: GraphqlRuntimeDebugScopeKind,
    scope_id: String,
    status: GraphqlRuntimeDebugStatus,
    started_at: String,
    ended_at: Option<String>,
    elapsed_milliseconds: i64,
    accounted_milliseconds: i64,
    uninstrumented_milliseconds: i64,
    spans: Vec<GraphqlRuntimeDebugSpan>,
}

pub(super) async fn runtime_debug_profile(
    state: &GraphqlState,
    principal: &str,
    input: GraphqlRuntimeDebugProfileInput,
) -> async_graphql::Result<Option<GraphqlRuntimeDebugProfile>> {
    let scope_id = input.scope_id.trim();
    if scope_id.is_empty() {
        return Ok(None);
    }
    let scope = match input.kind {
        GraphqlRuntimeDebugScopeKind::ConversationTurn => {
            RuntimeDebugScope::ConversationTurn(scope_id.to_string())
        }
        GraphqlRuntimeDebugScopeKind::TaskRun => RuntimeDebugScope::AgentRun(scope_id.to_string()),
    };
    let Some(profile) = state
        .store()?
        .runtime_debug_profile(scope)
        .await
        .map_err(graphql_error)?
    else {
        return Ok(None);
    };
    match &profile.scope {
        RuntimeDebugScope::ConversationTurn(_) => {
            if !state
                .store()?
                .conversation_is_owned_by_human(&profile.container_id, principal)
                .await
                .map_err(graphql_error)?
            {
                return Ok(None);
            }
        }
        RuntimeDebugScope::AgentRun(_) if principal != "human:local" => return Ok(None),
        RuntimeDebugScope::AgentRun(_) => {}
    }
    if profile.spans.is_empty() {
        return Ok(None);
    }
    project_profile(profile).map(Some)
}

fn project_profile(
    profile: RuntimeDebugProfileRecord,
) -> async_graphql::Result<GraphqlRuntimeDebugProfile> {
    let status = profile_status(&profile.status);
    let started = parse_time(&profile.started_at)?;
    let effective_end = profile
        .ended_at
        .as_deref()
        .map(parse_time)
        .transpose()?
        .unwrap_or_else(Utc::now);
    let elapsed = milliseconds(effective_end - started);
    let mut spans = Vec::with_capacity(profile.spans.len());
    let terminal = status != GraphqlRuntimeDebugStatus::Running;
    for span in profile.spans {
        spans.push(project_span(span, started, effective_end, terminal)?);
    }
    let intervals = spans
        .iter()
        .map(|span| {
            Ok((
                span.start_offset_milliseconds,
                timeline_duration_milliseconds(
                    &span.started_at,
                    span.ended_at.as_deref(),
                    span.duration_milliseconds,
                )?,
            ))
        })
        .collect::<async_graphql::Result<Vec<_>>>()?;
    let accounted = union_milliseconds(intervals, elapsed);
    let (kind, scope_id) = match profile.scope {
        RuntimeDebugScope::ConversationTurn(id) => {
            (GraphqlRuntimeDebugScopeKind::ConversationTurn, id)
        }
        RuntimeDebugScope::AgentRun(id) => (GraphqlRuntimeDebugScopeKind::TaskRun, id),
    };
    Ok(GraphqlRuntimeDebugProfile {
        kind,
        scope_id,
        status,
        started_at: format_time(started),
        ended_at: profile
            .ended_at
            .as_deref()
            .map(parse_time)
            .transpose()?
            .map(format_time),
        elapsed_milliseconds: elapsed,
        accounted_milliseconds: accounted,
        uninstrumented_milliseconds: elapsed.saturating_sub(accounted),
        spans,
    })
}

fn project_span(
    span: RuntimeDebugSpanRecord,
    profile_start: DateTime<Utc>,
    profile_end: DateTime<Utc>,
    profile_terminal: bool,
) -> async_graphql::Result<GraphqlRuntimeDebugSpan> {
    let started = parse_time(&span.started_at)?;
    let status = if profile_terminal && span.status == RuntimeDebugSpanStatus::Running {
        GraphqlRuntimeDebugStatus::Interrupted
    } else {
        span_status(span.status)
    };
    let duration = span
        .duration_milliseconds
        .map(exact_u64)
        .unwrap_or_else(|| milliseconds(profile_end - started));
    let metadata = span.metadata;
    Ok(GraphqlRuntimeDebugSpan {
        id: span.span_id,
        category: match span.category {
            RuntimeDebugSpanCategory::Provider => GraphqlRuntimeDebugSpanCategory::Provider,
            RuntimeDebugSpanCategory::Tool => GraphqlRuntimeDebugSpanCategory::Tool,
            RuntimeDebugSpanCategory::Runtime => GraphqlRuntimeDebugSpanCategory::Runtime,
            RuntimeDebugSpanCategory::Persistence => GraphqlRuntimeDebugSpanCategory::Persistence,
        },
        name: span.name,
        status,
        started_at: format_time(started),
        ended_at: span
            .ended_at
            .as_deref()
            .map(parse_time)
            .transpose()?
            .map(format_time),
        start_offset_milliseconds: milliseconds(started - profile_start).max(0),
        duration_milliseconds: duration,
        provider: metadata.provider,
        model: metadata.model,
        phase: metadata.phase,
        response_index: metadata.response_index.map(exact_u64),
        round_index: metadata.round_index.map(exact_u64),
        tool_name: metadata.tool_name,
        correlation_id: metadata.correlation_id,
        input_tokens: metadata.input_tokens.map(exact_u64),
        cached_input_tokens: metadata.cached_input_tokens.map(exact_u64),
        output_tokens: metadata.output_tokens.map(exact_u64),
        total_tokens: metadata.total_tokens.map(exact_u64),
    })
}

fn profile_status(status: &str) -> GraphqlRuntimeDebugStatus {
    match status {
        "completed" => GraphqlRuntimeDebugStatus::Completed,
        "failed" => GraphqlRuntimeDebugStatus::Failed,
        "cancelled" => GraphqlRuntimeDebugStatus::Cancelled,
        "interrupted" => GraphqlRuntimeDebugStatus::Interrupted,
        _ => GraphqlRuntimeDebugStatus::Running,
    }
}

fn span_status(status: RuntimeDebugSpanStatus) -> GraphqlRuntimeDebugStatus {
    match status {
        RuntimeDebugSpanStatus::Running => GraphqlRuntimeDebugStatus::Running,
        RuntimeDebugSpanStatus::Completed => GraphqlRuntimeDebugStatus::Completed,
        RuntimeDebugSpanStatus::Failed => GraphqlRuntimeDebugStatus::Failed,
        RuntimeDebugSpanStatus::Cancelled => GraphqlRuntimeDebugStatus::Cancelled,
        RuntimeDebugSpanStatus::Interrupted => GraphqlRuntimeDebugStatus::Interrupted,
    }
}

fn parse_time(value: &str) -> async_graphql::Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .or_else(|_| {
            value
                .parse::<i64>()
                .ok()
                .and_then(|seconds| DateTime::from_timestamp(seconds, 0))
                .ok_or(())
        })
        .map_err(|_| unavailable())
}

fn format_time(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn milliseconds(duration: chrono::TimeDelta) -> i64 {
    duration.num_milliseconds().max(0)
}

fn timeline_duration_milliseconds(
    started_at: &str,
    ended_at: Option<&str>,
    monotonic_duration: i64,
) -> async_graphql::Result<i64> {
    ended_at.map_or(Ok(monotonic_duration), |ended| {
        Ok(milliseconds(parse_time(ended)? - parse_time(started_at)?))
    })
}

fn union_milliseconds(intervals: impl IntoIterator<Item = (i64, i64)>, elapsed: i64) -> i64 {
    let mut intervals = intervals
        .into_iter()
        .filter_map(|(start, duration)| {
            let start = start.clamp(0, elapsed);
            let end = start.saturating_add(duration.max(0)).clamp(0, elapsed);
            (end > start).then_some((start, end))
        })
        .collect::<Vec<_>>();
    intervals.sort_unstable_by_key(|interval| interval.0);
    let Some((mut start, mut end)) = intervals.first().copied() else {
        return 0;
    };
    let mut covered = 0i64;
    for (next_start, next_end) in intervals.into_iter().skip(1) {
        if next_start <= end {
            end = end.max(next_end);
        } else {
            covered = covered.saturating_add(end - start);
            (start, end) = (next_start, next_end);
        }
    }
    covered.saturating_add(end - start).min(elapsed)
}

fn exact_u64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn unavailable() -> async_graphql::Error {
    async_graphql::Error::new("runtime debug profile is unavailable")
}

#[cfg(test)]
mod tests {
    use super::{timeline_duration_milliseconds, union_milliseconds};

    #[test]
    fn wall_clock_intervals_do_not_double_count_clock_adjustment() {
        let timeline_duration = timeline_duration_milliseconds(
            "2026-07-21T21:00:00.000Z",
            Some("2026-07-21T21:00:03.600Z"),
            4_200,
        )
        .expect("valid timestamps");
        assert_eq!(timeline_duration, 3_600);
        assert_eq!(
            union_milliseconds([(0, timeline_duration), (3_600, 2_500), (3_950, 7)], 6_100),
            6_100
        );
    }
}
