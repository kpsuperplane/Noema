#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::StreamExt;
    use noema_capabilities_mcp::McpRepository;
    use noema_runtime::{ConversationRuntimeEvent, TurnStreamEvent};
    use serde_json::json;
    use tempfile::TempDir;

    use crate::test_support::TestMcpSetupOutcome;

    #[test]
    fn schema_sdl_exposes_initial_noema_fields() {
        let schema = build_schema(GraphqlState::for_tests());
        let sdl = schema.sdl();

        assert!(sdl.contains("type Query"));
        assert!(sdl.contains("localStatus"));
        assert!(sdl.contains("localModelSetup"));
        assert!(sdl.contains("localModelCatalog"));
        assert!(sdl.contains("localModelInstallations"));
        assert!(sdl.contains("defaultModelPreference"));
        assert!(sdl.contains("onboardingStatus"));
        assert!(sdl.contains("type Mutation"));
        assert!(sdl.contains("startProviderAuthAttempt"));
        assert!(sdl.contains("installLocalModel"));
        assert!(sdl.contains("importLocalModel"));
        assert!(sdl.contains("cancelLocalModelInstall"));
        assert!(sdl.contains("removeLocalModel"));
        assert!(sdl.contains("activateLocalModel"));
        assert!(sdl.contains("saveDefaultModelPreference"));
        assert!(sdl.contains("retryLocalModelRuntime"));
        assert!(sdl.contains("primaryConversation"));
        assert!(sdl.contains("conversationTranscriptPage"));
        assert!(sdl.contains("type TaskReference"));
        assert!(sdl.contains("TaskReference"));
        assert!(sdl.contains("ensurePrimaryConversation"));
        assert!(!sdl.contains("startPrimaryConversation"));
        assert!(sdl.contains("sendConversationTurn"));
        assert!(sdl.contains("saveToolCalibration"));
        assert!(sdl.contains("autofillToolCalibrations"));
        assert!(!sdl.contains("Graphql"));
        assert!(sdl.contains("type AutofillToolCalibrationsResult"));
        assert!(sdl.contains("type ToolCalibrationSuggestion"));
        assert!(sdl.contains("createMcpServer"));
        assert!(sdl.contains("continueMcpServerSetup"));
        assert!(sdl.contains("startMcpServerReauthenticationOauthSetup"));
        assert!(sdl.contains("SaveToolCalibrationInput"));
        assert!(sdl.contains("type ToolCalibration"));
        assert!(sdl.contains("type Subscription"));
        assert!(sdl.contains("conversationEvents"));
        assert!(sdl.contains("taskEvents"));
        assert!(sdl.contains("localModelEvents"));
        assert!(sdl.contains("type TaskRunItem"));
        assert!(sdl.contains("taskRunItems"));
        assert!(sdl.contains("taskExecutionPolicy"));
        assert!(sdl.contains("updateTaskExecutionPolicy"));
        assert!(sdl.contains("resumeTask"));
        assert!(sdl.contains("cancelTask"));
        assert!(!sdl.contains("retryTask"));
        assert!(sdl.contains("AssistantTextDeltaEvent"));
        assert!(sdl.contains("memorySettings"));
        assert!(sdl.contains("memoryGraph"));
        assert!(sdl.contains("saveMemoryServiceSettings"));
        assert!(sdl.contains("checkMemoryService"));
        assert!(sdl.contains("type MemorySettings"));
        assert!(sdl.contains("type MemoryGraph"));
        assert!(sdl.contains("MemoryGraphInput"));
        assert!(sdl.contains("type MemoryServiceStatus"));
        assert!(sdl.contains("SaveMemoryServiceSettingsInput"));
        assert!(!sdl.contains("memoryClaims"));
        assert!(!sdl.contains("memoryClaim("));
        assert!(!sdl.contains("memoryPredicateProposals"));
        assert!(!sdl.contains("memoryPredicateProposal"));
        assert!(sdl.contains("providerAccounts"));
        assert!(sdl.contains("type ProviderAccount"));
        assert!(sdl.contains("agents"));
        assert!(sdl.contains("type Agent"));
        assert!(sdl.contains("saveAgentModelPreference"));
        assert!(sdl.contains("type AgentModelPreference"));
        assert!(sdl.contains("type AgentModelProviderOption"));
        assert!(sdl.contains("type AgentModelProfileOption"));
        assert!(sdl.contains("webFetchSettings"));
        assert!(sdl.contains("type WebFetchSettings"));
        assert!(sdl.contains("type WebFetchSummarizerSettings"));
        assert!(sdl.contains("webToolSettings"));
        assert!(sdl.contains("type WebToolSettings"));
        assert!(sdl.contains("type WebToolBindingSettings"));
        assert!(sdl.contains("type WebToolProviderOption"));
        assert!(sdl.contains("saveWebFetchSummarizerPreference"));
        assert!(sdl.contains("SaveWebFetchSummarizerPreferenceInput"));
        assert!(sdl.contains("saveWebToolProviderBinding"));
        assert!(sdl.contains("SaveWebToolProviderBindingInput"));
        assert!(sdl.contains("agentId"));
        assert!(sdl.contains("displayName"));
        assert!(sdl.contains("isPrimary"));
        assert!(sdl.contains("mcpServers"));
        assert!(sdl.contains("type McpServer"));
        assert!(sdl.contains("mcpTools"));
        assert!(sdl.contains("type McpTool"));
        assert!(!sdl.contains("type MemoryClaim"));
        assert!(!sdl.contains("type MemoryClaimEvidence"));
        assert!(!sdl.contains("type PredicateProposal"));
        assert!(sdl.contains("MemoryGraph"));
        assert!(sdl.contains("MemoryGraphInput"));
    }

    #[tokio::test]
    async fn owner_sensitive_operations_require_a_request_principal() {
        let schema = build_schema_without_request_principal(GraphqlState::for_tests());
        let query_response = schema
            .execute(
                r#"
                query {
                  task(taskId: "task:foreign") { taskId }
                  artifact(artifactId: "artifact:foreign") { artifactId }
                  conversationTranscriptPage(input: {
                    conversationId: "conversation:foreign"
                  }) {
                    pageInfo { limit }
                  }
                }
                "#,
            )
            .await;
        assert_eq!(query_response.errors.len(), 3);
        assert!(query_response.errors.iter().all(|error| {
            error
                .message
                .contains("request is unauthenticated")
        }));

        let mutation_response = schema
            .execute(
                r#"
                mutation {
                  createConversationExternalArtifact(input: {
                    conversationId: "conversation:foreign"
                    title: "Foreign"
                    artifactKind: "document"
                    externalUrl: "https://example.com/foreign"
                  }) {
                    artifactId
                  }
                }
                "#,
            )
            .await;
        assert_eq!(mutation_response.errors.len(), 1);
        assert!(
            mutation_response.errors[0]
                .message
                .contains("request is unauthenticated")
        );

        let mut stream = schema.execute_stream(async_graphql::Request::new(
            r#"
            subscription {
              conversationEvents(conversationId: "conversation:foreign") {
                __typename
              }
            }
            "#,
        ));
        let subscription_response = stream.next().await.expect("subscription response");
        assert_eq!(subscription_response.errors.len(), 1);
        assert!(
            subscription_response.errors[0]
                .message
                .contains("request is unauthenticated")
        );
    }

    fn conversation_for_human(human_id: &str) -> noema_conversations::NewConversation {
        let mut conversation = noema_conversations::NewConversation::local_chat(None, None);
        conversation.owner =
            noema_conversations::ConversationOwnerRef::human(human_id).expect("human owner");
        conversation.primary_human_id = Some(human_id.to_string());
        conversation
    }

    async fn schema_with_reasoning_profile() -> (GraphqlSchema, String) {
        use crate::test_support::test_store;

        let store = test_store().await;
        store.ensure_default_actors().await.expect("actors");
        let account = store
            .ensure_default_provider_account()
            .await
            .expect("Codex account");
        let account_id = account.provider_account_id;
        store
            .update_provider_account_status(
                &account_id,
                noema_providers::ProviderAccountStatus::Authenticated,
                None,
                None,
            )
            .await
            .expect("authenticated account");
        store
            .update_provider_account_metadata(
                &account_id,
                serde_json::json!({
                    "profiles": [{
                        "id": "gpt-5.5",
                        "label": "GPT-5.5",
                        "reasoning_efforts": ["low", "medium", "high"],
                        "default_reasoning_effort": "medium"
                    }]
                }),
            )
            .await
            .expect("metadata");
        (
            build_schema(GraphqlState::for_tests_with_store(store)),
            account_id,
        )
    }

    mod runtime_support {
        use super::*;
        include!("schema_tests/runtime_support.rs");
    }

    use runtime_support::*;

    mod memory {
        use super::*;
        include!("schema_tests/memory_graph.rs");
        include!("schema_tests/memory_article.rs");
    }

    mod provider_accounts {
        use super::*;
        include!("schema_tests/provider_accounts.rs");
    }

    mod agents {
        use super::*;
        include!("schema_tests/agents.rs");
    }

    mod settings {
        use super::*;
        include!("schema_tests/settings.rs");
    }

    mod conversations {
        use super::*;
        include!("schema_tests/conversations.rs");
        include!("schema_tests/agent_errors.rs");
    }

    mod mcp {
        use super::*;
        include!("schema_tests/mcp_setup.rs");
        include!("schema_tests/mcp_calibration.rs");
    }

    mod artifacts {
        use super::*;
        include!("schema_tests/artifacts.rs");
    }

    mod subscriptions {
        use super::*;
        include!("schema_tests/subscriptions.rs");
    }
}
