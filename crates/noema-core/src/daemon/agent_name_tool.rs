use crate::{NoemaStore, store::StoreError};
use noema_capabilities::{ToolContractError, ToolSpec};

use serde::Deserialize;
use serde_json::{Value, json};

pub(super) const UPDATE_OWN_NAME_TOOL: &str = "update_own_name";
pub(super) const MAX_AGENT_NAME_CHARS: usize = 80;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AgentNameToolRuntimeContext {
    pub agent_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AgentNameToolResult {
    pub call_id: Option<String>,
    pub name: String,
    pub success: bool,
    pub payload: Value,
}

#[derive(Debug, thiserror::Error)]
pub(super) enum AgentNameToolError {
    #[error("{0}")]
    InvalidArguments(String),
    #[error(transparent)]
    Store(#[from] StoreError),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct UpdateOwnNameArguments {
    pub name: String,
}

pub(super) fn is_update_own_name_tool(name: &str) -> bool {
    name == UPDATE_OWN_NAME_TOOL
}

pub(super) fn update_own_name_tool_spec() -> Result<ToolSpec, ToolContractError> {
    ToolSpec::new(
        UPDATE_OWN_NAME_TOOL,
        "Persist the primary agent display name when the current user explicitly asks to name or rename the agent.",
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "minLength": 1,
                    "maxLength": MAX_AGENT_NAME_CHARS,
                    "pattern": ".*\\S.*",
                    "description": "The agent display name requested by the current user."
                }
            },
            "required": ["name"],
            "additionalProperties": false
        }),
    )
}

pub(super) async fn execute_update_own_name(
    store: &NoemaStore,
    context: &AgentNameToolRuntimeContext,
    call_id: Option<String>,
    payload: &Value,
) -> AgentNameToolResult {
    match execute_update_own_name_inner(store, context, payload).await {
        Ok(payload) => AgentNameToolResult {
            call_id,
            name: UPDATE_OWN_NAME_TOOL.to_string(),
            success: true,
            payload,
        },
        Err(error) => AgentNameToolResult {
            call_id,
            name: UPDATE_OWN_NAME_TOOL.to_string(),
            success: false,
            payload: json!({
                "error": safe_error_message(&error),
            }),
        },
    }
}

pub(super) async fn execute_update_own_name_inner(
    store: &NoemaStore,
    context: &AgentNameToolRuntimeContext,
    payload: &Value,
) -> Result<Value, AgentNameToolError> {
    let arguments = parse_arguments(payload)?;
    // The provider tool call is the structured intent signal; do not re-parse
    // user text here with language-specific string matching.
    let updated = store
        .update_agent_display_name(&context.agent_id, &arguments.name)
        .await?;

    Ok(json!({
        "agent_id": updated.agent_id,
        "display_name": updated.display_name,
    }))
}

fn parse_arguments(payload: &Value) -> Result<UpdateOwnNameArguments, AgentNameToolError> {
    let argument_value = if let Some(arguments) = payload.get("arguments") {
        reject_nested_outer_fields(payload)?;
        arguments.clone()
    } else {
        payload.clone()
    };
    let mut arguments: UpdateOwnNameArguments =
        serde_json::from_value(argument_value).map_err(|error| {
            AgentNameToolError::InvalidArguments(format!("invalid arguments: {error}"))
        })?;
    arguments.name = arguments.name.trim().to_string();
    if arguments.name.is_empty() {
        return Err(AgentNameToolError::InvalidArguments(
            "name is required".to_string(),
        ));
    }
    if arguments.name.chars().count() > MAX_AGENT_NAME_CHARS {
        return Err(AgentNameToolError::InvalidArguments(
            "name must be 80 characters or fewer".to_string(),
        ));
    }
    Ok(arguments)
}

fn reject_nested_outer_fields(payload: &Value) -> Result<(), AgentNameToolError> {
    let Some(object) = payload.as_object() else {
        return Ok(());
    };
    if object.keys().all(|key| key == "arguments") {
        Ok(())
    } else {
        Err(AgentNameToolError::InvalidArguments(
            "nested arguments payload cannot include outer fields".to_string(),
        ))
    }
}

