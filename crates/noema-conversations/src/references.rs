use crate::ConversationError;

/// A reference to the actor that authored a conversation item.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ActorRef {
    /// Actor interface id.
    pub actor_id: String,
}

impl ActorRef {
    /// Reference an actor row.
    ///
    /// # Errors
    ///
    /// Returns [`ConversationError::EmptyReferenceId`] when the id is empty or
    /// only whitespace.
    pub fn new(actor_id: impl Into<String>) -> Result<Self, ConversationError> {
        let actor_id = actor_id.into();
        if actor_id.trim().is_empty() {
            return Err(ConversationError::EmptyReferenceId {
                reference_kind: "actor",
            });
        }
        Ok(Self { actor_id })
    }
}

/// Validated human owner of a durable conversation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConversationOwnerRef {
    /// Human owner identifier.
    pub human_id: String,
}

impl ConversationOwnerRef {
    /// Construct a conversation owner reference.
    ///
    /// # Errors
    ///
    /// Returns [`ConversationError::EmptyReferenceId`] when the id is empty or
    /// only whitespace.
    pub fn human(human_id: impl Into<String>) -> Result<Self, ConversationError> {
        let human_id = human_id.into();
        if human_id.trim().is_empty() {
            return Err(ConversationError::EmptyReferenceId {
                reference_kind: "conversation_owner",
            });
        }
        Ok(Self { human_id })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references_preserve_wire_values_valid_ids_and_validation_boundaries() {
        for value in ["", "   ", "\t\n"] {
            assert_eq!(
                ActorRef::new(value),
                Err(ConversationError::EmptyReferenceId {
                    reference_kind: "actor",
                })
            );
            assert_eq!(
                ConversationOwnerRef::human(value),
                Err(ConversationError::EmptyReferenceId {
                    reference_kind: "conversation_owner",
                })
            );
        }
    }
}
