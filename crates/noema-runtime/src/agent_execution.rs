//! Shared execution roles and tool-dispatch policy.
//!
//! This module intentionally contains no task persistence types.  It provides
//! the small policy boundary that both foreground conversations and future
//! background runs can use while they share the provider/tool continuation
//! engine.

use std::collections::BTreeSet;

/// Product role under which a model execution is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExecutionRole {
    /// The primary agent responding in a human conversation.
    PrimaryConversation,
    /// A background Task Executor.
    TaskPlanner,
    /// A background ACP Task Executor.
    TaskExecutor,
    /// A background Reviewer validating current Task files.
    TaskReviewer,
}

/// Semantic class assigned by a tool builder before it enters a role policy.
///
/// The class is explicit product state supplied by the builder; it is not
/// inferred from model text or tool-call arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolAccessClass {
    /// A read-only retrieval or inspection operation.
    ReadOnly,
    /// A write that is scoped to the currently executing task.
    TaskOwnedWrite,
    /// A write owned by a foreground conversation.
    ConversationWrite,
    /// A tool backed by an external destination.
    ExternalTool,
    /// The terminal Executor tool, such as `task.finish_execution`.
    ExecutorTerminal,
    /// The terminal Reviewer tool, such as `task.finish_review`.
    ReviewerTerminal,
    /// An internal identity/control operation.
    Internal,
}

/// Dispatch policy for one execution.
///
/// The allowlist is populated from the exact tool set advertised to the model
/// (plus explicit role-contract tools).  Background roles reject unlisted
/// names at dispatch even when a provider emits a forged or hidden call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolPolicy {
    role: ExecutionRole,
    allowed_tool_names: BTreeSet<String>,
}

impl ToolPolicy {
    /// Build an empty policy for a role.
    #[must_use]
    pub fn for_role(role: ExecutionRole) -> Self {
        Self {
            role,
            allowed_tool_names: BTreeSet::new(),
        }
    }

    /// Return the execution role represented by this policy.
    #[must_use]
    pub const fn role(&self) -> ExecutionRole {
        self.role
    }

    /// Add one exact model-visible tool name to the dispatch allowlist.
    pub(crate) fn allow_tool_name(&mut self, name: impl Into<String>) {
        let name = name.into();
        if !name.trim().is_empty() {
            self.allowed_tool_names.insert(name);
        }
    }

    /// Return whether a model-emitted tool name may be dispatched.
    #[must_use]
    pub fn allows_tool(&self, name: &str) -> bool {
        self.allowed_tool_names.contains(name)
    }

    /// Record a declared tool when its builder has classified its access.
    ///
    /// Returns `true` when the role may expose the tool.  Read-only MCP tools
    /// should use `ReadOnly` only after the MCP tool-policy layer has verified
    /// that they cannot mutate the environment.
    pub fn declare_tool(&mut self, name: impl Into<String>, class: ToolAccessClass) -> bool {
        let name = name.into();
        if name.trim().is_empty() || !self.allows_class(class) {
            return false;
        }
        self.allowed_tool_names.insert(name);
        true
    }

    /// Return whether a role can expose a classified tool.
    #[must_use]
    pub const fn allows_class(&self, class: ToolAccessClass) -> bool {
        match self.role {
            ExecutionRole::PrimaryConversation => true,
            ExecutionRole::TaskPlanner => matches!(
                class,
                ToolAccessClass::ReadOnly
                    | ToolAccessClass::TaskOwnedWrite
                    | ToolAccessClass::ExecutorTerminal
            ),
            ExecutionRole::TaskExecutor => matches!(
                class,
                ToolAccessClass::ReadOnly
                    | ToolAccessClass::TaskOwnedWrite
                    | ToolAccessClass::ExternalTool
                    | ToolAccessClass::ExecutorTerminal
            ),
            ExecutionRole::TaskReviewer => matches!(
                class,
                ToolAccessClass::ReadOnly | ToolAccessClass::ReviewerTerminal
            ),
        }
    }

    /// Intersect exact allowed names without permitting a later catalog to
    /// grow authority retained from an earlier provider request.
    #[must_use]
    pub fn intersect_allowed_names(&self, later: &Self) -> Self {
        if self.role != later.role {
            return Self::for_role(self.role);
        }
        Self {
            role: self.role,
            allowed_tool_names: self
                .allowed_tool_names
                .intersection(&later.allowed_tool_names)
                .cloned()
                .collect(),
        }
    }
}

impl Default for ToolPolicy {
    fn default() -> Self {
        Self::for_role(ExecutionRole::PrimaryConversation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_compatibility_can_be_made_strict() {
        let mut policy = ToolPolicy::for_role(ExecutionRole::PrimaryConversation);
        assert!(!policy.allows_tool("legacy.existing.tool"));
        policy.declare_tool("search_memory", ToolAccessClass::ReadOnly);
        assert!(policy.allows_tool("search_memory"));
        assert!(!policy.allows_tool("legacy.existing.tool"));
    }
}
