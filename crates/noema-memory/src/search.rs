//! Stable governed `search_memory` capability semantics.

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
        authority.active_scope_ids.clone()
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
    validate_purpose(arguments.purpose.as_deref())?;
    Ok(arguments)
}

impl SearchMemoryArguments {
    fn limit(&self) -> usize {
        self.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT)
    }
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
    for scope_id in &arguments.scope_ids {
        if authority.active_scope_ids.binary_search(scope_id).is_err() {
            return Err(SearchMemoryError::InvalidArguments(format!(
                "unsupported scope_id: {scope_id}"
            )));
        }
    }
    Ok(())
}

fn validate_purpose(value: Option<&str>) -> Result<(), SearchMemoryError> {
    let value = value.unwrap_or("answer_human_question");
    if SEARCH_MEMORY_PURPOSE_VALUES.contains(&value) {
        Ok(())
    } else {
        Err(SearchMemoryError::InvalidArguments(format!(
            "unsupported purpose: {value}"
        )))
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
        model::{AddMemoryRequest, ListMemoriesResponse, SearchMemoriesResponse},
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
                serde_json::from_value(json!({"results": [
                    {"id": "missing_text", "score": 0.99, "metadata": {"private": true}},
                    {"id": "memory_1", "memory": "likes tea", "score": 0.75,
                     "metadata": {"private": true}, "updated_at": "2026-07-16T00:00:00Z"}
                ]}))
                .map_err(|_| MemoryOperationError::UnreadableResponse)
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
            validate_purpose(purpose.as_str()).expect("schema purpose accepted");
        }
    }

    #[tokio::test]
    async fn search_enforces_authority_and_preserves_canonical_conversation_run_ids() {
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
        assert_eq!(result.payload["memories"].as_array().unwrap().len(), 1);
        assert!(result.payload["memories"][0].get("metadata").is_none());
        assert_eq!(
            operations.searches.lock().expect("search lock")[0]
                .run_id
                .as_deref(),
            Some("conv_123")
        );
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

        assert_eq!(
            operations.searches.lock().expect("search lock")[1]
                .run_id
                .as_deref(),
            Some("conversation:conv_123")
        );
    }

    #[tokio::test]
    async fn invalid_scope_fails_before_service_access_and_absence_uses_safe_error() {
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
    fn argument_parser_enforces_purpose_scope_query_shape_and_limit() {
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
        validate_memory_scope_shape("human:local").expect("human scope");
        validate_memory_scope_shape("conversation:abc_123").expect("conversation scope");
        let error = validate_memory_scope_shape("not allowed").expect_err("invalid scope");
        assert_eq!(error.to_string(), "unsupported scope_id: not allowed");
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
}
