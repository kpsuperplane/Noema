use crate::MemoryPersistenceError;

fn invalid_enum<T>(kind: &'static str, value: &str) -> Result<T, MemoryPersistenceError> {
    Err(MemoryPersistenceError::InvalidEnum {
        kind,
        value: value.to_string(),
    })
}

/// Live agent coordination state for a durable conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentStatus {
    /// No agent work is currently active.
    Idle,
    /// Human or external input has been accepted.
    InputReceived,
    /// The agent is producing or planning a response.
    Thinking,
    /// The agent is waiting on a tool invocation.
    ToolRunning,
    /// A newer turn is waiting for a prior turn's side effects to settle.
    WaitingForPreviousTurnCompletion,
    /// The agent is interrupting a previous turn.
    Interrupting,
    /// The conversation is in an error state.
    Error,
}

impl AgentStatus {
    /// Return the stable storage string for this status.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::InputReceived => "input_received",
            Self::Thinking => "thinking",
            Self::ToolRunning => "tool_running",
            Self::WaitingForPreviousTurnCompletion => "waiting_for_previous_turn_completion",
            Self::Interrupting => "interrupting",
            Self::Error => "error",
        }
    }

    /// Parse a stored status string.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError::InvalidEnum`] for unknown values.
    pub fn parse(value: &str) -> Result<Self, MemoryPersistenceError> {
        match value {
            "idle" => Ok(Self::Idle),
            "input_received" => Ok(Self::InputReceived),
            "thinking" => Ok(Self::Thinking),
            "tool_running" => Ok(Self::ToolRunning),
            "waiting_for_previous_turn_completion" => Ok(Self::WaitingForPreviousTurnCompletion),
            "interrupting" => Ok(Self::Interrupting),
            "error" => Ok(Self::Error),
            _ => invalid_enum("agent_status", value),
        }
    }
}

/// Durable lifecycle state for one causal conversation turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversationTurnStatus {
    /// Input has been recorded but work has not started.
    InputReceived,
    /// The turn is actively running.
    Running,
    /// The turn is waiting for a tool result.
    WaitingForTool,
    /// The turn was interrupted by newer input.
    Interrupted,
    /// The turn completed successfully.
    Completed,
    /// The turn failed.
    Failed,
    /// The turn was cancelled.
    Cancelled,
}

impl ConversationTurnStatus {
    /// Return the stable storage string for this status.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InputReceived => "input_received",
            Self::Running => "running",
            Self::WaitingForTool => "waiting_for_tool",
            Self::Interrupted => "interrupted",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    /// Parse a stored turn status string.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError::InvalidEnum`] for unknown values.
    pub fn parse(value: &str) -> Result<Self, MemoryPersistenceError> {
        match value {
            "input_received" => Ok(Self::InputReceived),
            "running" => Ok(Self::Running),
            "waiting_for_tool" => Ok(Self::WaitingForTool),
            "interrupted" => Ok(Self::Interrupted),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            _ => invalid_enum("conversation_turn_status", value),
        }
    }
}

/// Semantic kind for a durable conversation stream item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversationItemKind {
    /// Text authored by a human.
    UserText,
    /// Text authored by an assistant.
    AssistantText,
    /// Non-text activity that should appear in the transcript.
    Activity,
    /// Structured A2UI card payload.
    A2uiCard,
    /// A tool invocation request.
    ToolCall,
    /// A tool invocation result.
    ToolResult,
    /// Provider-encrypted reasoning state used only for stateless provider replay.
    Reasoning,
    /// A request for human approval.
    ApprovalRequest,
    /// A recorded approval decision.
    ApprovalResult,
    /// An error visible in conversation history.
    ErrorNotice,
}

impl ConversationItemKind {
    /// Return the stable storage string for this item kind.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UserText => "user_text",
            Self::AssistantText => "assistant_text",
            Self::Activity => "activity",
            Self::A2uiCard => "a2ui_card",
            Self::ToolCall => "tool_call",
            Self::ToolResult => "tool_result",
            Self::Reasoning => "reasoning",
            Self::ApprovalRequest => "approval_request",
            Self::ApprovalResult => "approval_result",
            Self::ErrorNotice => "error_notice",
        }
    }

    /// Parse a stored item kind string.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError::InvalidEnum`] for unknown values.
    pub fn parse(value: &str) -> Result<Self, MemoryPersistenceError> {
        match value {
            "user_text" => Ok(Self::UserText),
            "assistant_text" => Ok(Self::AssistantText),
            "activity" => Ok(Self::Activity),
            "a2ui_card" => Ok(Self::A2uiCard),
            "tool_call" => Ok(Self::ToolCall),
            "tool_result" => Ok(Self::ToolResult),
            "reasoning" => Ok(Self::Reasoning),
            "approval_request" => Ok(Self::ApprovalRequest),
            "approval_result" => Ok(Self::ApprovalResult),
            "error_notice" => Ok(Self::ErrorNotice),
            _ => invalid_enum("conversation_item_kind", value),
        }
    }
}

/// Execution status for a durable conversation item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversationItemStatus {
    /// The item has been created but work has not started.
    Pending,
    /// The item represents work in progress.
    Running,
    /// The item completed successfully.
    Completed,
    /// The item failed.
    Failed,
    /// The item was cancelled.
    Cancelled,
    /// The item was interrupted.
    Interrupted,
}

impl ConversationItemStatus {
    /// Return the stable storage string for this item status.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Interrupted => "interrupted",
        }
    }

    /// Parse a stored item status string.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError::InvalidEnum`] for unknown values.
    pub fn parse(value: &str) -> Result<Self, MemoryPersistenceError> {
        match value {
            "pending" => Ok(Self::Pending),
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            "interrupted" => Ok(Self::Interrupted),
            _ => invalid_enum("conversation_item_status", value),
        }
    }
}

/// Lifecycle state for a derived conversation context summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversationContextSummaryStatus {
    /// Summary creation has been requested but is not active.
    Pending,
    /// Summary is the active checkpoint for its context profile.
    Active,
    /// Summary creation failed.
    Failed,
    /// Summary was replaced by a newer active checkpoint.
    Superseded,
}

impl ConversationContextSummaryStatus {
    /// Return the stable storage string for this status.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Active => "active",
            Self::Failed => "failed",
            Self::Superseded => "superseded",
        }
    }

    /// Parse a stored context summary status string.
    ///
    /// # Errors
    ///
    /// Returns [`MemoryPersistenceError::InvalidEnum`] for unknown values.
    pub fn parse(value: &str) -> Result<Self, MemoryPersistenceError> {
        match value {
            "pending" => Ok(Self::Pending),
            "active" => Ok(Self::Active),
            "failed" => Ok(Self::Failed),
            "superseded" => Ok(Self::Superseded),
            _ => invalid_enum("conversation_context_summary_status", value),
        }
    }
}
