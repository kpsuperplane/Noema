//! Provider-account adapter and replay tests retained with GraphQL product support.

use super::*;
use noema_conversations::{ConversationItemKind, ConversationItemRecord, ConversationItemStatus};
use std::sync::{Arc, Mutex};

use noema_providers::{
    CreateSecretProviderAccountRequest, ProviderAccountCatalogEntry, ProviderAccountOperationError,
    ProviderAccountOperationFuture, ProviderAccountOperations, ProviderAccountOperationsHandle,
    ProviderAccountRecord, ProviderAuthAttemptStatus, ProviderAuthAttemptView,
    SaveProviderAccountSecretRequest, StartProviderAuthRequest,
};
use noema_runtime::TurnTranscriptItem;
use serde_json::json;

#[tokio::test]
async fn start_provider_auth_attempt_uses_account_operations_handle() {
    let operations = Arc::new(RecordingProviderAccountOperations::default());
    let handle: ProviderAccountOperationsHandle = operations.clone();
    let state = GraphqlState::for_tests_with_provider_account_operations(handle);

    let attempt = onboarding::start_provider_auth_attempt(
        &state,
        onboarding::GraphqlStartProviderAuthAttemptInput {
            provider_kind: "codex".to_string(),
            provider_account_id: "provider_account:codex:default".to_string(),
            method: onboarding::GraphqlProviderAuthMethod::OauthDeviceCode,
        },
    )
    .await
    .expect("start provider auth");

    assert_eq!(
        attempt.status,
        onboarding::GraphqlProviderAuthAttemptStatus::WaitingForUser
    );
    assert_eq!(
        operations
            .start_requests
            .lock()
            .expect("start requests")
            .as_slice(),
        [StartProviderAuthRequest {
            provider_kind: "codex".to_string(),
            provider_account_id: "provider_account:codex:default".to_string(),
            method: noema_providers::ProviderAuthMethod::OauthDeviceCode,
        }]
    );
}

#[tokio::test]
async fn provider_auth_attempt_uses_account_operations_without_store() {
    let operations = Arc::new(RecordingProviderAccountOperations::default());
    let handle: ProviderAccountOperationsHandle = operations.clone();
    let state = GraphqlState::for_tests_with_provider_account_operations(handle);

    let attempt =
        onboarding::provider_auth_attempt(&state, "provider_auth_attempt_test".to_string())
            .await
            .expect("provider auth attempt")
            .expect("attempt");
    assert_eq!(
        attempt.status,
        onboarding::GraphqlProviderAuthAttemptStatus::WaitingForUser
    );
    assert_eq!(
        operations
            .attempt_requests
            .lock()
            .expect("attempt requests")
            .as_slice(),
        ["provider_auth_attempt_test"]
    );
}

#[tokio::test]
async fn provider_auth_operation_errors_use_safe_messages() {
    let operations = Arc::new(RecordingProviderAccountOperations {
        start_error: Some(ProviderAccountOperationError::ProviderUnavailable),
        ..RecordingProviderAccountOperations::default()
    });
    let state = GraphqlState::for_tests_with_provider_account_operations(operations);

    let error = onboarding::start_provider_auth_attempt(
        &state,
        onboarding::GraphqlStartProviderAuthAttemptInput {
            provider_kind: "codex".to_string(),
            provider_account_id: "provider_account:codex:default".to_string(),
            method: onboarding::GraphqlProviderAuthMethod::OauthDeviceCode,
        },
    )
    .await
    .expect_err("provider auth should fail");

    assert_eq!(error.message, "provider unavailable");
}

fn test_provider_account() -> noema_providers::ProviderAccountRecord {
    let status = noema_providers::ProviderAccountStatus::Unknown;
    noema_providers::ProviderAccountRecord {
        provider_account_id: "provider_account:codex:default".to_string(),
        provider_kind: "codex".to_string(),
        account_key: "default".to_string(),
        display_name: "Codex".to_string(),
        auth_method: noema_providers::ProviderAuthMethod::OauthDeviceCode,
        is_active: true,
        is_default: true,
        status,
        last_checked_at: None,
        last_authenticated_at: None,
        last_error_code: None,
        last_error_message: None,
        metadata: json!({}),
        capabilities: noema_providers::capabilities_for_provider_account(
            "codex", "default", status,
        ),
    }
}

fn test_provider_auth_attempt() -> ProviderAuthAttemptView {
    ProviderAuthAttemptView {
        attempt_id: "provider_auth_attempt_test".to_string(),
        provider_kind: "codex".to_string(),
        provider_account_id: "provider_account:codex:default".to_string(),
        method: noema_providers::ProviderAuthMethod::OauthDeviceCode,
        status: ProviderAuthAttemptStatus::WaitingForUser,
        verification_url: Some("https://example.com/device".to_string()),
        user_code: Some("ABCD-EFGH".to_string()),
        instructions: Some("Complete provider authentication.".to_string()),
        error_code: None,
        error_message: None,
    }
}

