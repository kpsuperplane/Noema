#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::StreamExt;
    use noema_runtime::{ConversationRuntimeEvent, TurnStreamEvent};
    use serde_json::json;

    macro_rules! assert_json_fields {
        ($value:expr, $($pointer:literal => $expected:expr),+ $(,)?) => {{
            let value = &$value;
            $(assert_eq!(
                value.pointer($pointer),
                Some(&serde_json::json!($expected)),
                "unexpected JSON field at {}",
                $pointer,
            );)+
        }};
    }

    fn assert_single_graphql_error(response: &async_graphql::Response, expected: &str) {
        assert_eq!(response.errors.len(), 1, "{:?}", response.errors);
        assert!(response.errors[0].message.contains(expected));
    }

    #[test]
    fn schema_sdl_exposes_initial_noema_fields() {
        let production_sdl = schema_sdl().replace("\n\ttestRequestPrincipal: String!", "");
        assert_eq!(
            production_sdl,
            include_str!("../../../../apps/web/src/generated/schema.graphql")
        );
    }

    #[tokio::test]
    async fn owner_sensitive_operations_require_a_request_principal() {
        let schema = build_schema_without_request_principal(GraphqlState::for_tests());
        let query_response = schema
            .execute(
                r#"query {
                  task(taskId: "task:foreign") { taskId }
                  artifact(artifactId: "artifact:foreign") { artifactId }
                  conversationTranscriptPage(input: { conversationId: "conversation:foreign" }) {
                    pageInfo { limit }
                  }
                }"#,
            )
            .await;
        assert_eq!(query_response.errors.len(), 3);
        assert!(
            query_response
                .errors
                .iter()
                .all(|error| { error.message.contains("request is unauthenticated") })
        );

        let mutation_response = schema
            .execute(
                r#"mutation {
                  createConversationExternalArtifact(input: {
                    conversationId: "conversation:foreign", title: "Foreign",
                    artifactKind: "document", externalUrl: "https://example.com/foreign"
                  }) { artifactId }
                }"#,
            )
            .await;
        assert_single_graphql_error(&mutation_response, "request is unauthenticated");

        let mut stream = schema.execute_stream(async_graphql::Request::new(
            r#"subscription {
              conversationEvents(conversationId: "conversation:foreign") { __typename }
            }"#,
        ));
        let subscription_response = stream.next().await.expect("subscription response");
        assert_single_graphql_error(&subscription_response, "request is unauthenticated");
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
        let account = crate::test_support::authenticated_default_provider(&store).await;
        let account_id = account.provider_account_id;
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

    mod settings {
        use super::*;
        include!("schema_tests/settings.rs");
    }

    mod mcp_boundaries {
        use super::*;
        include!("schema_tests/mcp_boundaries.rs");
    }

    mod conversations {
        use super::*;
        include!("schema_tests/conversations.rs");
        include!("schema_tests/agent_errors.rs");
    }

    mod artifacts {
        use super::*;
        include!("schema_tests/artifacts.rs");
    }

    mod subscriptions {
        use super::*;
        include!("schema_tests/subscriptions.rs");
    }

    mod work_graphql {
        use super::*;
        include!("schema_tests/work_graphql.rs");
    }

}
