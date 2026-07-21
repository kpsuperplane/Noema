//! Canonical bounded authorization context derived from durable conversation items.

use noema_tasks::{
    TASK_AUTHORIZATION_CONTEXT_MAX_MESSAGES, TaskAuthorizationContext, TaskAuthorizationMessage,
    TaskAuthorizationMessageRole, WorkDomainError,
};
use rusqlite::{Connection, OptionalExtension, params};

use crate::{NoemaStore, StoreError};

pub(crate) const MAX_AUTHORIZATION_CONTEXT_BYTES: usize = 262_144;

impl NoemaStore {
    /// Read a bounded conversation authorization context ending at one exact
    /// authenticated human item.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the source item is missing, mismatched,
    /// non-human, incomplete, or the resulting context exceeds its byte bound.
    pub async fn conversation_authorization_context(
        &self,
        conversation_id: &str,
        turn_id: &str,
        item_id: &str,
    ) -> Result<TaskAuthorizationContext, StoreError> {
        self.with_connection(|connection| {
            conversation_authorization_context(connection, conversation_id, turn_id, item_id)
        })
        .await
    }
}

pub(crate) fn conversation_authorization_context(
    connection: &Connection,
    conversation_id: &str,
    turn_id: &str,
    item_id: &str,
) -> Result<TaskAuthorizationContext, StoreError> {
    let anchor = connection
        .query_row(
            r#"
            SELECT item.conversation_id, item.turn_id, item.sequence_index, item.kind,
                   item.status, item.author_actor_id, conversation.primary_human_id
            FROM conversation_items AS item
            JOIN conversations AS conversation
              ON conversation.conversation_id = item.conversation_id
            WHERE item.item_id = ?1 AND item.deleted_at IS NULL
            LIMIT 1
            "#,
            [item_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, Option<String>>(6)?,
                ))
            },
        )
        .optional()?;
    let Some((
        anchor_conversation_id,
        anchor_turn_id,
        anchor_sequence,
        anchor_kind,
        status,
        anchor_author_id,
        primary_human_id,
    )) = anchor
    else {
        return Err(invalid_context("source human item is unavailable"));
    };
    if anchor_conversation_id != conversation_id || anchor_turn_id.as_deref() != Some(turn_id) {
        return Err(invalid_context(
            "source human item does not match task provenance",
        ));
    }
    if !matches!(
        anchor_kind.as_str(),
        "user_text" | "multiple_choice_selection"
    ) || status != "completed"
        || primary_human_id.as_deref() != Some(anchor_author_id.as_str())
    {
        return Err(invalid_context(
            "authorization context must end at a completed human message",
        ));
    }

    let mut statement = connection.prepare(
        r#"
        SELECT item_id, kind, author_actor_id, content_text
        FROM conversation_items
        WHERE conversation_id = ?1
          AND deleted_at IS NULL
          AND sequence_index <= ?2
          AND status = 'completed'
          AND kind IN (
            'user_text', 'assistant_text',
            'multiple_choice_prompt', 'multiple_choice_selection'
          )
          AND content_text IS NOT NULL
        ORDER BY sequence_index DESC
        LIMIT ?3
        "#,
    )?;
    let rows = statement.query_map(
        params![
            conversation_id,
            anchor_sequence,
            i64::try_from(TASK_AUTHORIZATION_CONTEXT_MAX_MESSAGES).map_err(|_| {
                StoreError::InvariantViolation {
                    message: "authorization context message limit exceeds SQLite range".to_string(),
                }
            })?,
        ],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        },
    )?;
    let mut messages = rows
        .map(|row| {
            let (item_id, kind, author_actor_id, text) = row?;
            let role = match kind.as_str() {
                "user_text" | "multiple_choice_selection" => TaskAuthorizationMessageRole::Human,
                "assistant_text" | "multiple_choice_prompt" => {
                    TaskAuthorizationMessageRole::Assistant
                }
                _ => {
                    return Err(rusqlite::Error::InvalidColumnType(
                        1,
                        "kind".to_string(),
                        rusqlite::types::Type::Text,
                    ));
                }
            };
            if role == TaskAuthorizationMessageRole::Human
                && primary_human_id.as_deref() != Some(author_actor_id.as_str())
            {
                return Err(rusqlite::Error::InvalidQuery);
            }
            Ok(TaskAuthorizationMessage {
                item_id,
                role,
                text,
            })
        })
        .collect::<Result<Vec<_>, rusqlite::Error>>()?;
    messages.reverse();
    if messages.last().map(|message| message.item_id.as_str()) != Some(item_id) {
        return Err(invalid_context(
            "source human item has no usable authorization text",
        ));
    }
    let context = TaskAuthorizationContext::ConversationExcerpt { messages };
    bounded_authorization_context_json(&context)?;
    Ok(context)
}

pub(crate) fn bounded_authorization_context_json(
    context: &TaskAuthorizationContext,
) -> Result<String, StoreError> {
    context.validate().map_err(StoreError::Work)?;
    let serialized = serde_json::to_string(context)?;
    if serialized.len() > MAX_AUTHORIZATION_CONTEXT_BYTES {
        return Err(invalid_context(
            "authorization context exceeds bounded size",
        ));
    }
    Ok(serialized)
}

