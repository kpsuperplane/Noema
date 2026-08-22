use crate::{
    FoundationLocalProviderConfig, GenerateResponseItem, GenerateStreamEvent, ModelProvider,
};

use super::{
    super::FoundationLocalProvider,
    support::{bridge_script, healthy_bridge_script},
};

#[tokio::test]
async fn generate_returns_plain_bridge_text() {
    let (_dir, bridge_path) = bridge_script(&healthy_bridge_script(
        r#"
    *'"id":"create_session"'*) printf '%s\n' '{"id":"create_session","payload":{"type":"session_created","session_id":"session-1"}}' ;;
    *'"id":"generate"'*) printf '%s\n' '{"id":"generate","payload":{"type":"generate_complete","text":"bridge answer"}}' ;;
"#,
    ));
    let provider = provider(bridge_path);

    let response = provider
        .generate(crate::GenerateRequest::text("prompt text"))
        .await
        .expect("generate");

    assert_eq!(
        response.responses,
        vec![GenerateResponseItem::Text {
            id: None,
            phase: None,
            text: "bridge answer".to_string(),
            citations: Vec::new(),
        }]
    );
    assert_eq!(response.assistant_text(), "bridge answer");
}

#[tokio::test]
async fn generate_streaming_forwards_plain_assistant_text_deltas() {
    let (_dir, bridge_path) = bridge_script(&healthy_bridge_script(
        r#"
    *'"id":"create_session"'*) printf '%s\n' '{"id":"create_session","payload":{"type":"session_created","session_id":"session-1"}}' ;;
    *'"id":"generate"'*) printf '%s\n' '{"id":"generate","payload":{"type":"assistant_text_delta","delta":"bridge "}}'; printf '%s\n' '{"id":"generate","payload":{"type":"generate_complete","text":"bridge answer"}}' ;;
"#,
    ));
    let provider = provider(bridge_path);
    let mut events = Vec::new();

    let response = provider
        .generate_streaming(crate::GenerateRequest::text("prompt text"), &mut |event| {
            events.push(event)
        })
        .await
        .expect("generate");

    assert_eq!(response.assistant_text(), "bridge answer");
    assert_eq!(
        events,
        vec![GenerateStreamEvent::AssistantTextDelta {
            response_index: 0,
            delta: "bridge ".to_string(),
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
