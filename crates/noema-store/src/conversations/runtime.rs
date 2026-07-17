use noema_conversations::{AgentStatus, ConversationRuntimeStatus, ConversationTurnStatus};
use rusqlite::{OptionalExtension, TransactionBehavior, params};

use super::{NoemaStore, StoreError};

impl NoemaStore {
    /// Update the live agent status for a durable conversation.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the conversation is missing or the embedded
    /// store write fails.
    pub async fn update_conversation_agent_status(
        &self,
        conversation_id: &str,
        status: AgentStatus,
    ) -> Result<(), StoreError> {
        self.require_conversation(conversation_id).await?;
        self.with_connection(|conn| {
            conn.execute(
                r#"
                UPDATE conversations
                SET agent_status = ?2,
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                WHERE conversation_id = ?1
                "#,
                params![conversation_id, status.as_str()],
            )?;
            Ok(())
        })
        .await
    }

    /// Return the latest durable turn and live-agent state for a conversation.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the embedded store read fails or a stored
    /// status value is invalid.
    pub async fn conversation_runtime_status(
        &self,
        conversation_id: &str,
    ) -> Result<Option<ConversationRuntimeStatus>, StoreError> {
        let row = self
            .with_connection(|conn| {
                conn.query_row(
                    r#"
                    SELECT turn.status, conversation.agent_status
                    FROM conversations AS conversation
                    JOIN conversation_turns AS turn
                      ON turn.conversation_id = conversation.conversation_id
                    WHERE conversation.conversation_id = ?1
                    ORDER BY turn.created_at DESC
                    LIMIT 1
                    "#,
                    [conversation_id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .optional()
                .map_err(StoreError::Sqlite)
            })
            .await?;
        row.map(|(turn_status, agent_status)| {
            Ok(ConversationRuntimeStatus {
                turn_status: ConversationTurnStatus::parse(&turn_status)?,
                agent_status: AgentStatus::parse(&agent_status)?,
            })
        })
        .transpose()
    }

    /// Recover in-flight conversation state after process shutdown cancels runtime work.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] when the transactional recovery write fails.
    pub async fn recover_shutdown_cancelled_work(
        &self,
        conversation_id: &str,
    ) -> Result<(), StoreError> {
        self.with_connection(|conn| {
            let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            transaction.execute(
                r#"
                UPDATE conversation_items
                SET status = 'cancelled',
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                WHERE conversation_id = ?1
                  AND status IN ('pending', 'running')
                  AND turn_id IN (
                    SELECT turn_id
                    FROM conversation_turns
                    WHERE conversation_id = ?1
                      AND status IN ('input_received', 'running', 'waiting_for_tool')
                  )
                "#,
                [conversation_id],
            )?;
            transaction.execute(
                r#"
                UPDATE conversation_turns
                SET status = 'cancelled',
                    completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                WHERE conversation_id = ?1
                  AND status IN ('input_received', 'running', 'waiting_for_tool')
                "#,
                [conversation_id],
            )?;
            transaction.execute(
                r#"
                UPDATE conversations
                SET agent_status = 'idle',
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                WHERE conversation_id = ?1
                "#,
                [conversation_id],
            )?;
            transaction.commit()?;
            Ok(())
        })
        .await
    }
}
