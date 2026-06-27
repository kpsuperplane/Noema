use crate::{
    daemon::memory_pipeline::project_scope_from_cwd,
    memory::{
        EligibilityReason, MemoryRetrievalRequest, MemoryRetrievalResult, Purpose, Sensitivity,
        TrustedRetrievalContext, UntrustedHints,
    },
    memory_persistence::{MemoryPersistenceError, MemorySummary, PostgresMemoryRepository},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashSet;

const SEARCH_MEMORY_TOOL: &str = "search_memory";
const DEFAULT_LIMIT: usize = 8;
const MAX_LIMIT: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MemoryToolRuntimeContext {
    pub conversation_id: String,
    pub turn_id: String,
    pub turn_index: u64,
    pub call_site_id: String,
    pub cwd: Option<String>,
    pub user_input: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MemoryToolResult {
    pub call_id: Option<String>,
    pub name: String,
    pub success: bool,
    pub payload: Value,
}

#[derive(Debug, thiserror::Error)]
pub(super) enum MemoryToolError {
    #[error("{0}")]
    InvalidArguments(String),
    #[error(transparent)]
    Persistence(#[from] MemoryPersistenceError),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchMemoryArguments {
    query: String,
    #[serde(default)]
    purpose: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
}

pub(super) fn is_search_memory_tool(name: &str) -> bool {
    name == SEARCH_MEMORY_TOOL
}

pub(super) async fn execute_search_memory(
    repository: &PostgresMemoryRepository,
    context: &MemoryToolRuntimeContext,
    call_id: Option<String>,
    payload: &Value,
) -> MemoryToolResult {
    match execute_search_memory_inner(repository, context, call_id.as_deref(), payload).await {
        Ok(payload) => MemoryToolResult {
            call_id,
            name: SEARCH_MEMORY_TOOL.to_string(),
            success: true,
            payload,
        },
        Err(error) => MemoryToolResult {
            call_id,
            name: SEARCH_MEMORY_TOOL.to_string(),
            success: false,
            payload: json!({
                "error": safe_error_message(&error),
            }),
        },
    }
}

async fn execute_search_memory_inner(
    repository: &PostgresMemoryRepository,
    context: &MemoryToolRuntimeContext,
    call_id: Option<&str>,
    payload: &Value,
) -> Result<Value, MemoryToolError> {
    let arguments = parse_arguments(payload)?;
    let request = build_request(context, &arguments)?;
    let retrieval = repository.retrieve_memories(&request).await?;
    let limited_retrieval = limit_retrieval_result(retrieval, arguments.limit());
    let context_packet_id = context_packet_id(context, call_id);
    repository
        .record_context_packet(
            &context_packet_id,
            &context.turn_id,
            &request,
            &limited_retrieval,
        )
        .await?;

    let mut memories = Vec::new();
    for included in &limited_retrieval.included {
        if let Some(memory) = repository.get_memory(&included.memory_id).await? {
            memories.push(format_memory(
                &memory,
                eligibility_label(included.eligibility_reason),
            ));
        }
    }

    let omissions = limited_retrieval
        .agent_visible_omissions
        .iter()
        .map(|omission| json!({ "reason": omission.reason }))
        .collect::<Vec<_>>();

    Ok(json!({
        "memories": memories,
        "omissions": omissions,
    }))
}

fn parse_arguments(payload: &Value) -> Result<SearchMemoryArguments, MemoryToolError> {
    let argument_value = payload.get("arguments").unwrap_or(payload).clone();
    let arguments: SearchMemoryArguments =
        serde_json::from_value(argument_value).map_err(|error| {
            MemoryToolError::InvalidArguments(format!("invalid arguments: {error}"))
        })?;
    if arguments.query.trim().is_empty() {
        return Err(MemoryToolError::InvalidArguments(
            "query is required".to_string(),
        ));
    }
    parse_purpose(arguments.purpose.as_deref())?;
    Ok(arguments)
}

impl SearchMemoryArguments {
    fn limit(&self) -> usize {
        self.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT)
    }
}

fn build_request(
    context: &MemoryToolRuntimeContext,
    arguments: &SearchMemoryArguments,
) -> Result<MemoryRetrievalRequest, MemoryToolError> {
    let mut trusted = TrustedRetrievalContext::for_human(
        "human:local",
        runtime_purpose(arguments.purpose.as_deref())?,
    );
    trusted.active_agent_ids = vec!["agent:primary".to_string()];
    trusted.active_scopes = vec![format!("conversation:{}", context.conversation_id)];
    if let Some(project_scope) = project_scope_from_cwd(context.cwd.as_deref()) {
        trusted.active_scopes.push(project_scope);
    }
    trusted.explicit_memory_request = explicit_memory_request(&context.user_input);
    trusted.sensitivity_ceiling = Sensitivity::Normal;
    trusted.include_candidate_memories = false;

    Ok(MemoryRetrievalRequest {
        requesting_principal_id: "agent:primary".to_string(),
        trusted,
        untrusted_hints: UntrustedHints {
            query_text: arguments.query.trim().to_string(),
            fuzzy_topics: Vec::new(),
            fuzzy_entities: Vec::new(),
        },
    })
}

fn runtime_purpose(value: Option<&str>) -> Result<Purpose, MemoryToolError> {
    parse_purpose(value)?;
    Ok(Purpose::AnswerHumanQuestion)
}

fn parse_purpose(value: Option<&str>) -> Result<Purpose, MemoryToolError> {
    match value.unwrap_or("answer_human_question") {
        "answer_human_question" => Ok(Purpose::AnswerHumanQuestion),
        "draft_internal_content" => Ok(Purpose::DraftInternalContent),
        "general_personalization" => Ok(Purpose::GeneralPersonalization),
        "manage_task" => Ok(Purpose::ManageTask),
        "manage_calendar" => Ok(Purpose::ManageCalendar),
        "draft_external_content" => Ok(Purpose::DraftExternalContent),
        "use_tool" => Ok(Purpose::UseTool),
        "proactive_suggestion" => Ok(Purpose::ProactiveSuggestion),
        "external_action" => Ok(Purpose::ExternalAction),
        "debug_audit" => Ok(Purpose::DebugAudit),
        other => Err(MemoryToolError::InvalidArguments(format!(
            "unsupported purpose: {other}"
        ))),
    }
}

fn context_packet_id(context: &MemoryToolRuntimeContext, call_id: Option<&str>) -> String {
    let discriminator = call_id
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&context.call_site_id);
    format!(
        "ctx_search_memory:{}:{}:{}",
        sanitize_context_packet_fragment(&context.conversation_id),
        context.turn_index,
        sanitize_context_packet_fragment(discriminator)
    )
}

fn sanitize_context_packet_fragment(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect::<String>();
    let sanitized = sanitized.trim_matches('_');
    if sanitized.is_empty() {
        "unknown".to_string()
    } else {
        sanitized.to_string()
    }
}

fn limit_retrieval_result(retrieval: MemoryRetrievalResult, limit: usize) -> MemoryRetrievalResult {
    let included = retrieval
        .included
        .into_iter()
        .take(limit)
        .collect::<Vec<_>>();
    let included_ids = included
        .iter()
        .map(|included| included.memory_id.as_str())
        .collect::<HashSet<_>>();
    let use_records = retrieval
        .use_records
        .into_iter()
        .filter(|record| included_ids.contains(record.memory_id.as_str()))
        .collect::<Vec<_>>();

    MemoryRetrievalResult {
        included,
        denied_for_audit: retrieval.denied_for_audit,
        agent_visible_omissions: retrieval.agent_visible_omissions,
        use_records,
    }
}

fn explicit_memory_request(input: &str) -> bool {
    let lowered = input.to_ascii_lowercase();
    lowered.contains("search memory")
        || lowered.contains("read memory")
        || lowered.contains("recall memory")
        || lowered.contains("remembered")
        || lowered.contains("what do you know about")
}

fn format_memory(memory: &MemorySummary, why: &'static str) -> Value {
    json!({
        "id": memory.id,
        "title": memory.title,
        "content": memory.content,
        "scope": memory.home_scope_id,
        "sensitivity": sensitivity_label(memory.sensitivity),
        "why": why,
    })
}

fn sensitivity_label(sensitivity: Sensitivity) -> &'static str {
    match sensitivity {
        Sensitivity::Public => "public",
        Sensitivity::Normal => "normal",
        Sensitivity::Private => "private",
        Sensitivity::Sensitive => "sensitive",
        Sensitivity::Secret => "secret",
    }
}

