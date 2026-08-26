#[tokio::test]
async fn missing_primary_provider_returns_no_primary_conversation() {
    for conversation_exists in [false, true] {
        let store = crate::test_support::test_store().await;
        if conversation_exists {
            store
                .create_conversation(conversation_for_human("human:local"))
                .await
                .expect("primary conversation");
        }
        let schema = build_schema(GraphqlState::for_tests_with_store(store));

        let response = schema
            .execute("{ primaryConversation { conversationId provider } }")
            .await;

        assert!(response.errors.is_empty(), "{:?}", response.errors);
        assert_eq!(response.data.into_json().expect("response data"), json!({
            "primaryConversation": null,
        }));
    }
}

#[tokio::test]
async fn conversation_operations_reject_foreign_human_conversations() {
    // Case: conversation_operations_reject_foreign_human_conversations.
        let store = crate::test_support::test_store().await;
        let conversation = store
            .create_conversation(conversation_for_human("human:other"))
            .await
            .expect("foreign conversation");
        let schema = build_schema(GraphqlState::for_tests_with_store(store));

        let query_response = schema
            .execute(async_graphql::Request::new(format!(
                r#"query {{
                  conversationTranscriptPage(input: {{ conversationId: "{}" }}) {{
                    pageInfo {{ hasMoreBefore }}
                  }}
                }}"#,
                conversation.conversation_id,
            )))
            .await;
        assert_single_graphql_error(&query_response, "conversation is unavailable");

        let turn_response = schema
            .execute(async_graphql::Request::new(format!(
                r#"mutation {{
                  sendConversationTurn(input: {{
                    conversationId: "{}", input: "private", clientMessageId: "client:foreign"
                  }}) {{ conversationId }}
                }}"#,
                conversation.conversation_id,
            )))
            .await;
        assert_single_graphql_error(&turn_response, "conversation is unavailable");

        let selection_response = schema
            .execute(async_graphql::Request::new(format!(
                r#"mutation {{
                  sendMultipleChoiceSelection(input: {{
                    conversationId: "{}", promptItemId: "item:foreign",
                    selectedOptionIds: ["option:foreign"], clientMessageId: "client:foreign"
                  }}) {{ conversationId }}
                }}"#,
                conversation.conversation_id,
            )))
            .await;
    assert_single_graphql_error(&selection_response, "conversation is unavailable");
}