fn invalid_context(message: &str) -> StoreError {
    StoreError::Work(WorkDomainError::InvalidInput {
        field: "task.authorization_context",
        message: message.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use noema_conversations::{
        ActorRef, ConversationItemKind, ConversationItemStatus, NewConversationItem,
        NewConversationTurn,
    };
    use serde_json::json;

    use super::*;

    #[tokio::test]
    async fn excerpt_is_role_labelled_bounded_and_ends_at_source_human() {
        let store = crate::tests::test_store().await;
        store.ensure_default_actors().await.expect("actors");
        let conversation = store
            .get_or_create_primary_conversation("human:local", None, None)
            .await
            .expect("conversation");
        let turn = store
            .create_conversation_turn(NewConversationTurn {
                conversation_id: conversation.conversation_id.clone(),
                trigger_item_id: None,
                metadata: json!({}),
            })
            .await
            .expect("turn");

        let kinds = [
            ConversationItemKind::UserText,
            ConversationItemKind::AssistantText,
            ConversationItemKind::MultipleChoicePrompt,
            ConversationItemKind::MultipleChoiceSelection,
            ConversationItemKind::AssistantText,
            ConversationItemKind::UserText,
            ConversationItemKind::AssistantText,
            ConversationItemKind::AssistantText,
            ConversationItemKind::UserText,
        ];
        let mut source = None;
        for (index, kind) in kinds.into_iter().enumerate() {
            let author = if matches!(
                kind,
                ConversationItemKind::UserText | ConversationItemKind::MultipleChoiceSelection
            ) {
                ActorRef::human("human:local").expect("human actor")
            } else {
                ActorRef::agent("agent:primary").expect("agent actor")
            };
            let item = store
                .append_conversation_item(NewConversationItem {
                    conversation_id: conversation.conversation_id.clone(),
                    turn_id: Some(turn.turn_id.clone()),
                    parent_item_id: None,
                    kind,
                    status: ConversationItemStatus::Completed,
                    author,
                    content_text: Some(format!("message-{index}")),
                    payload_json: json!({}),
                    metadata: json!({}),
                })
                .await
                .expect("message");
            source = Some(item);
        }
        store
            .append_conversation_item(NewConversationItem {
                conversation_id: conversation.conversation_id.clone(),
                turn_id: Some(turn.turn_id.clone()),
                parent_item_id: None,
                kind: ConversationItemKind::AssistantText,
                status: ConversationItemStatus::Completed,
                author: ActorRef::agent("agent:primary").expect("agent actor"),
                content_text: Some("same-turn output".to_string()),
                payload_json: json!({}),
                metadata: json!({}),
            })
            .await
            .expect("post-anchor assistant");
        let source = source.expect("source item");

        assert!(
            store
                .conversation_authorization_context(
                    &conversation.conversation_id,
                    "turn:wrong",
                    &source.item_id,
                )
                .await
                .is_err()
        );
        let context = store
            .conversation_authorization_context(
                &conversation.conversation_id,
                &turn.turn_id,
                &source.item_id,
            )
            .await
            .expect("authorization context");
        let TaskAuthorizationContext::ConversationExcerpt { messages } = context else {
            panic!("expected conversation excerpt");
        };
        assert_eq!(
            messages
                .iter()
                .map(|message| message.text.as_str())
                .collect::<Vec<_>>(),
            vec![
                "message-2",
                "message-3",
                "message-4",
                "message-5",
                "message-6",
                "message-7",
                "message-8",
            ]
        );
        assert_eq!(
            messages
                .iter()
                .filter(|message| message.role == TaskAuthorizationMessageRole::Human)
                .map(|message| message.text.as_str())
                .collect::<Vec<_>>(),
            vec!["message-3", "message-5", "message-8"]
        );
        assert_eq!(
            messages.last().map(|message| &message.item_id),
            Some(&source.item_id)
        );

        let forged_source = store
            .append_conversation_item(NewConversationItem {
                conversation_id: conversation.conversation_id.clone(),
                turn_id: Some(turn.turn_id.clone()),
                parent_item_id: None,
                kind: ConversationItemKind::UserText,
                status: ConversationItemStatus::Completed,
                author: ActorRef::agent("agent:primary").expect("agent actor"),
                content_text: Some("forged human kind".to_string()),
                payload_json: json!({}),
                metadata: json!({}),
            })
            .await
            .expect("forged source fixture");
        assert!(
            store
                .conversation_authorization_context(
                    &conversation.conversation_id,
                    &turn.turn_id,
                    &forged_source.item_id,
                )
                .await
                .is_err()
        );
    }

    #[test]
    fn authorization_context_rejects_oversized_exact_text() {
        assert!(
            bounded_authorization_context_json(&TaskAuthorizationContext::ManualTaskBody {
                title: "x".repeat(MAX_AUTHORIZATION_CONTEXT_BYTES),
                description_markdown: String::new(),
            })
            .is_err()
        );
    }
}