fn eligibility_label(reason: EligibilityReason) -> &'static str {
    match reason {
        EligibilityReason::ActiveScope => "active_scope",
        EligibilityReason::ParticipantOverlap => "participant_overlap",
        EligibilityReason::ExplicitGrant => "explicit_grant",
        EligibilityReason::TrustedObjectLink => "trusted_object_link",
        EligibilityReason::PublicHint => "public_hint",
        EligibilityReason::GraphExpansion => "graph_expansion",
    }
}

fn safe_error_message(error: &MemoryToolError) -> String {
    match error {
        MemoryToolError::InvalidArguments(message) => message.clone(),
        MemoryToolError::Persistence(_) => "memory retrieval failed".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{MemoryUseRecord, RetrievedMemory};

    #[test]
    fn parses_nested_payload_clamps_limit_and_accepts_default_purpose() {
        let payload = json!({
            "arguments": {
                "query": "project notes",
                "purpose": "answer_human_question",
                "limit": 99,
            }
        });

        let arguments = parse_arguments(&payload).expect("parse arguments");

        assert_eq!(arguments.query, "project notes");
        assert_eq!(arguments.purpose.as_deref(), Some("answer_human_question"));
        assert_eq!(arguments.limit(), MAX_LIMIT);
    }

    #[test]
    fn rejects_empty_query() {
        let payload = json!({
            "query": "   ",
        });

        let error = parse_arguments(&payload).expect_err("empty query rejected");

        assert_eq!(safe_error_message(&error), "query is required");
    }

    #[test]
    fn rejects_unknown_purpose() {
        let payload = json!({
            "query": "project notes",
            "purpose": "dump_everything",
        });

        let error = parse_arguments(&payload).expect_err("unknown purpose rejected");

        assert_eq!(
            safe_error_message(&error),
            "unsupported purpose: dump_everything"
        );
    }

    #[test]
    fn builds_conservative_trusted_request() {
        let context = MemoryToolRuntimeContext {
            conversation_id: "conv_123".to_string(),
            turn_id: "turn_456".to_string(),
            turn_index: 7,
            call_site_id: "output_0".to_string(),
            cwd: None,
            user_input: "Please search memory for our plan".to_string(),
        };
        let arguments = SearchMemoryArguments {
            query: "  launch criteria  ".to_string(),
            purpose: Some("answer_human_question".to_string()),
            limit: None,
        };

        let request = build_request(&context, &arguments).expect("build request");

        assert_eq!(request.requesting_principal_id, "agent:primary");
        assert_eq!(request.trusted.active_human_ids, vec!["human:local"]);
        assert_eq!(request.trusted.active_agent_ids, vec!["agent:primary"]);
        assert_eq!(request.trusted.active_scopes, vec!["conversation:conv_123"]);
        assert_eq!(request.trusted.purpose, Purpose::AnswerHumanQuestion);
        assert!(request.trusted.explicit_memory_request);
        assert_eq!(request.trusted.sensitivity_ceiling, Sensitivity::Normal);
        assert!(!request.trusted.include_candidate_memories);
        assert_eq!(request.untrusted_hints.query_text, "launch criteria");
        assert!(request.untrusted_hints.fuzzy_topics.is_empty());
        assert!(request.untrusted_hints.fuzzy_entities.is_empty());
    }

    #[test]
    fn purpose_argument_is_validated_but_not_trusted() {
        let context = MemoryToolRuntimeContext {
            conversation_id: "conv_123".to_string(),
            turn_id: "turn_456".to_string(),
            turn_index: 7,
            call_site_id: "output_0".to_string(),
            cwd: None,
            user_input: "Please search memory for our plan".to_string(),
        };
        let arguments = SearchMemoryArguments {
            query: "project memory".to_string(),
            purpose: Some("external_action".to_string()),
            limit: None,
        };

        let request = build_request(&context, &arguments).expect("build request");

        assert_eq!(request.trusted.purpose, Purpose::AnswerHumanQuestion);
    }

    #[test]
    fn context_packet_id_uses_sanitized_per_call_discriminator() {
        let context = MemoryToolRuntimeContext {
            conversation_id: "conv/123".to_string(),
            turn_id: "turn_456".to_string(),
            turn_index: 7,
            call_site_id: "output:0".to_string(),
            cwd: None,
            user_input: "Please search memory for our plan".to_string(),
        };

        assert_eq!(
            context_packet_id(&context, Some("call-abc/123")),
            "ctx_search_memory:conv_123:7:call_abc_123"
        );
        assert_eq!(
            context_packet_id(&context, None),
            "ctx_search_memory:conv_123:7:output_0"
        );
    }

    #[test]
    fn limit_truncates_included_and_corresponding_use_records() {
        let retrieval = MemoryRetrievalResult {
            included: vec![
                retrieved_memory("mem_1"),
                retrieved_memory("mem_2"),
                retrieved_memory("mem_3"),
            ],
            denied_for_audit: Vec::new(),
            agent_visible_omissions: Vec::new(),
            use_records: vec![
                memory_use_record("mem_1"),
                memory_use_record("mem_2"),
                memory_use_record("mem_3"),
            ],
        };

        let limited = limit_retrieval_result(retrieval, 2);

        assert_eq!(
            limited
                .included
                .iter()
                .map(|memory| memory.memory_id.as_str())
                .collect::<Vec<_>>(),
            vec!["mem_1", "mem_2"]
        );
        assert_eq!(
            limited
                .use_records
                .iter()
                .map(|record| record.memory_id.as_str())
                .collect::<Vec<_>>(),
            vec!["mem_1", "mem_2"]
        );
    }

    fn retrieved_memory(memory_id: &str) -> RetrievedMemory {
        RetrievedMemory {
            memory_id: memory_id.to_string(),
            rank_score: 1,
            eligibility_reason: EligibilityReason::ActiveScope,
            rank_reasons: Vec::new(),
        }
    }

    fn memory_use_record(memory_id: &str) -> MemoryUseRecord {
        MemoryUseRecord {
            memory_id: memory_id.to_string(),
            stage: crate::memory::MemoryUseStage::IncludedInPacket,
        }
    }
}
