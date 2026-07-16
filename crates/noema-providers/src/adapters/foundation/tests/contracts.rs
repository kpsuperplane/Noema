use std::path::PathBuf;

use crate::{
    AssistantTextPhase, FoundationLocalProviderConfig, GenerateInput, GenerateMessage,
    GenerateMessageRole, GenerateResponseItem, GenerateResponseStatus, GenerateToolCall,
    ModelProvider, ParsedNoemaResponse, ProviderError, ProviderToolSchemaDialect,
    ProviderToolTransport,
};

use super::super::{
    FoundationLocalProvider,
    bridge::{
        BridgeReplayTurn, BridgeRole, default_bridge_package_path, default_development_bridge_path,
    },
    lowering::{bridge_replay_parsed_response, foundation_prompt_parts},
};

#[tokio::test]
async fn missing_configured_bridge_fails_without_path_configuration_error() {
    let bridge_path = std::env::temp_dir().join(format!(
        "missing-noema-foundation-bridge-{}",
        std::process::id()
    ));
    let provider = FoundationLocalProvider::new(FoundationLocalProviderConfig {
        default_profile: "default".to_string(),
        bridge_path: Some(bridge_path),
        system_errors: None,
    })
    .expect("provider");

    let error = provider
        .generate(crate::GenerateRequest::text("hello"))
        .await
        .expect_err("stub should be unavailable");

    assert!(matches!(error, ProviderError::ProviderUnavailable { .. }));
    let ProviderError::ProviderUnavailable { message, .. } = error else {
        unreachable!("matched provider unavailable above");
    };
    assert!(
        !message.contains("bridge path is not configured"),
        "provider should report bridge launch/materialization errors directly: {message}"
    );
}

#[test]
fn foundation_local_advertises_context_window_metadata() {
    let provider = test_provider("default", None);

    let metadata = provider.context_metadata(Some("default"));

    assert_eq!(metadata.context_window_tokens, Some(4_096));
    assert_eq!(metadata.default_output_reserve_tokens, Some(512));
}

#[test]
fn foundation_local_advertises_noema_envelope_tool_transport() {
    let provider = test_provider("default", None);

    let capabilities = provider.tool_capabilities(Some("default"));

    assert_eq!(
        capabilities.tool_transport,
        ProviderToolTransport::NoemaEnvelope
    );
    assert!(!capabilities.parallel_tool_calls);
    assert!(!capabilities.tool_choice);
    assert!(!capabilities.native_tool_results);
    assert_eq!(capabilities.schema_dialect, ProviderToolSchemaDialect::None);
}

#[test]
fn foundation_local_tool_classification_default_uses_provider_profile() {
    let provider = test_provider("foundation-live", None);

    assert_eq!(
        provider.default_tool_classification_model().as_deref(),
        Some("foundation-live")
    );
}

#[test]
fn default_macos_debug_bridge_config_materializes_source_tree_bridge() {
    let provider = test_provider("default", None);

    let config = provider.bridge_config();

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
    let provider = test_provider("default", Some(bridge_path.clone()));

    let config = provider.bridge_config();

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
            },
            BridgeReplayTurn {
                role: BridgeRole::Assistant,
                text: "first answer".to_string(),
            },
        ]
    );
    assert_eq!(prompt.generate_input, "second question");
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
        }]
    );
    assert_eq!(prompt.generate_input, "what day is it?");
}

#[test]
fn parsed_response_replay_normalizes_text_and_tracks_structured_output() {
    let turns = bridge_replay_parsed_response(&ParsedNoemaResponse {
        responses: vec![
            GenerateResponseItem::Text {
                phase: Some(AssistantTextPhase::FinalAnswer),
                text: "  bridge answer \n".to_string(),
            },
            GenerateResponseItem::Structured {
                schema: "noema.test".to_string(),
                payload: serde_json::json!({ "value": 1 }),
            },
        ],
        tool_calls: vec![GenerateToolCall {
            id: None,
            provider_call_id: None,
            provider_name: None,
            name: "search_memory".to_string(),
            payload: serde_json::json!({ "query": "cache" }),
        }],
        response_status: GenerateResponseStatus::Final,
    });

    assert_eq!(turns.len(), 3);
    assert_eq!(turns[0].role, BridgeRole::Assistant);
    assert_eq!(turns[0].text, "bridge answer");
    assert_eq!(turns[1].role, BridgeRole::Assistant);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&turns[1].text).expect("structured marker"),
        serde_json::json!({
            "kind": "structured",
            "schema": "noema.test",
            "payload": { "value": 1 },
        })
    );
    assert_eq!(turns[2].role, BridgeRole::Assistant);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&turns[2].text).expect("tool-call marker"),
        serde_json::json!({
            "kind": "tool_call_without_correlation_id",
            "name": "search_memory",
            "provider_name": null,
            "payload": { "query": "cache" },
        })
    );
}

fn test_provider(default_profile: &str, bridge_path: Option<PathBuf>) -> FoundationLocalProvider {
    FoundationLocalProvider::new(FoundationLocalProviderConfig {
        default_profile: default_profile.to_string(),
        bridge_path,
        system_errors: None,
    })
    .expect("provider")
}
