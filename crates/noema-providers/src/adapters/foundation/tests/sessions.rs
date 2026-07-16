use crate::{
    FoundationLocalProviderConfig, GenerateInput, GenerateMessage, GenerateMessageRole,
    GenerateOptions, GenerateRequest, ModelProvider,
};

use super::{super::FoundationLocalProvider, support::bridge_script};

#[tokio::test]
async fn generate_reuses_bridge_session_for_canonical_required_response() {
    let log = tempfile::NamedTempFile::new().expect("log");
    let log_path = log.path().to_string_lossy().to_string();
    let (_dir, bridge_path) = bridge_script(&format!(
        r#"#!/bin/sh
LOG_PATH="{}"
while IFS= read -r line; do
  case "$line" in
    *'"id":"handshake"'*) printf '%s\n' '{{"id":"handshake","payload":{{"type":"handshake_ok","protocol_version":1}}}}' ;;
    *'"id":"health"'*) printf '%s\n' '{{"id":"health","payload":{{"type":"health","available":true,"profiles":[{{"id":"default","label":"Default"}}],"unavailable_reason":null}}}}' ;;
    *'"id":"create_session"'*) printf '%s\n' "create_session" >> "$LOG_PATH"; printf '%s\n' '{{"id":"create_session","payload":{{"type":"session_created","session_id":"session-1"}}}}' ;;
    *'"id":"generate"'*) printf '%s\n' '{{"id":"generate","payload":{{"type":"generate_complete","text":"{{\"response_status\":\"final\",\"responses\":[{{\"kind\":\"text\",\"phase\":\"final_answer\",\"text\":\"bridge answer\"}}],\"tool_calls\":[]}}"}}}}' ;;
    *) printf '%s\n' '{{"id":"unknown","payload":{{"type":"error","code":"unsupported_request","message":"Unsupported request."}}}}' ;;
  esac
done
"#,
        log_path
    ));
    let provider = provider(bridge_path);

    for input in [
        GenerateInput::Text("first".to_string()),
        GenerateInput::Messages(vec![
            message(GenerateMessageRole::User, "first"),
            message(GenerateMessageRole::Assistant, "bridge answer"),
            message(GenerateMessageRole::User, "second"),
        ]),
    ] {
        provider
            .generate(GenerateRequest {
                conversation_id: Some("conversation:stable".to_string()),
                model: Some("default".to_string()),
                input,
                instructions: Some("be concise".to_string()),
                options: GenerateOptions {
                    require_noema_response: true,
                    ..GenerateOptions::default()
                },
                tools: Vec::new(),
                tool_choice: Default::default(),
                parallel_tool_calls: false,
            })
            .await
            .expect("generate");
    }

    let create_session_count = std::fs::read_to_string(log.path())
        .expect("log read")
        .lines()
        .filter(|line| *line == "create_session")
        .count();
    assert_eq!(create_session_count, 1);
}

#[tokio::test]
async fn generate_recreates_bridge_session_when_static_instructions_change() {
    let log = tempfile::NamedTempFile::new().expect("log");
    let log_path = log.path().to_string_lossy().to_string();
    let (_dir, bridge_path) = bridge_script(&format!(
        r#"#!/bin/sh
LOG_PATH="{}"
while IFS= read -r line; do
  case "$line" in
    *'"id":"handshake"'*) printf '%s\n' '{{"id":"handshake","payload":{{"type":"handshake_ok","protocol_version":1}}}}' ;;
    *'"id":"health"'*) printf '%s\n' '{{"id":"health","payload":{{"type":"health","available":true,"profiles":[{{"id":"default","label":"Default"}}],"unavailable_reason":null}}}}' ;;
    *'"id":"create_session"'*) printf '%s\n' "$line" >> "$LOG_PATH"; printf '%s\n' '{{"id":"create_session","payload":{{"type":"session_created","session_id":"session-1"}}}}' ;;
    *'"id":"close_session"'*) printf '%s\n' '{{"id":"close_session","payload":{{"type":"replay_complete"}}}}' ;;
    *'"id":"generate"'*) printf '%s\n' '{{"id":"generate","payload":{{"type":"generate_complete","text":"bridge answer"}}}}' ;;
    *) printf '%s\n' '{{"id":"unknown","payload":{{"type":"error","code":"unsupported_request","message":"Unsupported request."}}}}' ;;
  esac
