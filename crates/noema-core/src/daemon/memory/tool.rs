use crate::{
    NoemaStore,
    daemon::memory::{
        HUMAN_MEMORY_SCOPE_ID, context::project_scope_from_cwd, conversation_scope_id,
    },
    provider::{NoemaToolExecution, NoemaToolSpec, ToolContractError},
    store::StoreError,
};
use std::collections::HashSet;

use serde::Deserialize;
use serde_json::{Value, json};

const SEARCH_MEMORY_TOOL: &str = "search_memory";
const DEFAULT_LIMIT: usize = 8;
const MAX_LIMIT: usize = 16;
const SEARCH_MEMORY_PURPOSE_VALUES: &[&str] = &[
    "answer_human_question",
    "draft_internal_content",
    "general_personalization",
    "manage_task",
    "manage_calendar",
    "draft_external_content",
    "use_tool",
    "proactive_suggestion",
    "external_action",
    "debug_audit",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::daemon) struct MemoryToolRuntimeContext {
    pub conversation_id: String,
    pub turn_id: String,
    pub turn_index: u64,
    pub call_site_id: String,
    pub cwd: Option<String>,
    pub user_input: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::daemon) struct MemoryToolResult {
    pub call_id: Option<String>,
    pub name: String,
    pub success: bool,
    pub payload: Value,
}

#[derive(Debug, thiserror::Error)]
pub(in crate::daemon) enum MemoryToolError {
    #[error("{0}")]
    InvalidArguments(String),
    #[error("memory service unavailable")]
    Unavailable {
        code: &'static str,
        message: &'static str,
    },
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum SearchMemoryPurpose {
    AnswerHumanQuestion,
    DraftInternalContent,
    GeneralPersonalization,
    ManageTask,
    ManageCalendar,
    DraftExternalContent,
    UseTool,
    ProactiveSuggestion,
    ExternalAction,
    DebugAudit,
}

pub(in crate::daemon) fn is_search_memory_tool(name: &str) -> bool {
    name == SEARCH_MEMORY_TOOL
}

pub(in crate::daemon) fn search_memory_tool_spec() -> Result<NoemaToolSpec, ToolContractError> {
    NoemaToolSpec::new(
        SEARCH_MEMORY_TOOL,
        "Search governed Noema memory for the current user, conversation, or active project.",
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search text. May be empty only when scope_ids is non-empty."
                },
                "scope_ids": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Concrete active memory scopes such as human:local or conversation:<id>."
                },
                "purpose": {
                    "type": "string",
                    "enum": SEARCH_MEMORY_PURPOSE_VALUES,
                    "description": "Policy purpose for retrieval."
                },
                "limit": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": MAX_LIMIT,
                    "description": "Maximum number of memory facts to return."
                }
            },
            "required": ["query"],
            "additionalProperties": false
        }),
        NoemaToolExecution::LocalBuiltin,
    )
}

pub(in crate::daemon) async fn execute_search_memory(
    _store: &NoemaStore,
    client: Option<crate::Mem0Client>,
    context: &MemoryToolRuntimeContext,
    call_id: Option<String>,
    payload: &Value,
) -> MemoryToolResult {
    match execute_search_memory_inner(client, context, call_id.as_deref(), payload).await {
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
                "error": safe_error_payload(&error),
            }),
        },
    }
}

