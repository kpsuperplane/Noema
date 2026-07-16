use std::{fmt, str::FromStr};

use crate::ConversationError;

/// Closed set of actor interface kinds that can author conversation items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActorKind {
    /// Actor backed by a human profile.
    Human,
    /// Actor backed by an agent profile.
    Agent,
    /// System actor without a concrete human or agent profile.
    System,
}

impl ActorKind {
    /// Return the stable storage string for this actor kind.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Agent => "agent",
            Self::System => "system",
        }
    }

    /// Parse a stored actor kind.
    ///
    /// # Errors
    ///
    /// Returns [`ConversationError::InvalidEnum`] for unknown values.
    pub fn parse(value: &str) -> Result<Self, ConversationError> {
        match value {
            "human" => Ok(Self::Human),
            "agent" => Ok(Self::Agent),
            "system" => Ok(Self::System),
            _ => Err(ConversationError::InvalidEnum {
                kind: "actor_kind",
                value: value.to_string(),
            }),
        }
    }
}

impl FromStr for ActorKind {
    type Err = ConversationError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

/// A reference to the actor that authored a conversation item.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ActorRef {
    /// Actor kind discriminator.
    pub actor_kind: ActorKind,
    /// Actor interface id.
    pub actor_id: String,
}

impl ActorRef {
    /// Construct an actor reference.
    ///
    /// # Errors
    ///
    /// Returns [`ConversationError::EmptyReferenceId`] when the id is empty or
    /// only whitespace.
    pub fn new(
        actor_kind: ActorKind,
        actor_id: impl Into<String>,
    ) -> Result<Self, ConversationError> {
        let actor_id = actor_id.into();
        if actor_id.trim().is_empty() {
            return Err(ConversationError::EmptyReferenceId {
                reference_kind: "actor",
            });
        }
        Ok(Self {
            actor_kind,
            actor_id,
        })
    }

    /// Reference a human actor row.
    ///
    /// # Errors
    ///
    /// Returns [`ConversationError::EmptyReferenceId`] when the id is empty or
    /// only whitespace.
    pub fn human(actor_id: impl Into<String>) -> Result<Self, ConversationError> {
        Self::new(ActorKind::Human, actor_id)
    }

    /// Reference an agent actor row.
    ///
    /// # Errors
    ///
    /// Returns [`ConversationError::EmptyReferenceId`] when the id is empty or
    /// only whitespace.
    pub fn agent(actor_id: impl Into<String>) -> Result<Self, ConversationError> {
        Self::new(ActorKind::Agent, actor_id)
    }

    /// Reference a system actor row.
    ///
    /// # Errors
    ///
    /// Returns [`ConversationError::EmptyReferenceId`] when the id is empty or
    /// only whitespace.
    pub fn system(actor_id: impl Into<String>) -> Result<Self, ConversationError> {
        Self::new(ActorKind::System, actor_id)
    }
}

impl fmt::Display for ActorRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.actor_id)
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

    /// Parse a stored conversation owner kind.
    ///
    /// # Errors
    ///
    /// Returns [`ConversationError::InvalidEnum`] for unknown values.
    pub fn parse(value: &str) -> Result<Self, ConversationError> {
        match value {
            "human" => Ok(Self::Human),
            "agent" => Ok(Self::Agent),
            "conversation" => Ok(Self::Conversation),
            "workspace" => Ok(Self::Workspace),
            "project" => Ok(Self::Project),
            "task" => Ok(Self::Task),
            "tool" => Ok(Self::Tool),
            _ => Err(ConversationError::InvalidEnum {
                kind: "conversation_owner_kind",
                value: value.to_string(),
            }),
        }
    }
}

impl FromStr for ConversationOwnerKind {
    type Err = ConversationError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
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
    fn actor_kind_round_trips_every_wire_value() {
        for (value, expected) in [
            ("human", ActorKind::Human),
            ("agent", ActorKind::Agent),
            ("system", ActorKind::System),
        ] {
            let parsed = ActorKind::parse(value).expect("valid actor kind");
            assert_eq!(parsed, expected);
            assert_eq!(parsed.as_str(), value);
        }
    }

    #[test]
    fn conversation_owner_kind_round_trips_every_schema_wire_value() {
        for (value, expected) in [
            ("human", ConversationOwnerKind::Human),
            ("agent", ConversationOwnerKind::Agent),
            ("conversation", ConversationOwnerKind::Conversation),
            ("workspace", ConversationOwnerKind::Workspace),
            ("project", ConversationOwnerKind::Project),
            ("task", ConversationOwnerKind::Task),
            ("tool", ConversationOwnerKind::Tool),
        ] {
            let parsed = ConversationOwnerKind::parse(value).expect("valid owner kind");
            assert_eq!(parsed, expected);
            assert_eq!(parsed.as_str(), value);
        }
    }

    #[test]
    fn reference_vocabulary_rejects_unknown_values() {
        assert!(matches!(
            ActorKind::parse("robot"),
            Err(ConversationError::InvalidEnum {
                kind: "actor_kind",
                ..
            })
        ));
        assert!(matches!(
            ConversationOwnerKind::parse("conversation_item"),
            Err(ConversationError::InvalidEnum {
                kind: "conversation_owner_kind",
                ..
            })
        ));
    }

    #[test]
    fn references_preserve_valid_ids_and_discriminators() {
        let actor = ActorRef::agent("agent:primary").expect("valid actor");
        assert_eq!(actor.actor_kind, ActorKind::Agent);
        assert_eq!(actor.actor_id, "agent:primary");
        assert_eq!(actor.to_string(), "agent:primary");

        let owner = ConversationOwnerRef::new(ConversationOwnerKind::Workspace, "workspace:local")
            .expect("valid owner");
        assert_eq!(owner.object_type, ConversationOwnerKind::Workspace);
        assert_eq!(owner.object_id, "workspace:local");
    }

    #[test]
    fn actor_ref_rejects_empty_and_whitespace_ids() {
        for value in ["", "   ", "\t\n"] {
            assert_eq!(
                ActorRef::new(ActorKind::Human, value),
                Err(ConversationError::EmptyReferenceId {
                    reference_kind: "actor",
                })
            );
        }
    }

    #[test]
    fn conversation_owner_ref_rejects_empty_and_whitespace_ids() {
        for value in ["", "   ", "\t\n"] {
            assert_eq!(
                ConversationOwnerRef::new(ConversationOwnerKind::Human, value),
                Err(ConversationError::EmptyReferenceId {
                    reference_kind: "conversation_owner",
                })
            );
        }
    }
}