#[derive(Default)]
struct RecordingProviderAccountOperations {
    start_requests: Mutex<Vec<StartProviderAuthRequest>>,
    attempt_requests: Mutex<Vec<String>>,
    start_error: Option<ProviderAccountOperationError>,
}

impl ProviderAccountOperations for RecordingProviderAccountOperations {
    fn account_catalog(&self) -> Vec<ProviderAccountCatalogEntry> {
        Vec::new()
    }

    fn active_accounts(&self) -> ProviderAccountOperationFuture<'_, Vec<ProviderAccountRecord>> {
        Box::pin(async { Ok(vec![test_provider_account()]) })
    }

    fn create_secret_account(
        &self,
        _request: CreateSecretProviderAccountRequest,
    ) -> ProviderAccountOperationFuture<'_, ProviderAccountRecord> {
        Box::pin(async { Err(ProviderAccountOperationError::UnsupportedProvider) })
    }

    fn save_secret(
        &self,
        _request: SaveProviderAccountSecretRequest,
    ) -> ProviderAccountOperationFuture<'_, ProviderAccountRecord> {
        Box::pin(async { Err(ProviderAccountOperationError::UnsupportedProvider) })
    }

    fn clear_secret<'a>(
        &'a self,
        _provider_account_id: &'a str,
    ) -> ProviderAccountOperationFuture<'a, ProviderAccountRecord> {
        Box::pin(async { Err(ProviderAccountOperationError::UnsupportedProvider) })
    }

    fn delete_account<'a>(
        &'a self,
        _provider_account_id: &'a str,
    ) -> ProviderAccountOperationFuture<'a, bool> {
        Box::pin(async { Err(ProviderAccountOperationError::UnsupportedProvider) })
    }

    fn start_auth(
        &self,
        request: StartProviderAuthRequest,
    ) -> ProviderAccountOperationFuture<'_, ProviderAuthAttemptView> {
        self.start_requests
            .lock()
            .expect("start requests")
            .push(request);
        let result = self
            .start_error
            .clone()
            .map_or_else(|| Ok(test_provider_auth_attempt()), Err);
        Box::pin(async move { result })
    }

    fn auth_attempt<'a>(
        &'a self,
        attempt_id: &'a str,
    ) -> ProviderAccountOperationFuture<'a, Option<ProviderAuthAttemptView>> {
        self.attempt_requests
            .lock()
            .expect("attempt requests")
            .push(attempt_id.to_string());
        Box::pin(async { Ok(Some(test_provider_auth_attempt())) })
    }

    fn cancel_auth_attempt<'a>(
        &'a self,
        _attempt_id: &'a str,
    ) -> ProviderAccountOperationFuture<'a, Option<ProviderAuthAttemptView>> {
        Box::pin(async { Ok(None) })
    }

    fn record_auth_failure<'a>(
        &'a self,
        _provider_account_id: &'a str,
        _expected_credential_revision: u64,
    ) -> ProviderAccountOperationFuture<'a, ProviderAccountRecord> {
        Box::pin(async { Ok(test_provider_account()) })
    }

    fn reconcile_account<'a>(
        &'a self,
        _provider_account_id: &'a str,
    ) -> ProviderAccountOperationFuture<'a, ProviderAccountRecord> {
        Box::pin(async { Ok(test_provider_account()) })
    }

    fn refresh_model_catalog<'a>(
        &'a self,
        _provider_account_id: &'a str,
    ) -> ProviderAccountOperationFuture<'a, ProviderAccountRecord> {
        Box::pin(async { Ok(test_provider_account()) })
    }
}

#[test]
fn chat_onboarding_gate_requires_authenticated_provider_account() {
    assert!(!noema_host::onboarding_status_from_options(None, false).is_user_onboarded);

    let mut account = test_provider_account();
    account.status = noema_providers::ProviderAccountStatus::Unknown;
    assert!(!noema_host::onboarding_status_from_options(Some(account), false).is_user_onboarded);

    let mut account = test_provider_account();
    account.status = noema_providers::ProviderAccountStatus::Unauthenticated;
    assert!(!noema_host::onboarding_status_from_options(Some(account), false).is_user_onboarded);

    let mut account = test_provider_account();
    account.status = noema_providers::ProviderAccountStatus::Authenticated;
    assert!(noema_host::onboarding_status_from_options(Some(account), false).is_user_onboarded);
    assert!(noema_host::onboarding_status_from_options(None, true).is_user_onboarded);
}

