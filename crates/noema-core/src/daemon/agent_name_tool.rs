#![allow(dead_code)]

use crate::{NoemaStore, store::StoreError};

use serde::Deserialize;
use serde_json::{Value, json};

pub(super) const UPDATE_OWN_NAME_TOOL: &str = "update_own_name";
pub(super) const MAX_AGENT_NAME_CHARS: usize = 80;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AgentNameToolRuntimeContext {
    pub agent_id: String,
    pub user_input: String,
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
    if !user_explicitly_names_agent(&context.user_input, &arguments.name) {
        return Err(AgentNameToolError::InvalidArguments(
            "user did not explicitly instruct this agent rename".to_string(),
        ));
    }

    let updated = store
        .update_agent_display_name(&context.agent_id, &arguments.name)
        .await?;

    Ok(json!({
        "agent_id": updated.agent_id,
        "display_name": updated.display_name,
    }))
}

fn parse_arguments(payload: &Value) -> Result<UpdateOwnNameArguments, AgentNameToolError> {
    let argument_value = payload.get("arguments").unwrap_or(payload).clone();
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

fn user_explicitly_names_agent(user_input: &str, name: &str) -> bool {
    let input = normalize_for_name_match(user_input);
    let name = normalize_for_name_match(name);
    if name.is_empty() {
        return false;
    }

    let explicit_phrases = [
        format!("your name is {name}"),
        format!("call yourself {name}"),
        format!("rename yourself to {name}"),
        format!("i want to call you {name}"),
    ];
    explicit_phrases
        .iter()
        .any(|phrase| contains_normalized_phrase(&input, phrase))
}

fn normalize_for_name_match(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .map(|ch| if ch.is_alphanumeric() { ch } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn contains_normalized_phrase(input: &str, phrase: &str) -> bool {
    input == phrase
        || input
            .strip_prefix(phrase)
            .is_some_and(|remaining| remaining.starts_with(' '))
        || input
            .strip_suffix(phrase)
            .is_some_and(|remaining| remaining.ends_with(' '))
        || input.contains(&format!(" {phrase} "))
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

    #[test]
    fn accepts_explicit_name_instructions() {
        let examples = [
            ("Your name is Mira.", "Mira"),
            ("Call yourself Orin.", "Orin"),
            ("Rename yourself to Halcyon.", "Halcyon"),
            ("I want to call you Tess.", "Tess"),
        ];

        for (input, name) in examples {
            assert!(
                user_explicitly_names_agent(input, name),
                "{input:?} should explicitly name the agent {name:?}"
            );
        }
    }

    #[test]
    fn rejects_ambiguous_name_mentions() {
        let examples = [
            ("What name do you like?", "Mira"),
            ("Maybe you could be Mira?", "Mira"),
            ("Mira is a nice name.", "Mira"),
        ];

        for (input, name) in examples {
            assert!(
                !user_explicitly_names_agent(input, name),
                "{input:?} should not explicitly name the agent {name:?}"
            );
        }
    }

    #[tokio::test]
    async fn store_backed_success_updates_agent_display_name() {
        let store = test_store().await;
        store.ensure_default_actors().await.expect("actors");
        let context = AgentNameToolRuntimeContext {
            agent_id: "agent:primary".to_string(),
            user_input: "Your name is Mira.".to_string(),
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
    async fn rejects_call_without_explicit_user_instruction() {
        let store = test_store().await;
        store.ensure_default_actors().await.expect("actors");
        let context = AgentNameToolRuntimeContext {
            agent_id: "agent:primary".to_string(),
            user_input: "Maybe you could be Mira?".to_string(),
        };

        let result = execute_update_own_name(
            &store,
            &context,
            Some("call_2".to_string()),
            &json!({"name": "Mira"}),
        )
        .await;

        assert!(!result.success);
        assert_eq!(result.call_id.as_deref(), Some("call_2"));
        assert_eq!(
            result.payload,
            json!({
                "error": "user did not explicitly instruct this agent rename"
            })
        );
        let agent = store
            .get_agent("agent:primary")
            .await
            .expect("get agent")
            .expect("agent exists");
        assert_eq!(agent.display_name, None);
    }

    #[test]
    fn matches_update_own_name_tool_exactly() {
        assert!(is_update_own_name_tool("update_own_name"));
        assert!(!is_update_own_name_tool(" update_own_name"));
        assert!(!is_update_own_name_tool("update_own_name_v2"));
    }

    async fn test_store() -> crate::NoemaStore {
        let home = tempfile::tempdir().expect("temp noema home");
        let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
        let config = crate::StoreConfig::from_paths(&paths);
        let store = crate::NoemaStore::open(&config).await.expect("open store");
        std::mem::forget(home);
        store
    }
}