fn safe_error_message(error: &AgentNameToolError) -> String {
    match error {
        AgentNameToolError::InvalidArguments(message) => message.clone(),
        AgentNameToolError::Store(_) => "agent name update failed".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn parses_nested_name_args_and_trims() {
        let payload = json!({
            "arguments": {
                "name": "  Mira  "
            }
        });

        let arguments = parse_arguments(&payload).expect("parse arguments");

        assert_eq!(arguments.name, "Mira");
    }

    #[test]
    fn update_own_name_tool_spec_matches_runtime_arguments() {
        let spec = update_own_name_tool_spec().expect("tool spec");

        assert_eq!(spec.name.as_str(), UPDATE_OWN_NAME_TOOL);
        assert!(spec.description.contains("name or rename"));
        assert_eq!(spec.input_schema.as_value()["required"], json!(["name"]));
        assert_eq!(
            spec.input_schema.as_value()["properties"]["name"]["maxLength"],
            MAX_AGENT_NAME_CHARS
        );
        assert_eq!(
            spec.input_schema.as_value()["properties"]["name"]["pattern"],
            r".*\S.*"
        );
    }

    #[test]
    fn rejects_nested_arguments_with_outer_targeting_fields() {
        let payload = json!({
            "arguments": {
                "name": "Mira"
            },
            "agent_id": "agent:other"
        });

        let error = parse_arguments(&payload).expect_err("outer targeting field rejected");

        assert_eq!(
            safe_error_message(&error),
            "nested arguments payload cannot include outer fields"
        );
    }

    #[test]
    fn rejects_empty_name() {
        let payload = json!({
            "name": "   "
        });

        let error = parse_arguments(&payload).expect_err("empty name rejected");

        assert_eq!(safe_error_message(&error), "name is required");
    }

    #[test]
    fn rejects_overlong_name() {
        let payload = json!({
            "name": "a".repeat(MAX_AGENT_NAME_CHARS + 1)
        });

        let error = parse_arguments(&payload).expect_err("overlong name rejected");

        assert_eq!(
            safe_error_message(&error),
            "name must be 80 characters or fewer"
        );
    }

    #[tokio::test]
    async fn store_backed_success_updates_agent_display_name() {
        let (_home, store) = test_store().await;
        store.ensure_default_actors().await.expect("actors");
        let context = AgentNameToolRuntimeContext {
            agent_id: "agent:primary".to_string(),
        };

        let result = execute_update_own_name(
            &store,
            &context,
            Some("call_1".to_string()),
            &json!({"name": " Mira "}),
        )
        .await;

        assert!(result.success);
        assert_eq!(result.call_id.as_deref(), Some("call_1"));
        assert_eq!(result.name, UPDATE_OWN_NAME_TOOL);
        assert_eq!(
            result.payload,
            json!({
                "agent_id": "agent:primary",
                "display_name": "Mira"
            })
        );
        let agent = store
            .get_agent("agent:primary")
            .await
            .expect("get agent")
            .expect("agent exists");
        assert_eq!(agent.display_name.as_deref(), Some("Mira"));
    }

    #[tokio::test]
    async fn store_backed_structured_name_tool_accepts_onboarding_answer() {
        let (_home, store) = test_store().await;
        store.ensure_default_actors().await.expect("actors");
        let context = AgentNameToolRuntimeContext {
            agent_id: "agent:primary".to_string(),
        };

        let result = execute_update_own_name(
            &store,
            &context,
            Some("call_name_1".to_string()),
            &json!({"name": "Fred"}),
        )
        .await;

        assert!(result.success);
        assert_eq!(
            result.payload,
            json!({
                "agent_id": "agent:primary",
                "display_name": "Fred"
            })
        );
        let agent = store
            .get_agent("agent:primary")
            .await
            .expect("get agent")
            .expect("agent exists");
        assert_eq!(agent.display_name.as_deref(), Some("Fred"));
    }

    #[test]
    fn matches_update_own_name_tool_exactly() {
        assert!(is_update_own_name_tool("update_own_name"));
        assert!(!is_update_own_name_tool(" update_own_name"));
        assert!(!is_update_own_name_tool("update_own_name_v2"));
    }

    async fn test_store() -> (tempfile::TempDir, crate::NoemaStore) {
        let home = tempfile::tempdir().expect("temp noema home");
        let paths = noema_home::NoemaPaths::from_noema_home(home.path()).expect("paths");
        let store = crate::test_support::test_store_for_paths(&paths).await;
        (home, store)
    }
}
