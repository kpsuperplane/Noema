//! Stable governed `search_memory` capability semantics.

use std::collections::HashSet;

use noema_capabilities::{ToolContractError, ToolSpec};
use serde::Deserialize;
use serde_json::{Value, json};
use thiserror::Error;

use crate::{
    model::{
        HUMAN_MEMORY_SCOPE_ID, ListMemoriesRequest, SearchMemoriesRequest, conversation_scope_id,
    },
    operations::{MemoryOperationError, MemoryOperations},
};

/// Stable memory-search tool name.
pub const SEARCH_MEMORY_TOOL_NAME: &str = "search_memory";

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

/// Runtime-authoritative memory scopes for one tool invocation.
///
/// Runtime owns active-object derivation. Memory validates model-requested
/// scopes only against this explicit snapshot and never infers authority from
/// natural language, filesystem paths, or service results.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemorySearchAuthority {
    /// Canonical conversation scope authorized for the current turn.
    conversation_scope_id: String,
    /// Exact runtime conversation id forwarded to the memory service as run id.
    conversation_run_id: String,
    /// Complete active scope set authorized for this invocation.
    active_scope_ids: Vec<String>,
}

impl MemorySearchAuthority {
    /// Build the standard human/conversation authority plus explicit scopes
    /// assembled by runtime, such as the active project.
    #[must_use]
    pub fn for_conversation(
        conversation_id: &str,
        additional_active_scope_ids: impl IntoIterator<Item = String>,
    ) -> Self {
        let conversation_scope_id = conversation_scope_id(conversation_id);
        let mut active_scope_ids = vec![
            HUMAN_MEMORY_SCOPE_ID.to_string(),
            conversation_scope_id.clone(),
        ];
        active_scope_ids.extend(additional_active_scope_ids);
        active_scope_ids.sort();
        active_scope_ids.dedup();
        Self {
            conversation_scope_id,
            conversation_run_id: conversation_id.to_string(),
            active_scope_ids,
        }
    }

    /// Return the complete runtime-authorized scope snapshot.
    #[must_use]
    pub fn active_scope_ids(&self) -> &[String] {
        &self.active_scope_ids
    }
}

/// Stable result envelope consumed by runtime transcript/audit handling.
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryToolResult {
    /// Provider/model tool-call id when present.
    pub call_id: Option<String>,
    /// Stable tool name.
    pub name: String,
    /// Whether execution succeeded.
    pub success: bool,
    /// Stable model-visible result or safe error payload.
    pub payload: Value,
}

