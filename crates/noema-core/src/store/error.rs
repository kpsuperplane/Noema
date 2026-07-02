use thiserror::Error;

/// Errors produced by the embedded canonical store.
#[derive(Debug, Error)]
pub enum StoreError {
    /// Store path could not be prepared.
    #[error("failed to prepare store path: {0}")]
    PreparePath(std::io::Error),
    /// Embedded SurrealDB operation failed.
    #[error("embedded store operation failed: {0}")]
    Surreal(Box<surrealdb::Error>),
    /// Schema bootstrap returned an invalid result.
    #[error("store schema bootstrap failed: {0}")]
    Schema(String),
    /// A stored value did not match a closed Noema vocabulary.
    #[error("invalid {kind} value in embedded store: {value}")]
    InvalidEnum {
        /// Vocabulary kind.
        kind: &'static str,
        /// Stored value.
        value: String,
    },
    /// A provider account expected to exist was not found.
    #[error("provider account not found: {provider_account_id}")]
    ProviderAccountNotFound {
        /// Missing provider account id.
        provider_account_id: String,
    },
    /// An agent expected to exist was not found.
    #[error("agent not found: {agent_id}")]
    AgentNotFound {
        /// Missing agent id.
        agent_id: String,
    },
    /// Agent display name input was empty after trimming whitespace.
    #[error("agent display name cannot be empty")]
    AgentDisplayNameEmpty,
    /// A conversation expected to exist was not found.
    #[error("conversation not found: {conversation_id}")]
    ConversationNotFound {
        /// Missing conversation id.
        conversation_id: String,
    },
    /// A turn expected to exist was not found.
    #[error("conversation turn not found: {turn_id}")]
    ConversationTurnNotFound {
        /// Missing turn id.
        turn_id: String,
    },
    /// A turn belongs to a different conversation than the item being written.
    #[error("conversation turn {turn_id} does not belong to conversation {conversation_id}")]
    ConversationTurnConversationMismatch {
        /// Referenced turn id.
        turn_id: String,
        /// Expected conversation id.
        conversation_id: String,
    },
    /// A referenced conversation item expected to exist was not found.
    #[error("conversation item not found: {item_id}")]
    ConversationItemNotFound {
        /// Missing item id.
        item_id: String,
    },
    /// A predicate expected to exist was not found.
    #[error("predicate not found: {predicate_id}")]
    PredicateNotFound {
        /// Missing predicate id.
        predicate_id: String,
    },
    /// A referenced item belongs to a different conversation than the owner row.
    #[error("conversation item {item_id} does not belong to conversation {conversation_id}")]
    ConversationItemConversationMismatch {
        /// Referenced item id.
        item_id: String,
        /// Expected conversation id.
        conversation_id: String,
    },
}

impl From<surrealdb::Error> for StoreError {
    fn from(source: surrealdb::Error) -> Self {
        Self::Surreal(Box::new(source))
    }
}

impl StoreError {
    /// Return true when this store error represents a Noema schema invariant failure.
    #[must_use]
    pub fn is_system_invariant(&self) -> bool {
        matches!(self, Self::Schema(_) | Self::InvalidEnum { .. })
    }

    /// Convert this store invariant into a system error event.
    #[must_use]
    pub fn system_error_event(
        &self,
        context: serde_json::Value,
        raw: serde_json::Value,
    ) -> Option<crate::SystemErrorEvent> {
        self.is_system_invariant().then(|| {
            crate::SystemErrorEvent::new(crate::SYSTEM_ERROR_STORE_INVARIANT, self.to_string())
                .with_context(context)
                .with_error_chain([self.to_string()])
                .with_raw(raw)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn schema_errors_build_system_error_events() {
        let error = StoreError::Schema("bad row".to_string());

        let event = error
            .system_error_event(
                json!({"table": "mcp_tools"}),
                json!({"row": {"status": "bad"}}),
            )
            .expect("event");

        assert_eq!(event.category, crate::SYSTEM_ERROR_STORE_INVARIANT);
        assert_eq!(event.context["table"], "mcp_tools");
        assert_eq!(event.raw["row"]["status"], "bad");
    }

    #[test]
    fn missing_records_are_not_store_invariants() {
        let error = StoreError::ConversationNotFound {
            conversation_id: "conversation:missing".to_string(),
        };

        assert!(error.system_error_event(json!({}), json!({})).is_none());
    }
}