done
"#,
        log_path
    ));
    let provider = provider(bridge_path);

    for instructions in ["be concise", "be expansive"] {
        provider
            .generate(GenerateRequest {
                conversation_id: Some("conversation:stable".to_string()),
                model: Some("default".to_string()),
                input: GenerateInput::Text("hello".to_string()),
                instructions: Some(instructions.to_string()),
                options: GenerateOptions::default(),
                tools: Vec::new(),
                tool_choice: Default::default(),
                parallel_tool_calls: false,
            })
            .await
            .expect("generate");
    }

    let create_session_requests = std::fs::read_to_string(log.path()).expect("log read");
    assert_eq!(create_session_requests.lines().count(), 2);
    assert!(create_session_requests.contains("be concise"));
    assert!(create_session_requests.contains("be expansive"));
}

#[tokio::test]
async fn generate_reuses_exact_history_and_resets_on_divergence() {
    let log = tempfile::NamedTempFile::new().expect("log");
    let log_path = log.path().to_string_lossy().to_string();
    let (_dir, bridge_path) = bridge_script(&format!(
        r#"#!/bin/sh
LOG_PATH="{}"
while IFS= read -r line; do
  case "$line" in
    *'"id":"handshake"'*) printf '%s\n' '{{"id":"handshake","payload":{{"type":"handshake_ok","protocol_version":1}}}}' ;;
    *'"id":"health"'*) printf '%s\n' '{{"id":"health","payload":{{"type":"health","available":true,"profiles":[{{"id":"default","label":"Default"}}],"unavailable_reason":null}}}}' ;;
    *'"id":"create_session"'*) printf '%s\n' '{{"id":"create_session","payload":{{"type":"session_created","session_id":"session-1"}}}}' ;;
    *'"id":"close_session"'*) printf '%s\n' '{{"id":"close_session","payload":{{"type":"replay_complete"}}}}' ;;
    *'"id":"replay_turns"'*) printf '%s\n' "$line" >> "$LOG_PATH"; printf '%s\n' '{{"id":"replay_turns","payload":{{"type":"replay_complete"}}}}' ;;
    *'"id":"generate"'*) printf '%s\n' '{{"id":"generate","payload":{{"type":"generate_complete","text":"bridge answer"}}}}' ;;
    *) printf '%s\n' '{{"id":"unknown","payload":{{"type":"error","code":"unsupported_request","message":"Unsupported request."}}}}' ;;
  esac
done
"#,
        log_path
    ));
    let provider = provider(bridge_path);

    provider
        .generate(session_request(vec![
            message(GenerateMessageRole::Developer, "environment@1"),
            message(GenerateMessageRole::User, "first"),
        ]))
        .await
        .expect("first generate");
    provider
        .generate(session_request(vec![
            message(GenerateMessageRole::Developer, "environment@1"),
            message(GenerateMessageRole::User, "first"),
            message(GenerateMessageRole::Assistant, "bridge answer"),
            message(GenerateMessageRole::Developer, "environment@2"),
            message(GenerateMessageRole::User, "second"),
        ]))
        .await
        .expect("second generate");
    provider
        .generate(session_request(vec![
            message(GenerateMessageRole::Developer, "environment@1"),
            message(GenerateMessageRole::User, "first"),
            message(GenerateMessageRole::Assistant, "divergent answer"),
            message(GenerateMessageRole::Developer, "environment@2"),
            message(GenerateMessageRole::User, "second"),
            message(GenerateMessageRole::Assistant, "bridge answer"),
            message(GenerateMessageRole::User, "third"),
        ]))
        .await
        .expect("divergent history should recreate the session");

    let replay_requests = std::fs::read_to_string(log.path()).expect("log read");
    let mut replay_requests = replay_requests.lines();
    let first = replay_requests.next().expect("initial context replay");
    let second = replay_requests.next().expect("context update replay");
    let third = replay_requests
        .next()
        .expect("divergent full-history replay");
    assert!(replay_requests.next().is_none());
    assert!(first.contains("environment@1"));
    assert!(second.contains("environment@2"));
    assert!(!second.contains("environment@1"));
    assert!(second.contains("application_context"));
    assert!(third.contains("environment@1"));
    assert!(third.contains("divergent answer"));
}

fn provider(bridge_path: std::path::PathBuf) -> FoundationLocalProvider {
    FoundationLocalProvider::new(FoundationLocalProviderConfig {
        default_profile: "default".to_string(),
        bridge_path: Some(bridge_path),
        system_errors: None,
    })
    .expect("provider")
}

fn session_request(messages: Vec<GenerateMessage>) -> GenerateRequest {
    GenerateRequest {
        conversation_id: Some("conversation:stable".to_string()),
        model: Some("default".to_string()),
        input: GenerateInput::Messages(messages),
        instructions: Some("stable kernel".to_string()),
        options: GenerateOptions::default(),
        tools: Vec::new(),
        tool_choice: Default::default(),
        parallel_tool_calls: false,
    }
}

fn message(role: GenerateMessageRole, content: &str) -> GenerateMessage {
    GenerateMessage {
        role,
        content: content.to_string(),
    }
}
