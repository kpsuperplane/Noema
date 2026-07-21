use crate::{
    AssistantTextPhase, FoundationLocalProviderConfig, GenerateInput, GenerateOptions,
    GenerateRequest, GenerateResponseItem, GenerateResponseStatus, GenerateStreamEvent,
    ModelProvider, ProviderToolTransport,
};

use super::{
    super::FoundationLocalProvider,
    support::{bridge_script, healthy_bridge_script},
};

#[tokio::test]
async fn generate_required_noema_response_parses_bridge_object() {
    let (_dir, bridge_path) = bridge_script(&healthy_bridge_script(
        r#"
    *'"id":"create_session"'*) printf '%s\n' '{"id":"create_session","payload":{"type":"session_created","session_id":"session-1"}}' ;;
    *'"id":"generate"'*) printf '%s\n' '{"id":"generate","payload":{"type":"generate_complete","text":"{\"response_status\":\"final\",\"responses\":[{\"kind\":\"text\",\"phase\":\"final_answer\",\"text\":\"bridge answer\"}],\"tool_calls\":[]}"}}' ;;
"#,
    ));
    let provider = provider(bridge_path);

    let response = provider
        .generate(required_response_request())
        .await
        .expect("generate");

    assert_eq!(
        response.responses,
        vec![GenerateResponseItem::Text {
            phase: Some(AssistantTextPhase::FinalAnswer),
            text: "bridge answer".to_string(),
        }]
    );
    assert_eq!(response.response_status, GenerateResponseStatus::Final);
}

#[tokio::test]
async fn generate_required_noema_response_streams_only_assistant_text_from_object() {
    let (_dir, bridge_path) = bridge_script(&healthy_bridge_script(
        r#"
    *'"id":"create_session"'*) printf '%s\n' '{"id":"create_session","payload":{"type":"session_created","session_id":"session-1"}}' ;;
    *'"id":"generate"'*) printf '%s\n' '{"id":"generate","payload":{"type":"assistant_text_delta","delta":"{\"response_status\":\"final\",\"responses\":[{\"kind\":\"text\",\"phase\":\"final_answer\",\"text\":\"bridge answer\"}],\"tool_calls\":[]}"}}'; printf '%s\n' '{"id":"generate","payload":{"type":"generate_complete","text":"{\"response_status\":\"final\",\"responses\":[{\"kind\":\"text\",\"phase\":\"final_answer\",\"text\":\"bridge answer\"}],\"tool_calls\":[]}"}}' ;;
"#,
    ));
    let provider = provider(bridge_path);
    let mut events = Vec::new();

    let response = provider
        .generate_streaming(required_response_request(), &mut |event| events.push(event))
        .await
        .expect("generate");

    assert_eq!(
        response.responses,
        vec![GenerateResponseItem::Text {
            phase: Some(AssistantTextPhase::FinalAnswer),
            text: "bridge answer".to_string(),
        }]
    );
    assert_eq!(
        events,
        vec![GenerateStreamEvent::AssistantTextDelta {
            response_index: 0,
            delta: "bridge answer".to_string(),
        }]
    );
}

fn provider(bridge_path: std::path::PathBuf) -> FoundationLocalProvider {
    FoundationLocalProvider::new(FoundationLocalProviderConfig {
        default_profile: "default".to_string(),
        bridge_path: Some(bridge_path),
        system_errors: None,
    })
    .expect("provider")
}

fn required_response_request() -> GenerateRequest {
    GenerateRequest {
        conversation_id: None,
        model: None,
        input: GenerateInput::Text("prompt text".to_string()),
        instructions: None,
        options: GenerateOptions {
            require_noema_response: true,
            ..GenerateOptions::default()
        },
        tools: Vec::new(),
        tool_transport: ProviderToolTransport::NoemaEnvelope,
        tool_choice: Default::default(),
        parallel_tool_calls: false,
    }
}
