//! Local web UI server for the Noema daemon.

mod assets;
pub(super) mod authority;
#[cfg(test)]
#[allow(dead_code)]
mod http;
mod provider_auth;
mod replay;
mod router;
pub(super) mod session;

use tokio::net::TcpListener;

use crate::WebConfig;

pub(crate) use self::{
    provider_auth::{
        ProviderAuthStartRequest, is_user_onboarded_for_chat,
        persist_provider_account_status_from_attempt, reconcile_onboarding_provider_account,
        start_provider_auth_attempt_view_from_parts,
    },
    replay::{ConversationReplayItem, web_conversation_item_from_record},
};

use super::protocol::DaemonError;

/// State shared by local web UI connections.
#[derive(Clone)]
pub(crate) struct WebState {
    graphql_state: crate::graphql::GraphqlState,
    graphql_schema: crate::graphql::GraphqlSchema,
    system_errors: Option<crate::SystemErrorLogger>,
    authority: authority::CanonicalAuthority,
    sessions: session::SessionSecurity,
}

impl WebState {
    /// Build shared web UI state.
    #[must_use]
    pub(super) fn new(
        graphql_state: crate::graphql::GraphqlState,
        authority: authority::CanonicalAuthority,
        sessions: session::SessionSecurity,
    ) -> Self {
        let graphql_schema = crate::graphql::build_schema(graphql_state.clone());
        let system_errors = graphql_state
            .paths()
            .ok()
            .map(crate::SystemErrorLogger::from_paths);
        Self {
            graphql_state,
            graphql_schema,
            system_errors,
            authority,
            sessions,
        }
    }

    pub(crate) fn graphql_state(&self) -> &crate::graphql::GraphqlState {
        &self.graphql_state
    }

    pub(crate) fn graphql_schema(&self) -> &crate::graphql::GraphqlSchema {
        &self.graphql_schema
    }

    fn authority(&self) -> &authority::CanonicalAuthority {
        &self.authority
    }

    fn sessions(&self) -> &session::SessionSecurity {
        &self.sessions
    }

    fn record_artifact_failure(&self, operation: &'static str) {
        let event = crate::SystemErrorEvent::new(
            "artifact_download_failure",
            "artifact download operation failed",
        )
        .with_context(serde_json::json!({ "operation": operation }));
        if let Some(system_errors) = &self.system_errors {
            if system_errors.append(event).is_err() {
                eprintln!("Noema artifact download failure: diagnostic_write");
            }
        } else {
            eprintln!("Noema artifact download failure: {operation}");
        }
    }
}

pub(super) use router::build_router;

