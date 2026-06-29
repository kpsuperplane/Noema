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
            "name update requires explicit user instruction".to_string(),
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

fn user_explicitly_names_agent(user_input: &str, name: &str) -> bool {
    let input = user_input.trim_start();
    let name = name.trim();
    if input.is_empty() || name.is_empty() {
        return false;
    }

    const EXPLICIT_PREFIXES: &[&str] = &[
        "your name is ",
        "call yourself ",
        "rename yourself to ",
        "i want to call you ",
        "i'll call you ",
        "i’ll call you ",
    ];

    EXPLICIT_PREFIXES
        .iter()
        .filter_map(|prefix| strip_prefix_case_insensitive(input, prefix))
        .any(|candidate_name| candidate_name_matches(candidate_name, name))
}

fn strip_prefix_case_insensitive<'a>(value: &'a str, prefix: &str) -> Option<&'a str> {
    let end = matching_prefix_end(value, prefix)?;
    Some(&value[end..])
}

fn candidate_name_matches(candidate: &str, name: &str) -> bool {
    let candidate = candidate.trim_start();
    let name = name.trim();
    if !payload_name_has_safe_punctuation(name) {
        return false;
    }
    let Some(remaining) = strip_prefix_case_insensitive(candidate, name) else {
        return false;
    };
    remaining_after_explicit_name_is_safe(remaining)
}

fn payload_name_has_safe_punctuation(name: &str) -> bool {
    if name.ends_with(['.', '!', '?']) {
        return false;
    }

    name.split(". ")
        .skip(1)
        .all(|part| part.split_whitespace().count() == 1)
}

fn matching_prefix_end(value: &str, prefix: &str) -> Option<usize> {
    let mut value_chars = value.char_indices();
    let mut end = 0;
    for prefix_ch in prefix.chars() {
        let (idx, value_ch) = value_chars.next()?;
        if !chars_equal_ignore_case(value_ch, prefix_ch) {
            return None;
        }
        end = idx + value_ch.len_utf8();
    }
    Some(end)
}

fn chars_equal_ignore_case(left: char, right: char) -> bool {
    left.to_lowercase().to_string() == right.to_lowercase().to_string()
}

fn remaining_after_explicit_name_is_safe(remaining: &str) -> bool {
    let remaining = remaining.trim_start();
    if remaining.is_empty() {
        return true;
    }

    if let Some(after_period) = remaining.strip_prefix('.') {
        let after_period = after_period.trim_start();
        return after_period.is_empty()
            || (starts_with_uppercase(after_period)
                && after_period.split_whitespace().count() > 1);
    }

    false
}

fn starts_with_uppercase(value: &str) -> bool {
    value.chars().next().is_some_and(|ch| ch.is_uppercase())
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

    #[test]
    fn accepts_explicit_name_instructions() {
        let examples = [
            ("Your name is Mira.", "Mira"),
            ("Call yourself Orin.", "Orin"),
            ("Rename yourself to Halcyon.", "Halcyon"),
            ("I want to call you Tess.", "Tess"),
            ("I'll call you Mira.", "Mira"),
            ("I’ll call you Mira.", "Mira"),
            ("Your name is Mira. Please say hi.", "Mira"),
            ("Your name is Dr. Nova.", "Dr. Nova"),
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

    #[test]
    fn rejects_negated_narrated_and_quoted_name_mentions() {
        let examples = [
            ("Don't call yourself Mira.", "Mira"),
            ("Do not rename yourself to Mira.", "Mira"),
            ("I heard your name is Mira.", "Mira"),
            (r#"The phrase "your name is Mira" is in my prompt."#, "Mira"),
        ];

        for (input, name) in examples {
            assert!(
                !user_explicitly_names_agent(input, name),
                "{input:?} should not explicitly name the agent {name:?}"
            );
        }
    }

    #[test]
    fn rejects_material_name_mismatches_after_normalization() {
        let examples = [
            ("Your name is C.", "C++"),
            ("Your name is Mira.", "Mira!!!"),
            ("Your name is Mira.", "Mira."),
            ("Your name is Mira?", "Mira"),
            ("Your name is Mira. Please say hi.", "Mira. Please say hi"),
            ("Your name is Dr. Nova.", "Dr"),
        ];

        for (input, name) in examples {
            assert!(
                !user_explicitly_names_agent(input, name),
                "{input:?} should not authorize materially different payload name {name:?}"
            );
        }
    }

    #[tokio::test]
    async fn store_backed_success_updates_agent_display_name() {
        let (_home, store) = test_store().await;
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
        let (_home, store) = test_store().await;
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
                "error": "name update requires explicit user instruction"
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

    async fn test_store() -> (tempfile::TempDir, crate::NoemaStore) {
        let home = tempfile::tempdir().expect("temp noema home");
        let paths = crate::NoemaPaths::from_noema_home(home.path()).expect("paths");
        let config = crate::StoreConfig::from_paths(&paths);
        let store = crate::NoemaStore::open(&config).await.expect("open store");
        (home, store)
    }
}
