/// Coarse memory type used by graph memory records and retrieval policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemoryType {
    /// Durable fact.
    Fact,
    /// User or scope preference.
    Preference,
    /// Person-related memory.
    Person,
    /// Organization-related memory.
    Organization,
    /// Project memory.
    Project,
    /// Place memory.
    Place,
    /// Routine or recurring behavior.
    Routine,
    /// Goal memory.
    Goal,
    /// Open loop or follow-up.
    OpenLoop,
    /// Procedure or workflow.
    Procedure,
    /// Constraint.
    Constraint,
    /// Trigger memory.
    Trigger,
    /// Decision.
    Decision,
    /// Agent skill memory.
    Skill,
    /// Policy memory.
    Policy,
    /// General note.
    Note,
    /// Other memory type.
    Other,
}

impl MemoryType {
    /// Storage representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fact => "fact",
            Self::Preference => "preference",
            Self::Person => "person",
            Self::Organization => "organization",
            Self::Project => "project",
            Self::Place => "place",
            Self::Routine => "routine",
            Self::Goal => "goal",
            Self::OpenLoop => "open_loop",
            Self::Procedure => "procedure",
            Self::Constraint => "constraint",
            Self::Trigger => "trigger",
            Self::Decision => "decision",
            Self::Skill => "skill",
            Self::Policy => "policy",
            Self::Note => "note",
            Self::Other => "other",
        }
    }
}
