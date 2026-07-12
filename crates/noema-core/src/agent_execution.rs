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
    /// A background task executor producing a task submission.
    TaskExecutor,
    /// A background reviewer validating a task submission.
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
    /// A terminal executor contract tool such as `task.submit_result`.
    ExecutorTerminal,
    /// A terminal reviewer contract tool such as `task.submit_review`.
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
    allow_unlisted_primary_tools: bool,
}

impl ToolPolicy {
    /// Build an empty policy for a role.
    #[must_use]
    pub fn for_role(role: ExecutionRole) -> Self {
        Self {
            role,
            allowed_tool_names: BTreeSet::new(),
            // Preserve the foreground gateway's existing behavior until the
            // foreground path opts into a model-derived strict allowlist.
            allow_unlisted_primary_tools: role == ExecutionRole::PrimaryConversation,
        }
    }

    /// Return the execution role represented by this policy.
    #[must_use]
    pub const fn role(&self) -> ExecutionRole {
        self.role
    }

    /// Add one exact model-visible tool name to the dispatch allowlist.
    pub fn allow_tool_name(&mut self, name: impl Into<String>) {
        let name = name.into();
        if !name.trim().is_empty() {
            self.allowed_tool_names.insert(name);
        }
    }

    /// Add several exact model-visible tool names to the dispatch allowlist.
    pub fn allow_tool_names<I, S>(&mut self, names: I)
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for name in names {
            self.allow_tool_name(name);
        }
    }

    /// Return whether a model-emitted tool name may be dispatched.
    #[must_use]
    pub fn allows_tool(&self, name: &str) -> bool {
        self.allowed_tool_names.contains(name)
            || (self.allow_unlisted_primary_tools
                && self.role == ExecutionRole::PrimaryConversation)
    }

    /// Record a declared tool when its builder has classified its access.
    ///
    /// Returns `true` when the role may expose the tool.  Read-only MCP tools
    /// should use `ReadOnly` only after the MCP calibration/eligibility layer
    /// has verified that they cannot write or export.
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
            ExecutionRole::TaskExecutor => matches!(
                class,
                ToolAccessClass::ReadOnly
                    | ToolAccessClass::TaskOwnedWrite
                    | ToolAccessClass::ExecutorTerminal
            ),
            ExecutionRole::TaskReviewer => matches!(
                class,
                ToolAccessClass::ReadOnly | ToolAccessClass::ReviewerTerminal
            ),
        }
    }

    /// Make a strict copy suitable for dispatch from the names advertised by
    /// a model-tool builder.  Primary executions retain the existing
    /// permissive behavior unless callers explicitly construct a strict
    /// policy; task roles are always strict.
    #[must_use]
    pub fn strict_for_dispatch(&self) -> Self {
        let mut strict = self.clone();
        strict.allow_unlisted_primary_tools = false;
        strict
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
    fn executor_and_reviewer_have_disjoint_terminal_contracts() {
        let mut executor = ToolPolicy::for_role(ExecutionRole::TaskExecutor);
        let mut reviewer = ToolPolicy::for_role(ExecutionRole::TaskReviewer);

        assert!(executor.declare_tool("web.search", ToolAccessClass::ReadOnly));
        assert!(executor.declare_tool("task.submit_result", ToolAccessClass::ExecutorTerminal));
        assert!(!executor.declare_tool("task.submit_review", ToolAccessClass::ReviewerTerminal));
        assert!(reviewer.declare_tool("task.read_artifact", ToolAccessClass::ReadOnly));
        assert!(!reviewer.declare_tool("task.submit_result", ToolAccessClass::ExecutorTerminal));
        assert!(!reviewer.declare_tool(
            "artifact.create_local_file",
            ToolAccessClass::TaskOwnedWrite
        ));
    }

    #[test]
    fn background_dispatch_rejects_hidden_names() {
        let mut policy = ToolPolicy::for_role(ExecutionRole::TaskReviewer);
        policy.declare_tool("web.fetch", ToolAccessClass::ReadOnly);

        assert!(policy.allows_tool("web.fetch"));
        assert!(!policy.allows_tool("mcp.hidden.write"));
        assert!(!policy.allows_tool("task.delegate"));
    }

    #[test]
    fn primary_compatibility_can_be_made_strict() {
        let mut policy = ToolPolicy::for_role(ExecutionRole::PrimaryConversation);
        assert!(policy.allows_tool("legacy.existing.tool"));

        let strict = policy.strict_for_dispatch();
        assert!(!strict.allows_tool("legacy.existing.tool"));
        policy.declare_tool("search_memory", ToolAccessClass::ReadOnly);
        assert!(policy.allows_tool("search_memory"));
    }
}
