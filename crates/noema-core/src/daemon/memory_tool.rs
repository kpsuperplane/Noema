use crate::{
    NoemaStore,
    daemon::memory_pipeline::project_scope_from_cwd,
    memory::{ClaimRetrievalRequest, Purpose, Sensitivity, UseMode},
    store::StoreError,
};
use std::collections::HashSet;

use serde::Deserialize;
use serde_json::{Value, json};

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
    Store(#[from] StoreError),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchMemoryArguments {
    query: String,
    #[serde(default)]
    scope_ids: Vec<String>,
    #[serde(default)]
    purpose: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
}

pub(super) fn is_search_memory_tool(name: &str) -> bool {
    name == SEARCH_MEMORY_TOOL
}

pub(super) async fn execute_search_memory(
    store: &NoemaStore,
    context: &MemoryToolRuntimeContext,
    call_id: Option<String>,
    payload: &Value,
) -> MemoryToolResult {
    match execute_search_memory_inner(store, context, call_id.as_deref(), payload).await {
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
    store: &NoemaStore,
    context: &MemoryToolRuntimeContext,
    _call_id: Option<&str>,
    payload: &Value,
) -> Result<Value, MemoryToolError> {
    let arguments = parse_arguments(payload)?;
    validate_scope_ids(context, &arguments)?;
    let request = build_request(context, &arguments)?;
    let retrieval = store
        .retrieve_claims_scoped(
            &request,
            arguments.query.trim(),
            &arguments.scope_ids,
            arguments.limit(),
        )
        .await?;
    let memories = retrieval
        .included
        .into_iter()
        .map(|claim| {
            json!({
                "id": claim.claim_id,
                "kind": "claim",
                "fact": claim.fact,
                "predicate_id": claim.predicate_id,
                "rank_score": claim.rank_score,
            })
        })
        .collect::<Vec<_>>();
    let omissions = if retrieval.redacted_omission_count == 0 {
        Vec::new()
    } else {
        vec![json!({
            "reason": "policy_restricted_context",
            "count": retrieval.redacted_omission_count,
        })]
    };

    Ok(json!({
        "memories": memories,
        "omissions": omissions,
        "scope_ids": arguments.scope_ids,
    }))
}

fn parse_arguments(payload: &Value) -> Result<SearchMemoryArguments, MemoryToolError> {
    let argument_value = payload.get("arguments").unwrap_or(payload).clone();
    let arguments: SearchMemoryArguments =
        serde_json::from_value(argument_value).map_err(|error| {
            MemoryToolError::InvalidArguments(format!("invalid arguments: {error}"))
        })?;
    if arguments.query.trim().is_empty() && arguments.scope_ids.is_empty() {
        return Err(MemoryToolError::InvalidArguments(
            "query is required unless scope_ids is non-empty".to_string(),
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

fn trusted_active_scope_ids(context: &MemoryToolRuntimeContext) -> Vec<String> {
    let mut ids = vec![
        "human:local".to_string(),
        format!("conversation:{}", context.conversation_id),
    ];
    if let Some(project_scope) = project_scope_from_cwd(context.cwd.as_deref()) {
        ids.push(project_scope);
    }
    ids.sort();
    ids.dedup();
    ids
}

fn validate_scope_ids(
    context: &MemoryToolRuntimeContext,
    arguments: &SearchMemoryArguments,
) -> Result<(), MemoryToolError> {
    let trusted = trusted_active_scope_ids(context)
        .into_iter()
        .collect::<HashSet<_>>();
    for scope_id in &arguments.scope_ids {
        if !trusted.contains(scope_id) {
            return Err(MemoryToolError::InvalidArguments(format!(
                "unsupported scope_id: {scope_id}"
            )));
        }
    }
    Ok(())
}

fn build_request(
    context: &MemoryToolRuntimeContext,
    arguments: &SearchMemoryArguments,
) -> Result<ClaimRetrievalRequest, MemoryToolError> {
    runtime_purpose(arguments.purpose.as_deref())?;
    let mut active_object_ids = vec![format!("conversation:{}", context.conversation_id)];
    if let Some(project_scope) = project_scope_from_cwd(context.cwd.as_deref()) {
        active_object_ids.push(project_scope);
    }
    active_object_ids.extend(arguments.scope_ids.iter().cloned());
    active_object_ids.sort();
    active_object_ids.dedup();

    Ok(ClaimRetrievalRequest {
        requesting_agent_id: "agent:primary".to_string(),
        active_human_ids: vec!["human:local".to_string()],
        active_object_ids,
        use_mode: UseMode::Answer,
        explicit_memory_request: false,
        sensitivity_ceiling: Sensitivity::Normal,
        approved_secret_access: false,
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

fn safe_error_message(error: &MemoryToolError) -> String {
    match error {
        MemoryToolError::InvalidArguments(message) => message.clone(),
        MemoryToolError::Store(_) => "memory retrieval failed".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        assert_eq!(
            safe_error_message(&error),
            "query is required unless scope_ids is non-empty"
        );
    }

    #[test]
    fn parses_empty_query_when_scope_ids_are_present() {
        let payload = json!({
            "arguments": {
                "scope_ids": ["human:local"],
                "query": "",
                "purpose": "answer_human_question"
            }
        });

        let arguments = parse_arguments(&payload).expect("parse scoped empty query");

        assert_eq!(arguments.query, "");
        assert_eq!(arguments.scope_ids, vec!["human:local"]);
    }

    #[test]
    fn rejects_empty_query_without_scope_ids() {
        let payload = json!({
            "arguments": {
                "query": "   "
            }
        });

        let error = parse_arguments(&payload).expect_err("empty unscoped query rejected");

        assert_eq!(
            safe_error_message(&error),
            "query is required unless scope_ids is non-empty"
        );
    }

    #[test]
    fn validates_scope_ids_against_trusted_active_ids() {
        let context = MemoryToolRuntimeContext {
            conversation_id: "conv_123".to_string(),
            turn_id: "turn_456".to_string(),
            turn_index: 7,
            call_site_id: "output_0".to_string(),
            cwd: Some("/Users/kpsuperplane/Documents/Projects/Noema".to_string()),
            user_input: "What memories do you have of me?".to_string(),
        };
        let arguments = SearchMemoryArguments {
            query: "".to_string(),
            scope_ids: vec!["human:local".to_string()],
            purpose: Some("answer_human_question".to_string()),
            limit: None,
        };

        validate_scope_ids(&context, &arguments).expect("trusted scope id");
    }

    #[test]
    fn rejects_out_of_context_scope_ids() {
        let context = MemoryToolRuntimeContext {
            conversation_id: "conv_123".to_string(),
            turn_id: "turn_456".to_string(),
            turn_index: 7,
            call_site_id: "output_0".to_string(),
            cwd: None,
            user_input: "What memories do you have of me?".to_string(),
        };
        let arguments = SearchMemoryArguments {
            query: "".to_string(),
            scope_ids: vec!["project:other".to_string()],
            purpose: Some("answer_human_question".to_string()),
            limit: None,
        };

        let error = validate_scope_ids(&context, &arguments).expect_err("invalid scope id");

        assert_eq!(
            safe_error_message(&error),
            "unsupported scope_id: project:other"
        );
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
    fn natural_language_memory_phrases_do_not_unlock_retrieval_policy() {
        let context = MemoryToolRuntimeContext {
            conversation_id: "conv_123".to_string(),
            turn_id: "turn_456".to_string(),
            turn_index: 7,
            call_site_id: "output_0".to_string(),
            cwd: None,
            user_input: "What do you know about my private plan?".to_string(),
        };
        let arguments = SearchMemoryArguments {
            query: "  launch criteria  ".to_string(),
            scope_ids: vec![],
            purpose: Some("answer_human_question".to_string()),
            limit: None,
        };

        let request = build_request(&context, &arguments).expect("build request");

        assert_eq!(request.requesting_agent_id, "agent:primary");
        assert_eq!(request.active_human_ids, vec!["human:local"]);
        assert_eq!(request.active_object_ids, vec!["conversation:conv_123"]);
        assert_eq!(request.use_mode, UseMode::Answer);
        assert!(!request.explicit_memory_request);
        assert_eq!(request.sensitivity_ceiling, Sensitivity::Normal);
        assert!(!request.approved_secret_access);
    }

    #[test]
    fn validated_scope_args_become_trusted_active_objects() {
        let context = MemoryToolRuntimeContext {
            conversation_id: "conv_123".to_string(),
            turn_id: "turn_456".to_string(),
            turn_index: 7,
            call_site_id: "output_0".to_string(),
            cwd: None,
            user_input: "What do you know about me?".to_string(),
        };
        let arguments = SearchMemoryArguments {
            query: "".to_string(),
            scope_ids: vec!["human:local".to_string()],
            purpose: Some("answer_human_question".to_string()),
            limit: None,
        };

        let request = build_request(&context, &arguments).expect("build scoped request");

        assert_eq!(
            request.active_object_ids,
            vec!["conversation:conv_123", "human:local"]
        );
        assert!(!request.explicit_memory_request);
    }

    #[test]
    fn purpose_argument_is_validated_but_not_trusted() {
        let context = MemoryToolRuntimeContext {
            conversation_id: "conv_123".to_string(),
            turn_id: "turn_456".to_string(),
            turn_index: 7,
            call_site_id: "output_0".to_string(),
            cwd: None,
            user_input: "Please continue the plan".to_string(),
        };
        let arguments = SearchMemoryArguments {
            query: "project memory".to_string(),
            scope_ids: vec![],
            purpose: Some("external_action".to_string()),
            limit: None,
        };

        let request = build_request(&context, &arguments).expect("build request");

        assert_eq!(request.use_mode, UseMode::Answer);
    }
}