async fn execute_search_memory_inner(
    client: Option<crate::Mem0Client>,
    context: &MemoryToolRuntimeContext,
    _call_id: Option<&str>,
    payload: &Value,
) -> Result<Value, MemoryToolError> {
    let arguments = parse_arguments(payload)?;
    validate_scope_ids(context, &arguments)?;
    let client = client.ok_or(MemoryToolError::Unavailable {
        code: "service_unavailable",
        message: "memory service is unavailable",
    })?;
    let scope_ids = if arguments.scope_ids.is_empty() {
        trusted_active_scope_ids(context)
    } else {
        arguments.scope_ids.clone()
    };
    let mut memories = Vec::new();
    for scope_id in scope_ids {
        validate_memory_scope_shape(&scope_id)?;
        let run_id = if scope_id == conversation_scope_id(&context.conversation_id) {
            Some(context.conversation_id.clone())
        } else {
            None
        };
        let response = client
            .search_memories(crate::Mem0SearchRequest {
                query: arguments.query.clone(),
                user_id: HUMAN_MEMORY_SCOPE_ID.to_string(),
                agent_id: None,
                run_id,
                limit: arguments.limit() as u16,
            })
            .await
            .map_err(|error| MemoryToolError::Unavailable {
                code: error.sanitized_code(),
                message: error.sanitized_message(),
            })?;
        for result in response.results {
            let Some(memory) = result.memory else {
                continue;
            };
            memories.push(json!({
                "id": result.id,
                "kind": "mem0",
                "memory": memory,
                "score": result.score,
                "updated_at": result.updated_at,
                "scope_id": scope_id,
            }));
        }
    }
    memories.sort_by(|left, right| {
        let left_score = left["score"].as_f64().unwrap_or(0.0);
        let right_score = right["score"].as_f64().unwrap_or(0.0);
        right_score
            .partial_cmp(&left_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left["id"].as_str().cmp(&right["id"].as_str()))
    });
    memories.truncate(arguments.limit());

    Ok(json!({
        "memories": memories,
        "omissions": [],
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
        HUMAN_MEMORY_SCOPE_ID.to_string(),
        conversation_scope_id(&context.conversation_id),
    ];
    if let Some(project_scope) = project_scope_from_cwd(context.cwd.as_deref()) {
        ids.push(project_scope);
    }
    ids.sort();
    ids.dedup();
    ids
}

fn validate_memory_scope_shape(scope_id: &str) -> Result<(), MemoryToolError> {
    let allowed_prefix = [
        "human:",
        "conversation:",
        "project:",
        "workspace:",
        "agent:",
    ]
    .iter()
    .any(|prefix| scope_id.starts_with(prefix));
    let valid_chars = scope_id
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | ':'));
    if !allowed_prefix || !valid_chars {
        Err(MemoryToolError::InvalidArguments(format!(
            "unsupported scope_id: {scope_id}"
        )))
    } else {
        Ok(())
    }
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

fn parse_purpose(value: Option<&str>) -> Result<SearchMemoryPurpose, MemoryToolError> {
    match value.unwrap_or("answer_human_question") {
        "answer_human_question" => Ok(SearchMemoryPurpose::AnswerHumanQuestion),
        "draft_internal_content" => Ok(SearchMemoryPurpose::DraftInternalContent),
        "general_personalization" => Ok(SearchMemoryPurpose::GeneralPersonalization),
        "manage_task" => Ok(SearchMemoryPurpose::ManageTask),
        "manage_calendar" => Ok(SearchMemoryPurpose::ManageCalendar),
        "draft_external_content" => Ok(SearchMemoryPurpose::DraftExternalContent),
        "use_tool" => Ok(SearchMemoryPurpose::UseTool),
        "proactive_suggestion" => Ok(SearchMemoryPurpose::ProactiveSuggestion),
        "external_action" => Ok(SearchMemoryPurpose::ExternalAction),
        "debug_audit" => Ok(SearchMemoryPurpose::DebugAudit),
        other => Err(MemoryToolError::InvalidArguments(format!(
            "unsupported purpose: {other}"
        ))),
    }
}

fn safe_error_payload(error: &MemoryToolError) -> Value {
    match error {
        MemoryToolError::InvalidArguments(message) => json!(message),
        MemoryToolError::Unavailable { code, message } => json!({
            "code": code,
            "message": message,
        }),
        MemoryToolError::Store(_) => json!({
            "code": "store_error",
            "message": "memory retrieval failed",
        }),
    }
}

#[cfg(test)]
fn safe_error_message(error: &MemoryToolError) -> String {
    match safe_error_payload(error) {
        Value::String(message) => message,
        Value::Object(mut object) => object
            .remove("message")
            .and_then(|value| value.as_str().map(str::to_string))
            .unwrap_or_else(|| "memory retrieval failed".to_string()),
        _ => "memory retrieval failed".to_string(),
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
    fn search_memory_tool_spec_matches_runtime_arguments() {
        let spec = search_memory_tool_spec().expect("tool spec");

        assert_eq!(spec.name.as_str(), SEARCH_MEMORY_TOOL);
        assert!(spec.description.contains("Search governed Noema memory"));
        assert_eq!(spec.input_schema.as_value()["required"], json!(["query"]));
        assert_eq!(spec.input_schema.as_value()["additionalProperties"], false);
        assert_eq!(
            spec.input_schema.as_value()["properties"]["scope_ids"]["items"]["type"],
            "string"
        );
        assert_eq!(
            spec.input_schema.as_value()["properties"]["limit"]["maximum"],
            MAX_LIMIT
        );
        let purpose_enum = &spec.input_schema.as_value()["properties"]["purpose"]["enum"];
        assert_eq!(purpose_enum, &json!(SEARCH_MEMORY_PURPOSE_VALUES));
        let purpose_values = purpose_enum.as_array().expect("purpose enum values");
        for purpose in purpose_values {
            parse_purpose(purpose.as_str()).expect("schema purpose accepted by runtime");
        }
    }

    #[test]
    fn memory_scope_shape_accepts_safe_scope_ids() {
        validate_memory_scope_shape("human:local").expect("scope");
        validate_memory_scope_shape("conversation:abc_123").expect("scope");
    }

    #[test]
    fn memory_scope_shape_rejects_unknown_scope_shape() {
        let error = validate_memory_scope_shape("not allowed").expect_err("invalid");
        assert!(error.to_string().contains("unsupported scope_id"));
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
    fn natural_language_memory_phrases_do_not_unlock_additional_scopes() {
        let context = MemoryToolRuntimeContext {
            conversation_id: "conv_123".to_string(),
            turn_id: "turn_456".to_string(),
            turn_index: 7,
            call_site_id: "output_0".to_string(),
            cwd: None,
            user_input: "What do you know about my private plan?".to_string(),
        };
        assert_eq!(
            trusted_active_scope_ids(&context),
            vec!["conversation:conv_123", "human:local"]
        );
    }

    #[test]
    fn trusted_scope_ids_preserve_canonical_conversation_ids() {
        let context = MemoryToolRuntimeContext {
            conversation_id: "conversation:conv_123".to_string(),
            turn_id: "turn_456".to_string(),
            turn_index: 7,
            call_site_id: "output_0".to_string(),
            cwd: None,
            user_input: "What do you remember from this chat?".to_string(),
        };

        assert_eq!(
            trusted_active_scope_ids(&context),
            vec!["conversation:conv_123", "human:local"]
        );
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

        validate_scope_ids(&context, &arguments).expect("validated scope");
    }

    #[test]
    fn purpose_argument_is_validated_but_not_used_as_scope_authority() {
        assert_eq!(
            parse_purpose(Some("external_action")).expect("valid purpose"),
            SearchMemoryPurpose::ExternalAction
        );
    }
}
