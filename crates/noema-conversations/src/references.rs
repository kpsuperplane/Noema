use crate::ConversationError;

/// A reference to the actor that authored a conversation item.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ActorRef {
    /// Actor interface id.
    pub actor_id: String,
}

impl ActorRef {
    fn from_id(actor_id: impl Into<String>) -> Result<Self, ConversationError> {
        let actor_id = actor_id.into();
        if actor_id.trim().is_empty() {
            return Err(ConversationError::EmptyReferenceId {
                reference_kind: "actor",
            });
        }
        Ok(Self { actor_id })
    }

    /// Reference a human actor row.
    ///
    /// # Errors
    ///
    /// Returns [`ConversationError::EmptyReferenceId`] when the id is empty or
    /// only whitespace.
    pub fn human(actor_id: impl Into<String>) -> Result<Self, ConversationError> {
        Self::from_id(actor_id)
    }

    /// Reference an agent actor row.
    ///
    /// # Errors
    ///
    /// Returns [`ConversationError::EmptyReferenceId`] when the id is empty or
    /// only whitespace.
    pub fn agent(actor_id: impl Into<String>) -> Result<Self, ConversationError> {
        Self::from_id(actor_id)
    }

    /// Reference a system actor row.
    ///
    /// # Errors
    ///
    /// Returns [`ConversationError::EmptyReferenceId`] when the id is empty or
    /// only whitespace.
    pub fn system(actor_id: impl Into<String>) -> Result<Self, ConversationError> {
        Self::from_id(actor_id)
    }
}

/// Closed set of object kinds that may own a durable conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConversationOwnerKind {
    /// A human user or collaborator.
    Human,
    /// A Noema agent.
    Agent,
    /// Another durable conversation.
    Conversation,
    /// A workspace.
    Workspace,
    /// A project.
    Project,
    /// A durable background task.
    Task,
    /// A callable tool or integration.
    Tool,
}

impl ConversationOwnerKind {
    /// Return the stable storage string for this owner kind.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Agent => "agent",
            Self::Conversation => "conversation",
            Self::Workspace => "workspace",
            Self::Project => "project",
            Self::Task => "task",
            Self::Tool => "tool",
        }
    }
}

/// A conversation-owned reference that preserves persisted owner wire fields.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConversationOwnerRef {
    /// Stored owner object discriminator.
    pub object_type: ConversationOwnerKind,
    /// Stored owner object id.
    pub object_id: String,
}

impl ConversationOwnerRef {
    /// Construct a conversation owner reference.
    ///
    /// # Errors
    ///
    /// Returns [`ConversationError::EmptyReferenceId`] when the id is empty or
    /// only whitespace.
    pub fn new(
        object_type: ConversationOwnerKind,
        object_id: impl Into<String>,
    ) -> Result<Self, ConversationError> {
        let object_id = object_id.into();
        if object_id.trim().is_empty() {
            return Err(ConversationError::EmptyReferenceId {
                reference_kind: "conversation_owner",
            });
        }
        Ok(Self {
            object_type,
            object_id,
        })
    }

    /// Reference a human-owned conversation.
    ///
    /// # Errors
    ///
    /// Returns [`ConversationError::EmptyReferenceId`] when the id is empty or
    /// only whitespace.
    pub fn human(object_id: impl Into<String>) -> Result<Self, ConversationError> {
        Self::new(ConversationOwnerKind::Human, object_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references_preserve_wire_values_valid_ids_and_validation_boundaries() {
        for (kind, value) in [
            (ConversationOwnerKind::Human, "human"),
            (ConversationOwnerKind::Agent, "agent"),
            (ConversationOwnerKind::Conversation, "conversation"),
            (ConversationOwnerKind::Workspace, "workspace"),
            (ConversationOwnerKind::Project, "project"),
            (ConversationOwnerKind::Task, "task"),
            (ConversationOwnerKind::Tool, "tool"),
        ] {
            assert_eq!(kind.as_str(), value);
        }
        let actor = ActorRef::agent("agent:primary").expect("valid actor");
        assert_eq!(actor.actor_id, "agent:primary");

        let owner = ConversationOwnerRef::new(ConversationOwnerKind::Workspace, "workspace:local")
            .expect("valid owner");
        assert_eq!(owner.object_type, ConversationOwnerKind::Workspace);
        assert_eq!(owner.object_id, "workspace:local");
        for value in ["", "   ", "\t\n"] {
            assert_eq!(
                ActorRef::human(value),
                Err(ConversationError::EmptyReferenceId {
                    reference_kind: "actor",
                })
            );
            assert_eq!(
                ConversationOwnerRef::new(ConversationOwnerKind::Human, value),
                Err(ConversationError::EmptyReferenceId {
                    reference_kind: "conversation_owner",
                })
            );
        }
    }
}
