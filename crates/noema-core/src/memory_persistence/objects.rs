use std::{fmt, str::FromStr};

use rusqlite::{Connection, OptionalExtension, params};

use super::MemoryPersistenceError;

/// Closed set of concrete object types that can be referenced polymorphically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectType {
    /// A human user or collaborator.
    Human,
    /// A Noema agent.
    Agent,
    /// A callable tool or integration.
    Tool,
    /// A durable conversation thread.
    Conversation,
    /// A causal conversation turn.
    ConversationTurn,
    /// A canonical conversation stream item.
    ConversationItem,
    /// A durable memory record.
    MemoryItem,
    /// A semantic graph entity.
    Entity,
    /// A semantic graph relationship.
    Relationship,
    /// A retrieval/context packet.
    ContextPacket,
}

impl ObjectType {
    /// Return the stable storage string for this object type.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Agent => "agent",
            Self::Tool => "tool",
            Self::Conversation => "conversation",
            Self::ConversationTurn => "conversation_turn",
            Self::ConversationItem => "conversation_item",
            Self::MemoryItem => "memory_item",
            Self::Entity => "entity",
            Self::Relationship => "relationship",
            Self::ContextPacket => "context_packet",
        }
    }

    /// Parse a storage string into a known object type.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError::InvalidObjectType`] when `value` is
    /// not in the closed object type vocabulary.
    pub fn parse(value: &str) -> Result<Self, MemoryPersistenceError> {
        match value {
            "human" => Ok(Self::Human),
            "agent" => Ok(Self::Agent),
            "tool" => Ok(Self::Tool),
            "conversation" => Ok(Self::Conversation),
            "conversation_turn" => Ok(Self::ConversationTurn),
            "conversation_item" => Ok(Self::ConversationItem),
            "memory_item" => Ok(Self::MemoryItem),
            "entity" => Ok(Self::Entity),
            "relationship" => Ok(Self::Relationship),
            "context_packet" => Ok(Self::ContextPacket),
            _ => Err(MemoryPersistenceError::InvalidObjectType {
                value: value.to_string(),
            }),
        }
    }

    /// Return the concrete table that stores this object type.
    #[must_use]
    pub const fn table_name(self) -> &'static str {
        match self {
            Self::Human => "humans",
            Self::Agent => "agents",
            Self::Tool => "tools",
            Self::Conversation => "conversations",
            Self::ConversationTurn => "conversation_turns",
            Self::ConversationItem => "conversation_items",
            Self::MemoryItem => "memory_items",
            Self::Entity => "entities",
            Self::Relationship => "relationships",
            Self::ContextPacket => "context_packets",
        }
    }

    /// Return the primary key column for this object type's concrete table.
    #[must_use]
    pub const fn id_column(self) -> &'static str {
        match self {
            Self::Human => "human_id",
            Self::Agent => "agent_id",
            Self::Tool => "tool_id",
            Self::Conversation => "conversation_id",
            Self::ConversationTurn => "turn_id",
            Self::ConversationItem => "item_id",
            Self::MemoryItem => "memory_id",
            Self::Entity => "entity_id",
            Self::Relationship => "relationship_id",
            Self::ContextPacket => "context_packet_id",
        }
    }
}

impl FromStr for ObjectType {
    type Err = MemoryPersistenceError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

/// A typed reference to a concrete object row.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ObjectRef {
    /// Concrete object type.
    pub object_type: ObjectType,
    /// Concrete object id.
    pub object_id: String,
}

impl ObjectRef {
    /// Construct a typed object reference.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError::EmptyObjectId`] when the id is empty
    /// or only whitespace.
    pub fn new(
        object_type: ObjectType,
        object_id: impl Into<String>,
    ) -> Result<Self, MemoryPersistenceError> {
        let object_id = object_id.into();
        if object_id.trim().is_empty() {
            return Err(MemoryPersistenceError::EmptyObjectId {
                object_type: object_type.as_str().to_string(),
            });
        }
        Ok(Self {
            object_type,
            object_id,
        })
    }

    /// Reference a human row.
    #[must_use]
    pub fn human(object_id: impl Into<String>) -> Self {
        Self {
            object_type: ObjectType::Human,
            object_id: object_id.into(),
        }
    }

    /// Reference an agent row.
    #[must_use]
    pub fn agent(object_id: impl Into<String>) -> Self {
        Self {
            object_type: ObjectType::Agent,
            object_id: object_id.into(),
        }
    }

    /// Reference a conversation item row.
    #[must_use]
    pub fn conversation_item(object_id: impl Into<String>) -> Self {
        Self {
            object_type: ObjectType::ConversationItem,
            object_id: object_id.into(),
        }
    }
}

impl fmt::Display for ObjectRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}:{}",
            self.object_type.as_str(),
            self.object_id
        )
    }
}

pub(super) fn validate_object_ref_for_conn(
    conn: &Connection,
    object_ref: &ObjectRef,
) -> Result<(), MemoryPersistenceError> {
    let sql = format!(
        "SELECT 1 FROM {} WHERE {} = ?1 LIMIT 1",
        object_ref.object_type.table_name(),
        object_ref.object_type.id_column()
    );
    let exists = conn
        .query_row(&sql, params![object_ref.object_id.as_str()], |_| Ok(()))
        .optional()
        .map_err(MemoryPersistenceError::Sqlite)?
        .is_some();
    if exists {
        Ok(())
    } else {
        Err(MemoryPersistenceError::ObjectRefNotFound {
            object_type: object_ref.object_type.as_str().to_string(),
            object_id: object_ref.object_id.clone(),
        })
    }
}
