#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[tokio::test]
    async fn runtime_turn_error_publishes_error_notice_and_completion() {
        let subscriptions = RuntimeEventRegistry::default();
        let mut rx = subscriptions.subscribe_conversation("conversation_1");

        publish_turn_terminal_events(
            &subscriptions,
            "conversation_1".to_string(),
            Some("client_1".to_string()),
            false,
            Err(noema_runtime::RuntimeError::Remote(
                "provider failed".to_string(),
            )),
        );

        let event = rx.recv().await.expect("error notice event");
        let ConversationRuntimeEvent::Turn {
            client_message_id,
            event,
        } = event
        else {
            panic!("expected turn event");
        };
        assert_eq!(client_message_id.as_deref(), Some("client_1"));
        let noema_runtime::TurnStreamEvent::ConversationItem {
            conversation_id,
            item_id,
            metadata,
            item,
            ..
        } = *event
        else {
            panic!("expected conversation item");
        };
        assert_eq!(conversation_id, "conversation_1");
        assert_eq!(item_id, "graphql_runtime_error:conversation_1:client_1");
        assert_eq!(metadata, json!({}));
        let noema_runtime::TurnTranscriptItem::ErrorNotice {
            message,
            recoverable,
        } = *item
        else {
            panic!("expected error notice");
        };
        assert!(message.contains("provider failed"));
        assert!(!recoverable);

        let event = rx.recv().await.expect("completion event");
        let ConversationRuntimeEvent::Completed {
            conversation_id,
            client_message_id,
        } = event
        else {
            panic!("expected completion event");
        };
        assert_eq!(conversation_id, "conversation_1");
        assert_eq!(client_message_id.as_deref(), Some("client_1"));
    }

    #[tokio::test]
    async fn runtime_turn_error_does_not_duplicate_published_error_notice() {
        let subscriptions = RuntimeEventRegistry::default();
        let mut rx = subscriptions.subscribe_conversation("conversation_1");
        subscriptions.publish_conversation(ConversationRuntimeEvent::Turn {
            client_message_id: Some("client_1".to_string()),
            event: Box::new(TurnStreamEvent::ConversationItem {
                conversation_id: "conversation_1".to_string(),
                item_id: "item:persisted_error".to_string(),
                cursor: None,
                turn_id: Some("turn_1".to_string()),
                metadata: json!({}),
                item: Box::new(noema_runtime::TurnTranscriptItem::ErrorNotice {
                    message: "provider failed".to_string(),
                    recoverable: false,
                }),
            }),
        });

        publish_turn_terminal_events(
            &subscriptions,
            "conversation_1".to_string(),
            Some("client_1".to_string()),
            true,
            Err(noema_runtime::RuntimeError::Remote(
                "provider failed".to_string(),
            )),
        );

        let mut error_notice_count = 0;
        let mut completion_count = 0;
        while let Ok(event) = rx.try_recv() {
            match event {
                ConversationRuntimeEvent::Turn { event, .. } => {
                    if matches!(
                        *event,
                        TurnStreamEvent::ConversationItem {
                            item,
                            ..
                        } if matches!(
                            item.as_ref(),
                            noema_runtime::TurnTranscriptItem::ErrorNotice { .. }
                        )
                    ) {
                        error_notice_count += 1;
                    }
                }
                ConversationRuntimeEvent::Completed { .. } => {
                    completion_count += 1;
                }
            }
        }

        assert_eq!(error_notice_count, 1);
        assert_eq!(completion_count, 1);
    }
}
