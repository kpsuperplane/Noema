use std::{fmt, str::FromStr};

use super::{ActorId, MemoryPersistenceError, ObjectId};

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

/// Closed set of actor interface kinds.
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

    /// Parse a storage string into a known actor kind.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError::InvalidEnum`] when `value` is not in
    /// the closed actor kind vocabulary.
    pub fn parse(value: &str) -> Result<Self, MemoryPersistenceError> {
        match value {
            "human" => Ok(Self::Human),
            "agent" => Ok(Self::Agent),
            "system" => Ok(Self::System),
            _ => Err(MemoryPersistenceError::InvalidEnum {
                kind: "actor_kind",
                value: value.to_string(),
            }),
        }
    }
}

impl FromStr for ActorKind {
    type Err = MemoryPersistenceError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

/// A typed reference to an actor interface row.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ActorRef {
    /// Actor kind discriminator.
    pub actor_kind: ActorKind,
    /// Actor interface id.
    pub actor_id: ActorId,
}

impl ActorRef {
    /// Construct a typed actor reference.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError::EmptyObjectId`] when the id is empty
    /// or only whitespace.
    pub fn new(
        actor_kind: ActorKind,
        actor_id: impl Into<String>,
    ) -> Result<Self, MemoryPersistenceError> {
        Ok(Self {
            actor_kind,
            actor_id: ActorId::new(actor_id)?,
        })
    }

    /// Reference a human actor row.
    #[must_use]
    pub fn human(actor_id: impl Into<String>) -> Self {
        Self::new(ActorKind::Human, actor_id).expect("human actor id must not be empty")
    }

    /// Reference an agent actor row.
    #[must_use]
    pub fn agent(actor_id: impl Into<String>) -> Self {
        Self::new(ActorKind::Agent, actor_id).expect("agent actor id must not be empty")
    }

    /// Reference a system actor row.
    #[must_use]
    pub fn system(actor_id: impl Into<String>) -> Self {
        Self::new(ActorKind::System, actor_id).expect("system actor id must not be empty")
    }
}

impl fmt::Display for ActorRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.actor_id.as_str())
    }
}

/// A typed reference to a concrete object row.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ObjectRef {
    /// Concrete object type.
    pub object_type: ObjectType,
    /// Concrete object id.
    pub object_id: ObjectId,
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
            object_id: ObjectId::new(object_id)?,
        })
    }

    /// Reference a human row.
    #[must_use]
    pub fn human(object_id: impl Into<String>) -> Self {
        Self::new(ObjectType::Human, object_id).expect("human object id must not be empty")
    }

    /// Reference an agent row.
    #[must_use]
    pub fn agent(object_id: impl Into<String>) -> Self {
        Self::new(ObjectType::Agent, object_id).expect("agent object id must not be empty")
    }

    /// Reference a conversation item row.
    #[must_use]
    pub fn conversation_item(object_id: impl Into<String>) -> Self {
        Self::new(ObjectType::ConversationItem, object_id)
            .expect("conversation item object id must not be empty")
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

pub(super) async fn validate_object_ref_for_pool(
    pool: &sqlx::PgPool,
    object_ref: &ObjectRef,
) -> Result<(), MemoryPersistenceError> {
    let sql = format!(
        "SELECT 1 FROM {} WHERE {} = $1 LIMIT 1",
        object_ref.object_type.table_name(),
        object_ref.object_type.id_column()
    );
    let exists = sqlx::query_scalar::<_, i32>(&sql)
        .bind(object_ref.object_id.as_str())
        .fetch_optional(pool)
        .await
        .map_err(MemoryPersistenceError::Database)?
        .is_some();
    if exists {
        Ok(())
    } else {
        Err(MemoryPersistenceError::ObjectRefNotFound {
            object_type: object_ref.object_type.as_str().to_string(),
            object_id: object_ref.object_id.to_string(),
        })
    }
}

#[allow(dead_code)]
pub(crate) async fn validate_actor_ref_for_pool(
    pool: &sqlx::PgPool,
    actor_ref: &ActorRef,
) -> Result<(), MemoryPersistenceError> {
    let exists = sqlx::query_scalar::<_, i32>(
        "SELECT 1 FROM actors WHERE actor_id = $1 AND actor_kind = $2 LIMIT 1",
    )
    .bind(actor_ref.actor_id.as_str())
    .bind(actor_ref.actor_kind.as_str())
    .fetch_optional(pool)
    .await
    .map_err(MemoryPersistenceError::Database)?
    .is_some();
    if exists {
        Ok(())
    } else {
        Err(MemoryPersistenceError::ObjectRefNotFound {
            object_type: actor_ref.actor_kind.as_str().to_string(),
            object_id: actor_ref.actor_id.to_string(),
        })
    }
}
