use std::path::PathBuf;

use crate::{
    FoundationLocalProviderConfig, GenerateInput, GenerateInputItem, GenerateMessage,
    GenerateMessageRole, GenerateResponse, GenerateToolCall, GenerateToolCallInput,
    GenerateToolResultInput, ModelProvider, ProviderError, ProviderResponseContinuation,
    ProviderToolSchemaDialect, ProviderToolTransport,
};

use super::super::{
    FoundationLocalProvider,
    bridge::{
        BridgeReplayTurn, BridgeRole, default_bridge_package_path, default_development_bridge_path,
    },
    lowering::{bridge_replay_response, foundation_prompt_parts},
};

fn test_provider(profile: &str, bridge_path: Option<PathBuf>) -> FoundationLocalProvider {
    FoundationLocalProvider::new(FoundationLocalProviderConfig {
        default_profile: profile.to_string(),
        bridge_path,
        system_errors: None,
    })
    .expect("provider")
}

#[tokio::test]
async fn missing_configured_bridge_fails_without_path_configuration_error() {
    let bridge_path = std::env::temp_dir().join(format!(
        "missing-noema-foundation-bridge-{}",
        std::process::id()
    ));
    let error = test_provider("default", Some(bridge_path))
        .generate(crate::GenerateRequest::text("hello"))
        .await
        .expect_err("missing bridge");

    let ProviderError::ProviderUnavailable { message, .. } = error else {
        panic!("expected provider-unavailable bridge failure: {error:?}");
    };
    assert!(!message.contains("bridge path is not configured"));
}

#[test]
fn foundation_local_advertises_context_window_metadata() {
    let metadata = test_provider("default", None).context_metadata(Some("default"));
    assert_eq!(metadata.context_window_tokens, Some(4_096));
    assert_eq!(metadata.default_output_reserve_tokens, Some(512));
}

#[test]
fn foundation_local_advertises_native_tool_transport() {
    let capabilities = test_provider("default", None).tool_capabilities(Some("default"));
    assert_eq!(capabilities.tool_transport, ProviderToolTransport::Native);
    assert!(!capabilities.parallel_tool_calls);
    assert!(!capabilities.tool_choice);
    assert!(!capabilities.allowed_tools);
    assert!(capabilities.native_tool_results);
    assert_eq!(
        capabilities.schema_dialect,
        ProviderToolSchemaDialect::FoundationLocal
    );
    assert_eq!(
        test_provider("default", None).response_continuation(Some("default")),
        ProviderResponseContinuation::ActiveSession
    );
}

#[test]
fn foundation_local_tool_classification_default_uses_provider_profile() {
    assert_eq!(
        test_provider("foundation-live", None)
            .default_tool_classification_model()
            .as_deref(),
        Some("foundation-live")
    );
}

#[test]
fn default_macos_debug_bridge_config_materializes_source_tree_bridge() {
    let config = test_provider("default", None).bridge_config();
    if cfg!(target_os = "macos") && cfg!(debug_assertions) {
        assert_eq!(config.bridge_path, default_development_bridge_path());
        let build = config.build.expect("debug macOS source build");
        assert_eq!(build.package_path, default_bridge_package_path());
        assert_eq!(build.swift_executable, PathBuf::from("swift"));
    } else {
        assert!(config.build.is_none());
    }
}

#[test]
fn configured_bridge_path_is_not_auto_materialized() {
    let bridge_path = PathBuf::from("/tmp/noema-foundation-bridge");
    let config = test_provider("default", Some(bridge_path.clone())).bridge_config();
    assert_eq!(config.bridge_path, bridge_path);
    assert!(config.build.is_none());
}

#[test]
fn message_prompt_replays_prior_turns_and_generates_from_latest_user_message() {
    let prompt = foundation_prompt_parts(&GenerateInput::Messages(vec![
        GenerateMessage {
            role: GenerateMessageRole::User,
            content: "first question".to_string(),
        },
        GenerateMessage {
            role: GenerateMessageRole::Assistant,
            content: "first answer".to_string(),
        },
        GenerateMessage {
            role: GenerateMessageRole::User,
            content: "second question".to_string(),
        },
    ]));

    assert_eq!(
        prompt.replay_turns,
        vec![
            BridgeReplayTurn {
                role: BridgeRole::User,
                text: "first question".to_string(),
                tool_call: None,
                tool_result: None,
            },
            BridgeReplayTurn {
                role: BridgeRole::Assistant,
                text: "first answer".to_string(),
                tool_call: None,
                tool_result: None,
            },
        ]
    );
    assert_eq!(prompt.generate_input, "second question");

    let recovery = foundation_prompt_parts(&GenerateInput::Items(vec![
        GenerateInputItem::Message(GenerateMessage {
            role: GenerateMessageRole::User,
            content: "choose".to_string(),
        }),
        GenerateInputItem::ToolCall(GenerateToolCallInput {
            id: None,
            call_id: "call_1".to_string(),
            name: "choose".to_string(),
            provider_name: None,
            arguments: serde_json::json!({}),
        }),
        GenerateInputItem::ToolResult(GenerateToolResultInput {
            id: None,
            call_id: "call_1".to_string(),
            name: "choose".to_string(),
            provider_name: None,
            arguments: serde_json::json!({}),
            success: true,
            payload: serde_json::json!({"selected": "yes"}),
        }),
    ]));
    assert_eq!(recovery.replay_turns.len(), 3);
    assert!(recovery.generate_input.is_empty());
}

#[test]
fn developer_context_replays_as_application_context() {
    let prompt = foundation_prompt_parts(&GenerateInput::Messages(vec![
        GenerateMessage {
            role: GenerateMessageRole::User,
            content: "what day is it?".to_string(),
        },
        GenerateMessage {
            role: GenerateMessageRole::Developer,
            content: "runtime date: 2026-07-15".to_string(),
        },
    ]));

    assert_eq!(
        prompt.replay_turns,
        vec![BridgeReplayTurn {
            role: BridgeRole::ApplicationContext,
            text: "runtime date: 2026-07-15".to_string(),
            tool_call: None,
            tool_result: None,
        }]
    );
    assert_eq!(prompt.generate_input, "what day is it?");
}

#[test]
fn native_response_replay_preserves_text_and_correlated_tool_calls() {
    let mut response = GenerateResponse::final_text("  bridge answer \n", "test", "default");
    response.tool_calls.push(GenerateToolCall {
        id: Some("item_1".to_string()),
        provider_call_id: Some("call_1".to_string()),
        provider_name: Some("search_memory".to_string()),
        name: "search_memory".to_string(),
        payload: serde_json::json!({ "query": "cache" }),
    });
    let turns = bridge_replay_response(&response);

    assert_eq!(turns.len(), 2);
    assert_eq!(turns[0].role, BridgeRole::Assistant);
    assert_eq!(turns[0].text, "bridge answer");
    assert_eq!(turns[1].role, BridgeRole::Assistant);
    assert_eq!(turns[1].text, "");
    assert_eq!(turns[1].tool_call.as_ref().unwrap().call_id, "call_1");
    assert_eq!(
        turns[1].tool_call.as_ref().unwrap().tool_name,
        "search_memory"
    );
}