#[test]
fn conversation_replay_item_converts_assistant_text_record() {
    let item = web_conversation_item_from_record(ConversationItemRecord {
        item_id: "item_1".to_string(),
        conversation_id: "conversation_1".to_string(),
        turn_id: Some("turn_1".to_string()),
        sequence_index: 1,
        cursor: "conversation_item:1".to_string(),
        kind: ConversationItemKind::AssistantText,
        status: ConversationItemStatus::Completed,
        content_text: Some("hello from replay".to_string()),
        payload_json: json!({}),
        metadata: json!({
            "provider_usage": {
                "provider": "codex",
                "model": "gpt-test",
                "phase": "initial",
                "response_index": 0,
                "input_tokens": 100,
                "cached_input_tokens": 50,
                "cache_hit_ratio": 0.5,
                "output_tokens": 10,
                "total_tokens": 110
            }
        }),
    })
    .expect("convert record")
    .expect("visible item");

    assert_eq!(item.item_id, "item_1");
    assert_eq!(item.turn_id.as_deref(), Some("turn_1"));
    assert_eq!(item.metadata["provider_usage"]["cache_hit_ratio"], 0.5);
    assert_eq!(
        item.item,
        TurnTranscriptItem::AssistantText {
            text: "hello from replay".to_string()
        }
    );
}

#[test]
fn conversation_replay_maps_multiple_choice_prompt() {
    let item = web_conversation_item_from_record(ConversationItemRecord {
        item_id: "item_choice_1".to_string(),
        conversation_id: "conversation_1".to_string(),
        turn_id: Some("turn_1".to_string()),
        sequence_index: 1,
        cursor: "conversation_item:1".to_string(),
        kind: ConversationItemKind::MultipleChoicePrompt,
        status: ConversationItemStatus::Completed,
        content_text: Some("Pick a direction".to_string()),
        payload_json: json!({
            "prompt": "Pick a direction",
            "selection_mode": "pick_one",
            "options": [
                {"id": "ship", "label": "Ship it"},
                {"id": "polish", "label": "Polish first"}
            ]
        }),
        metadata: json!({}),
    })
    .expect("convert record")
    .expect("visible item");

    assert_eq!(
        item.item,
        TurnTranscriptItem::MultipleChoicePrompt {
            prompt: "Pick a direction".to_string(),
            selection_mode: noema_providers::MultipleChoiceSelectionMode::PickOne,
            options: vec![
                noema_providers::MultipleChoiceOption {
                    id: "ship".to_string(),
                    label: "Ship it".to_string(),
                },
                noema_providers::MultipleChoiceOption {
                    id: "polish".to_string(),
                    label: "Polish first".to_string(),
                },
            ],
        }
    );
}

#[test]
fn conversation_replay_item_converts_persisted_action_record() {
    let item = web_conversation_item_from_record(ConversationItemRecord {
        item_id: "item_tool_1".to_string(),
        conversation_id: "conversation_1".to_string(),
        turn_id: Some("turn_1".to_string()),
        sequence_index: 1,
        cursor: "conversation_item:1".to_string(),
        kind: ConversationItemKind::ToolCall,
        status: ConversationItemStatus::Completed,
        content_text: Some("Tool call: search_memory".to_string()),
        payload_json: json!({
            "id": "tool_call:conversation_1:0:1",
            "activity_kind": "tool_call",
            "status": "completed",
            "title": "Tool call: search_memory",
            "summary": "provider id call_1",
            "metadata": {"action": {"name": "search_memory"}},
        }),
        metadata: json!({}),
    })
    .expect("convert record")
    .expect("visible item");

    assert_eq!(item.item_id, "item_tool_1");
    assert!(matches!(
        item.item,
        TurnTranscriptItem::Activity {
            ref activity_kind,
            ref title,
            ..
        } if activity_kind == "tool_call" && title == "Tool call: search_memory"
    ));
}

#[test]
fn conversation_replay_item_rejects_malformed_activity_record() {
    let error = web_conversation_item_from_record(ConversationItemRecord {
        item_id: "item_bad".to_string(),
        conversation_id: "conversation_1".to_string(),
        turn_id: Some("turn_1".to_string()),
        sequence_index: 1,
        cursor: "conversation_item:1".to_string(),
        kind: ConversationItemKind::Activity,
        status: ConversationItemStatus::Completed,
        content_text: None,
        payload_json: json!({ "not": "an activity payload" }),
        metadata: json!({}),
    })
    .expect_err("malformed record should fail");

    assert!(error.to_string().contains("invalid replay payload"));
}
