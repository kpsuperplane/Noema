string_enum! {
/// Bounded complexity selected for a delegated task or execution contract.
pub enum TaskComplexity, "task_complexity" {
    /// Small, low-risk work.
    Simple => "simple",
    /// Typical multi-step work.
    Medium => "medium",
    /// Large or reasoning-intensive work.
    Difficult => "difficult",
}
}