/// Bind the local web UI listener.
pub(super) async fn bind_listener(config: &WebConfig) -> Result<TcpListener, DaemonError> {
    let host = authority::parse_loopback_ip(&config.host).map_err(DaemonError::Protocol)?;
    TcpListener::bind((host, config.port))
        .await
        .map_err(|source| {
            DaemonError::Protocol(format!(
                "failed to bind web UI at {}:{}: {source}",
                config.host, config.port
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{future::Future, pin::Pin, time::Duration};

    use crate::provider::adapters::codex_oauth::{CodexOAuthTokens, CodexTokenStore};
    use crate::{
        TurnTranscriptItem,
        provider::auth::{CodexDeviceAuthRequest, ProviderAuthAttemptView},
        {ConversationItemKind, ConversationItemRecord, ConversationItemStatus},
    };
    use serde_json::json;

    use super::provider_auth::{
        CodexDeviceAuthStarter, ProviderAccountStatusStore, ProviderAccountStatusUpdate,
        ProviderAuthAttemptPoller, StartProviderAuthAttemptError,
        persist_provider_auth_attempt_terminal_status, provider_account_status_update_from_attempt,
        start_codex_provider_auth_attempt, validate_provider_auth_account,
    };

    #[test]
    fn provider_auth_account_validation_checks_active_kind_and_method() {
        let mut account = test_provider_account();
        assert!(
            validate_provider_auth_account(
                &account,
                "codex",
                crate::ProviderAuthMethod::OauthDeviceCode
            )
            .is_ok()
        );

        account.provider_kind = "other".to_string();
        assert_eq!(
            validate_provider_auth_account(
                &account,
                "codex",
                crate::ProviderAuthMethod::OauthDeviceCode
            )
            .unwrap_err()
            .message(),
            "provider account mismatch"
        );

        account = test_provider_account();
        account.is_active = false;
        assert_eq!(
            validate_provider_auth_account(
                &account,
                "codex",
                crate::ProviderAuthMethod::OauthDeviceCode
            )
            .unwrap_err()
            .message(),
            "provider account not found"
        );

        account = test_provider_account();
        account.auth_method = crate::ProviderAuthMethod::ExternalManual;
        assert_eq!(
            validate_provider_auth_account(
                &account,
                "codex",
                crate::ProviderAuthMethod::OauthDeviceCode
            )
            .unwrap_err()
            .message(),
            "provider account auth method mismatch"
        );
    }

    #[test]
    fn provider_auth_attempt_status_maps_to_safe_account_status() {
        let mut attempt = test_provider_auth_attempt();
        attempt.status = crate::provider::auth::ProviderAuthAttemptStatus::Completed;
        assert_eq!(
            provider_account_status_update_from_attempt(&attempt),
            Some(ProviderAccountStatusUpdate {
                status: crate::ProviderAccountStatus::Authenticated,
                error_code: None,
                error_message: None,
            })
        );

        attempt.status = crate::provider::auth::ProviderAuthAttemptStatus::Failed;
        attempt.error_code = Some("codex_login_failed".to_string());
        attempt.error_message = Some("codex auth failed".to_string());
        assert_eq!(
            provider_account_status_update_from_attempt(&attempt),
            Some(ProviderAccountStatusUpdate {
                status: crate::ProviderAccountStatus::Unauthenticated,
                error_code: Some("codex_login_failed".to_string()),
                error_message: Some("codex auth failed".to_string()),
            })
        );

        attempt.status = crate::provider::auth::ProviderAuthAttemptStatus::WaitingForUser;
        assert_eq!(provider_account_status_update_from_attempt(&attempt), None);
    }

    #[tokio::test]
    async fn start_auth_returned_completed_attempt_persists_authenticated_status() {
        let store = RecordingProviderAccountStatusStore::default();
        let mut attempt = test_provider_auth_attempt();
        attempt.status = crate::provider::auth::ProviderAuthAttemptStatus::Completed;
        let starter = RecordingCodexDeviceAuthStarter { attempt };
        let paths =
            crate::NoemaPaths::from_noema_home(tempfile::tempdir().expect("temp dir").path())
                .expect("paths");

        start_codex_provider_auth_attempt(&starter, &store, &paths, &test_provider_account())
            .await
            .expect("start provider auth");

        let updates = store.updates.lock().expect("updates lock");
        assert_eq!(
            updates.as_slice(),
            [RecordedProviderAccountStatusUpdate {
                provider_account_id: "provider_account:codex:default".to_string(),
                status: crate::ProviderAccountStatus::Authenticated,
                error_code: None,
                error_message: None,
            }]
        );
    }

    #[tokio::test]
    async fn start_auth_preserves_provider_start_error_message() {
        let store = RecordingProviderAccountStatusStore::default();
        let starter = FailingCodexDeviceAuthStarter {
            message: "device code request returned status 403",
        };
        let paths =
            crate::NoemaPaths::from_noema_home(tempfile::tempdir().expect("temp dir").path())
                .expect("paths");

        let error =
            start_codex_provider_auth_attempt(&starter, &store, &paths, &test_provider_account())
                .await
                .expect_err("auth start should fail");

        assert_eq!(
            error,
            StartProviderAuthAttemptError::ProviderUnavailable(
                "codex provider is unavailable: device code request returned status 403"
                    .to_string()
            )
        );
        assert!(store.updates.lock().expect("updates lock").is_empty());
    }

    #[tokio::test]
    async fn auth_terminal_watcher_persists_completed_attempt_without_http_poll() {
        let store = RecordingProviderAccountStatusStore::default();
        let mut waiting = test_provider_auth_attempt();
        waiting.status = crate::provider::auth::ProviderAuthAttemptStatus::WaitingForUser;
        let mut completed = waiting.clone();
        completed.status = crate::provider::auth::ProviderAuthAttemptStatus::Completed;
        let poller = RecordingProviderAuthAttemptPoller::new(vec![waiting, completed]);

        persist_provider_auth_attempt_terminal_status(
            &poller,
            &store,
            "provider_auth_attempt_test",
            Duration::from_millis(1),
        )
        .await
        .expect("persist terminal status");

        let updates = store.updates.lock().expect("updates lock");
        assert_eq!(
            updates.as_slice(),
            [RecordedProviderAccountStatusUpdate {
                provider_account_id: "provider_account:codex:default".to_string(),
                status: crate::ProviderAccountStatus::Authenticated,
                error_code: None,
                error_message: None,
            }]
        );
    }

    #[tokio::test]
    async fn onboarding_reconciles_existing_noema_codex_tokens() {
        let store = RecordingProviderAccountStatusStore::default();
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let paths = crate::NoemaPaths::from_noema_home(temp_dir.path()).expect("paths");
        let mut account = test_provider_account();
        account.status = crate::ProviderAccountStatus::Unauthenticated;
        let account_home =
            paths.provider_account_home(&account.provider_kind, &account.account_key);
        CodexTokenStore::new(account_home)
            .write(&CodexOAuthTokens {
                access_token: "access".to_string(),
                refresh_token: "refresh".to_string(),
                last_refresh: 123,
            })
            .expect("credential marker");

        let reconciled = reconcile_onboarding_provider_account(&store, &paths, Some(account))
            .await
            .expect("reconcile account")
            .expect("account");

        assert_eq!(
            reconciled.status,
            crate::ProviderAccountStatus::Authenticated
        );
        let updates = store.updates.lock().expect("updates lock");
        assert_eq!(
            updates.as_slice(),
            [RecordedProviderAccountStatusUpdate {
                provider_account_id: "provider_account:codex:default".to_string(),
                status: crate::ProviderAccountStatus::Authenticated,
                error_code: None,
                error_message: None,
            }]
        );
    }

    fn test_provider_account() -> crate::ProviderAccountRecord {
        let status = crate::ProviderAccountStatus::Unknown;
        crate::ProviderAccountRecord {
            provider_account_id: "provider_account:codex:default".to_string(),
            provider_kind: "codex".to_string(),
            account_key: "default".to_string(),
            display_name: "Codex".to_string(),
            auth_method: crate::ProviderAuthMethod::OauthDeviceCode,
            is_active: true,
            is_default: true,
            status,
            last_checked_at: None,
            last_authenticated_at: None,
            last_error_code: None,
            last_error_message: None,
            metadata: json!({}),
            capabilities: crate::provider::capabilities_for_provider_account(
                "codex", "default", status,
            ),
        }
    }

    fn test_provider_auth_attempt() -> ProviderAuthAttemptView {
        ProviderAuthAttemptView {
            attempt_id: "provider_auth_attempt_test".to_string(),
            provider_kind: "codex".to_string(),
            provider_account_id: "provider_account:codex:default".to_string(),
            method: crate::ProviderAuthMethod::OauthDeviceCode,
            status: crate::provider::auth::ProviderAuthAttemptStatus::Starting,
            verification_url: None,
            user_code: None,
            instructions: None,
            error_code: None,
            error_message: None,
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct RecordedProviderAccountStatusUpdate {
        provider_account_id: String,
        status: crate::ProviderAccountStatus,
        error_code: Option<String>,
        error_message: Option<String>,
    }

    #[derive(Default)]
    struct RecordingProviderAccountStatusStore {
        updates: std::sync::Mutex<Vec<RecordedProviderAccountStatusUpdate>>,
    }

    struct RecordingCodexDeviceAuthStarter {
        attempt: ProviderAuthAttemptView,
    }

    struct FailingCodexDeviceAuthStarter {
        message: &'static str,
    }

    struct RecordingProviderAuthAttemptPoller {
        attempts: std::sync::Mutex<Vec<ProviderAuthAttemptView>>,
    }

    impl RecordingProviderAuthAttemptPoller {
        fn new(attempts: Vec<ProviderAuthAttemptView>) -> Self {
            Self {
                attempts: std::sync::Mutex::new(attempts),
            }
        }
    }

    impl CodexDeviceAuthStarter for RecordingCodexDeviceAuthStarter {
        fn start_codex_device_code<'a>(
            &'a self,
            _request: CodexDeviceAuthRequest,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<ProviderAuthAttemptView, crate::ProviderError>>
                    + Send
                    + 'a,
            >,
        > {
            let attempt = self.attempt.clone();
            Box::pin(async move { Ok(attempt) })
        }
    }

    impl CodexDeviceAuthStarter for FailingCodexDeviceAuthStarter {
        fn start_codex_device_code<'a>(
            &'a self,
            _request: CodexDeviceAuthRequest,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<ProviderAuthAttemptView, crate::ProviderError>>
                    + Send
                    + 'a,
            >,
        > {
            let message = self.message.to_string();
            Box::pin(async move {
                Err(crate::ProviderError::ProviderUnavailable {
                    provider: "codex".to_string(),
                    message,
                })
            })
        }
    }

    impl ProviderAuthAttemptPoller for RecordingProviderAuthAttemptPoller {
        fn poll_provider_auth_attempt<'a>(
            &'a self,
            _attempt_id: &'a str,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<Option<ProviderAuthAttemptView>, DaemonError>>
                    + Send
                    + 'a,
            >,
        > {
            let attempt = {
                let mut attempts = self.attempts.lock().expect("attempts lock");
                if attempts.len() > 1 {
                    Some(attempts.remove(0))
                } else {
                    attempts.first().cloned()
                }
            };
            Box::pin(async move { Ok(attempt) })
        }
    }

    impl ProviderAccountStatusStore for RecordingProviderAccountStatusStore {
        fn update_provider_account_status<'a>(
            &'a self,
            provider_account_id: &'a str,
            status: crate::ProviderAccountStatus,
            error_code: Option<&'a str>,
            error_message: Option<&'a str>,
        ) -> Pin<Box<dyn Future<Output = Result<(), DaemonError>> + Send + 'a>> {
            Box::pin(async move {
                self.updates.lock().expect("updates lock").push(
                    RecordedProviderAccountStatusUpdate {
                        provider_account_id: provider_account_id.to_string(),
                        status,
                        error_code: error_code.map(str::to_string),
                        error_message: error_message.map(str::to_string),
                    },
                );
                Ok(())
            })
        }
    }

    #[test]
    fn chat_onboarding_gate_requires_authenticated_provider_account() {
        assert!(!is_user_onboarded_for_chat(None));

        let mut account = test_provider_account();
        account.status = crate::ProviderAccountStatus::Unknown;
        assert!(!is_user_onboarded_for_chat(Some(account)));

        let mut account = test_provider_account();
        account.status = crate::ProviderAccountStatus::Unauthenticated;
        assert!(!is_user_onboarded_for_chat(Some(account)));

        let mut account = test_provider_account();
        account.status = crate::ProviderAccountStatus::Authenticated;
        assert!(is_user_onboarded_for_chat(Some(account)));
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
                selection_mode: crate::provider::MultipleChoiceSelectionMode::PickOne,
                options: vec![
                    crate::provider::MultipleChoiceOption {
                        id: "ship".to_string(),
                        label: "Ship it".to_string(),
                    },
                    crate::provider::MultipleChoiceOption {
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
}
