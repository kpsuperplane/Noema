string_enum! {
/// Bounded execution complexity selected for a Task.
pub enum TaskComplexity, "task_complexity" {
    /// Small, low-risk work.
    Simple => "simple",
    /// Typical multi-step work.
    Medium => "medium",
    /// Large or reasoning-intensive work.
    Difficult => "difficult",
}
}
