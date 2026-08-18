string_enum! {
/// Current Reviewer decision for a Task.
pub enum TaskReviewVerdict, "review.decision" {
    /// The current work is complete.
    Approve => "approve",
    /// The Executor must address current feedback.
    RequestChanges => "request_changes",
    /// A structured human gate is required.
    NeedsHuman => "needs_human",
}
}
