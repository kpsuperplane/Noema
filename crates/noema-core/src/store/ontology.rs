/// Closed graph entity type vocabulary stored in the embedded graph tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntityType {
    /// A human owner or collaborator.
    Human,
    /// An agent principal.
    Agent,
    /// A person who is not necessarily a Noema human.
    Person,
    /// An organization.
    Organization,
    /// A project.
    Project,
    /// A workspace.
    Workspace,
    /// A conversation.
    Conversation,
    /// A document.
    Document,
    /// A tool.
    Tool,
    /// A place.
    Place,
    /// A task.
    Task,
    /// A goal.
    Goal,
    /// A concept.
    Concept,
    /// A fallback entity type.
    Other,
}

impl EntityType {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Human => "human",
            Self::Agent => "agent",
            Self::Person => "person",
            Self::Organization => "organization",
            Self::Project => "project",
            Self::Workspace => "workspace",
            Self::Conversation => "conversation",
            Self::Document => "document",
            Self::Tool => "tool",
            Self::Place => "place",
            Self::Task => "task",
            Self::Goal => "goal",
            Self::Concept => "concept",
            Self::Other => "other",
        }
    }
}

/// Candidate entity to upsert before creating or reinforcing a claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityCandidate {
    /// Stable graph entity id.
    pub entity_id: String,
    /// Closed entity type.
    pub entity_type: EntityType,
    /// Human-readable canonical name.
    pub canonical_name: String,
}

impl EntityCandidate {
    /// Return the local human entity used by the default single-user setup.
    #[must_use]
    pub fn local_human() -> Self {
        Self {
            entity_id: "human:local".to_string(),
            entity_type: EntityType::Human,
            canonical_name: "Local human".to_string(),
        }
    }

    /// Build a concept entity from a stable caller-provided id fragment.
    #[must_use]
    pub fn concept(id_fragment: &str, name: &str) -> Self {
        Self {
            entity_id: format!("concept:{}", super::ids::record_fragment(id_fragment)),
            entity_type: EntityType::Concept,
            canonical_name: name.to_string(),
        }
    }
}

/// Persisted predicate fields needed by the first graph-memory write API.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct PredicateRecord {
    /// Stable predicate id.
    pub predicate_id: String,
    /// Human-readable label.
    pub label: String,
    /// Default sensitivity stored as the schema vocabulary string.
    pub default_sensitivity: String,
    /// Use modes that may retrieve claims for this predicate.
    pub allowed_use_modes: Vec<String>,
}