/// Governed search input or service-operation failure.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SearchMemoryError {
    /// Model-supplied arguments violate the stable search contract.
    #[error("{0}")]
    InvalidArguments(String),
    /// The selected memory service could not complete retrieval.
    #[error(transparent)]
    Operation(#[from] MemoryOperationError),
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

/// Return whether `name` identifies the governed memory-search operation.
#[must_use]
pub fn is_search_memory_tool(name: &str) -> bool {
    name == SEARCH_MEMORY_TOOL_NAME
}

/// Build the canonical `search_memory` tool specification.
///
/// # Errors
///
/// Returns [`ToolContractError`] if the static specification violates the
/// capability contract.
pub fn search_memory_tool_spec() -> Result<ToolSpec, ToolContractError> {
    ToolSpec::new(
        SEARCH_MEMORY_TOOL_NAME,
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
    )
}

/// Execute governed memory search through an injected operations boundary.
pub async fn execute_search_memory(
    operations: Option<&dyn MemoryOperations>,
    authority: &MemorySearchAuthority,
    call_id: Option<String>,
    payload: &Value,
) -> MemoryToolResult {
    match execute_search_memory_inner(operations, authority, payload).await {
        Ok(payload) => MemoryToolResult {
            call_id,
            name: SEARCH_MEMORY_TOOL_NAME.to_string(),
            success: true,
            payload,
        },
        Err(error) => MemoryToolResult {
            call_id,
            name: SEARCH_MEMORY_TOOL_NAME.to_string(),
            success: false,
            payload: json!({"error": safe_error_payload(&error)}),
        },
    }
}

async fn execute_search_memory_inner(
    operations: Option<&dyn MemoryOperations>,
    authority: &MemorySearchAuthority,
    payload: &Value,
) -> Result<Value, SearchMemoryError> {
    let arguments = parse_arguments(payload)?;
    validate_scope_ids(authority, &arguments)?;
    let operations = operations.ok_or(MemoryOperationError::ServiceUnavailable)?;
    let scope_ids = if arguments.scope_ids.is_empty() {
        trusted_active_scope_ids(authority)
    } else {
        arguments.scope_ids.clone()
    };
    let mut memories = Vec::new();
    for scope_id in scope_ids {
        validate_memory_scope_shape(&scope_id)?;
        let run_id = (scope_id == authority.conversation_scope_id)
            .then(|| authority.conversation_run_id.clone());
        let results = if arguments.query.trim().is_empty() {
            if scope_id != HUMAN_MEMORY_SCOPE_ID {
                return Err(SearchMemoryError::InvalidArguments(format!(
                    "query is required for scope_id: {scope_id}"
                )));
            }
            operations
                .list_memories(ListMemoriesRequest {
                    user_id: HUMAN_MEMORY_SCOPE_ID.to_string(),
                    limit: arguments.limit() as u16,
                })
                .await?
                .results
        } else {
            operations
                .search_memories(SearchMemoriesRequest {
                    query: arguments.query.clone(),
                    user_id: HUMAN_MEMORY_SCOPE_ID.to_string(),
                    agent_id: None,
                    run_id,
                    limit: arguments.limit() as u16,
                })
                .await?
                .results
        };
        for result in results {
            let Some(memory) = result.memory else {
                continue;
            };
            memories.push(json!({
                "id": result.id,
                "kind": "mnemosyne",
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

fn parse_arguments(payload: &Value) -> Result<SearchMemoryArguments, SearchMemoryError> {
    let argument_value = payload.get("arguments").unwrap_or(payload).clone();
    let arguments: SearchMemoryArguments =
        serde_json::from_value(argument_value).map_err(|error| {
            SearchMemoryError::InvalidArguments(format!("invalid arguments: {error}"))
        })?;
    if arguments.query.trim().is_empty() && arguments.scope_ids.is_empty() {
        return Err(SearchMemoryError::InvalidArguments(
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

fn trusted_active_scope_ids(authority: &MemorySearchAuthority) -> Vec<String> {
    let mut ids = authority.active_scope_ids.clone();
    ids.sort();
    ids.dedup();
    ids
}

fn validate_memory_scope_shape(scope_id: &str) -> Result<(), SearchMemoryError> {
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
        Err(SearchMemoryError::InvalidArguments(format!(
            "unsupported scope_id: {scope_id}"
        )))
    } else {
        Ok(())
    }
}

fn validate_scope_ids(
    authority: &MemorySearchAuthority,
    arguments: &SearchMemoryArguments,
) -> Result<(), SearchMemoryError> {
    let trusted = trusted_active_scope_ids(authority)
        .into_iter()
        .collect::<HashSet<_>>();
    for scope_id in &arguments.scope_ids {
        if !trusted.contains(scope_id) {
            return Err(SearchMemoryError::InvalidArguments(format!(
                "unsupported scope_id: {scope_id}"
            )));
        }
    }
    Ok(())
}

fn parse_purpose(value: Option<&str>) -> Result<SearchMemoryPurpose, SearchMemoryError> {
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
        other => Err(SearchMemoryError::InvalidArguments(format!(
            "unsupported purpose: {other}"
        ))),
    }
}

fn safe_error_payload(error: &SearchMemoryError) -> Value {
    match error {
        SearchMemoryError::InvalidArguments(message) => json!(message),
        SearchMemoryError::Operation(error) => json!({
            "code": error.code(),
            "message": error.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::{
        model::{AddMemoryRequest, ListMemoriesResponse, MemoryRecord, SearchMemoriesResponse},
        operations::{MemoryOperationFuture, MemoryServiceReadiness},
    };

    #[derive(Debug, Default)]
    struct FakeOperations {
        searches: Mutex<Vec<SearchMemoriesRequest>>,
    }

    impl MemoryOperations for FakeOperations {
        fn check_readiness(&self) -> MemoryOperationFuture<'_, MemoryServiceReadiness> {
            Box::pin(async { Ok(MemoryServiceReadiness { ready: true }) })
        }

        fn add_memory(&self, _request: AddMemoryRequest) -> MemoryOperationFuture<'_, ()> {
            Box::pin(async { Ok(()) })
        }

        fn search_memories(
            &self,
            request: SearchMemoriesRequest,
        ) -> MemoryOperationFuture<'_, SearchMemoriesResponse> {
            self.searches.lock().expect("search lock").push(request);
            Box::pin(async {
                Ok(SearchMemoriesResponse {
                    results: vec![MemoryRecord {
                        id: "memory_1".to_string(),
                        memory: Some("likes tea".to_string()),
                        score: Some(0.75),
                        metadata: None,
                        created_at: None,
                        updated_at: Some("2026-07-16T00:00:00Z".to_string()),
                    }],
                })
            })
        }

        fn list_memories(
            &self,
            _request: ListMemoriesRequest,
        ) -> MemoryOperationFuture<'_, ListMemoriesResponse> {
            Box::pin(async { Ok(ListMemoriesResponse::default()) })
        }
    }

    fn authority() -> MemorySearchAuthority {
        MemorySearchAuthority::for_conversation("conv_123", ["project:noema".to_string()])
    }

    #[test]
    fn search_memory_tool_spec_matches_runtime_arguments() {
        let spec = search_memory_tool_spec().expect("tool spec");

        assert_eq!(spec.name.as_str(), SEARCH_MEMORY_TOOL_NAME);
        assert_eq!(spec.input_schema.as_value()["required"], json!(["query"]));
        assert_eq!(
            spec.input_schema.as_value()["properties"]["limit"]["maximum"],
            MAX_LIMIT
        );
        let purpose_enum = &spec.input_schema.as_value()["properties"]["purpose"]["enum"];
        assert_eq!(purpose_enum, &json!(SEARCH_MEMORY_PURPOSE_VALUES));
        for purpose in purpose_enum.as_array().expect("purpose enum") {
            parse_purpose(purpose.as_str()).expect("schema purpose accepted");
        }
    }

    #[tokio::test]
    async fn search_uses_only_explicit_runtime_authority() {
        let operations = FakeOperations::default();
        let result = execute_search_memory(
            Some(&operations),
            &authority(),
            Some("call_1".to_string()),
            &json!({
                "query": "preferences",
                "scope_ids": ["conversation:conv_123"]
            }),
        )
        .await;

        assert!(result.success);
        assert_eq!(
            result.payload["memories"][0]["scope_id"],
            "conversation:conv_123"
        );
        let searches = operations.searches.lock().expect("search lock");
        assert_eq!(searches[0].run_id.as_deref(), Some("conv_123"));
    }

    #[tokio::test]
    async fn canonical_conversation_id_is_preserved_as_service_run_id() {
        let operations = FakeOperations::default();
        let authority = MemorySearchAuthority::for_conversation("conversation:conv_123", []);
        execute_search_memory(
            Some(&operations),
            &authority,
            None,
            &json!({
                "query": "preferences",
                "scope_ids": ["conversation:conv_123"]
            }),
        )
        .await;

        let searches = operations.searches.lock().expect("search lock");
        assert_eq!(searches[0].run_id.as_deref(), Some("conversation:conv_123"));
    }

    #[tokio::test]
    async fn out_of_context_scope_fails_before_service_access() {
        let operations = FakeOperations::default();
        let result = execute_search_memory(
            Some(&operations),
            &authority(),
            None,
            &json!({"query": "secret", "scope_ids": ["project:other"]}),
        )
        .await;

        assert!(!result.success);
        assert_eq!(
            result.payload["error"],
            "unsupported scope_id: project:other"
        );
        assert!(operations.searches.lock().expect("search lock").is_empty());
    }

    #[tokio::test]
    async fn absent_service_preserves_safe_unavailable_payload() {
        let result = execute_search_memory(
            None,
            &authority(),
            None,
            &json!({"query": "preferences", "scope_ids": ["human:local"]}),
        )
        .await;

        assert_eq!(result.payload["error"]["code"], "service_unavailable");
        assert_eq!(
            result.payload["error"]["message"],
            "memory service is unavailable"
        );
    }

    #[test]
    fn rejects_unknown_purpose_and_empty_unscoped_query() {
        assert!(matches!(
            parse_arguments(&json!({"query": "notes", "purpose": "dump_everything"})),
            Err(SearchMemoryError::InvalidArguments(message))
                if message == "unsupported purpose: dump_everything"
        ));
        assert!(matches!(
            parse_arguments(&json!({"query": "  "})),
            Err(SearchMemoryError::InvalidArguments(message))
                if message == "query is required unless scope_ids is non-empty"
        ));
    }

    #[test]
    fn parses_nested_payload_clamps_limit_and_accepts_default_purpose() {
        let arguments = parse_arguments(&json!({
            "arguments": {
                "query": "project notes",
                "purpose": "answer_human_question",
                "limit": 99
            }
        }))
        .expect("parse arguments");

        assert_eq!(arguments.query, "project notes");
        assert_eq!(arguments.purpose.as_deref(), Some("answer_human_question"));
        assert_eq!(arguments.limit(), MAX_LIMIT);
    }

    #[test]
    fn memory_scope_shape_accepts_safe_scope_ids() {
        validate_memory_scope_shape("human:local").expect("human scope");
        validate_memory_scope_shape("conversation:abc_123").expect("conversation scope");
    }

    #[test]
    fn memory_scope_shape_rejects_unknown_scope_shape() {
        let error = validate_memory_scope_shape("not allowed").expect_err("invalid scope");
        assert_eq!(error.to_string(), "unsupported scope_id: not allowed");
    }

    #[test]
    fn rejects_empty_query() {
        let error = parse_arguments(&json!({"query": "   "})).expect_err("empty query rejected");
        assert_eq!(
            error.to_string(),
            "query is required unless scope_ids is non-empty"
        );
    }

    #[test]
    fn parses_empty_query_when_scope_ids_are_present() {
        let arguments = parse_arguments(&json!({
            "arguments": {
                "scope_ids": ["human:local"],
                "query": "",
                "purpose": "answer_human_question"
            }
        }))
        .expect("parse scoped empty query");

        assert_eq!(arguments.query, "");
        assert_eq!(arguments.scope_ids, vec!["human:local"]);
    }

    #[test]
    fn rejects_empty_query_without_scope_ids() {
        let error = parse_arguments(&json!({
            "arguments": {"query": "   "}
        }))
        .expect_err("empty unscoped query rejected");

        assert_eq!(
            error.to_string(),
            "query is required unless scope_ids is non-empty"
        );
    }

    #[test]
    fn validates_scope_ids_against_trusted_active_ids() {
        let arguments = SearchMemoryArguments {
            query: String::new(),
            scope_ids: vec![HUMAN_MEMORY_SCOPE_ID.to_string()],
            purpose: Some("answer_human_question".to_string()),
            limit: None,
        };

        validate_scope_ids(&authority(), &arguments).expect("trusted scope id");
    }

    #[test]
    fn rejects_out_of_context_scope_ids() {
        let arguments = SearchMemoryArguments {
            query: String::new(),
            scope_ids: vec!["project:other".to_string()],
            purpose: Some("answer_human_question".to_string()),
            limit: None,
        };

        let error = validate_scope_ids(&authority(), &arguments).expect_err("invalid scope id");
        assert_eq!(error.to_string(), "unsupported scope_id: project:other");
    }

    #[test]
    fn rejects_unknown_purpose() {
        let error = parse_arguments(&json!({
            "query": "project notes",
            "purpose": "dump_everything"
        }))
        .expect_err("unknown purpose rejected");

        assert_eq!(error.to_string(), "unsupported purpose: dump_everything");
    }

    #[test]
    fn natural_language_memory_phrases_do_not_unlock_additional_scopes() {
        let authority = MemorySearchAuthority::for_conversation("conv_123", []);

        assert_eq!(
            authority.active_scope_ids(),
            ["conversation:conv_123", "human:local"]
        );
    }

    #[test]
    fn trusted_scope_ids_preserve_canonical_conversation_ids() {
        let authority = MemorySearchAuthority::for_conversation("conversation:conv_123", []);

        assert_eq!(
            authority.active_scope_ids(),
            ["conversation:conv_123", "human:local"]
        );
    }

    #[test]
    fn validated_scope_args_become_trusted_active_objects() {
        let arguments = SearchMemoryArguments {
            query: String::new(),
            scope_ids: vec![HUMAN_MEMORY_SCOPE_ID.to_string()],
            purpose: Some("answer_human_question".to_string()),
            limit: None,
        };

        validate_scope_ids(&authority(), &arguments).expect("validated scope");
    }

    #[test]
    fn purpose_argument_is_validated_but_not_used_as_scope_authority() {
        assert_eq!(
            parse_purpose(Some("external_action")).expect("valid purpose"),
            SearchMemoryPurpose::ExternalAction
        );
    }
}
