use crate::ConversationError;

macro_rules! status_enum {
    (
        $(#[$enum_attr:meta])*
        pub enum $name:ident for $kind:literal {
            $($(#[$variant_attr:meta])* $variant:ident = $wire:literal),+ $(,)?
        }
    ) => {
        $(#[$enum_attr])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum $name {
            $($(#[$variant_attr])* $variant),+
        }

        impl $name {
            /// Return the stable storage string for this value.
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $wire),+ }
            }

            /// Parse a stored value.
            ///
            /// # Errors
            ///
            /// Returns [`ConversationError::InvalidEnum`] for unknown values.
            pub fn parse(value: &str) -> Result<Self, ConversationError> {
                match value {
                    $($wire => Ok(Self::$variant),)+
                    _ => Err(ConversationError::InvalidEnum {
                        kind: $kind,
                        value: value.to_string(),
                    }),
                }
            }

            #[cfg(test)]
            const ALL: &'static [Self] = &[$(Self::$variant),+];
        }
    };
}

status_enum! {
    /// Live agent coordination state for a durable conversation.
    pub enum AgentStatus for "agent_status" {
        /// No agent work is currently active.
        Idle = "idle",
        /// Human or external input has been accepted.
        InputReceived = "input_received",
        /// The agent is producing or planning a response.
        Thinking = "thinking",
        /// The agent is waiting on a tool invocation.
        ToolRunning = "tool_running",
        /// A newer turn is waiting for a prior turn's side effects to settle.
        WaitingForPreviousTurnCompletion = "waiting_for_previous_turn_completion",
        /// The agent is interrupting a previous turn.
        Interrupting = "interrupting",
        /// The conversation is in an error state.
        Error = "error",
    }
}

status_enum! {
    /// Durable lifecycle state for one causal conversation turn.
    pub enum ConversationTurnStatus for "conversation_turn_status" {
        /// Input has been recorded but work has not started.
        InputReceived = "input_received",
        /// The turn is actively running.
        Running = "running",
        /// The turn is waiting for a tool result.
        WaitingForTool = "waiting_for_tool",
        /// The turn was interrupted by newer input.
        Interrupted = "interrupted",
        /// The turn completed successfully.
        Completed = "completed",
        /// The turn failed.
        Failed = "failed",
        /// The turn was cancelled.
        Cancelled = "cancelled",
    }
}

status_enum! {
    /// Semantic kind for a durable conversation stream item.
    pub enum ConversationItemKind for "conversation_item_kind" {
        /// Text authored by a human.
        UserText = "user_text",
        /// Text authored by an assistant.
        AssistantText = "assistant_text",
        /// Non-text activity that should appear in the transcript.
        Activity = "activity",
        /// Structured A2UI card payload.
        A2uiCard = "a2ui_card",
        /// Assistant-authored multiple-choice prompt.
        MultipleChoicePrompt = "multiple_choice_prompt",
        /// Human-authored multiple-choice selection.
        MultipleChoiceSelection = "multiple_choice_selection",
        /// A tool invocation request.
        ToolCall = "tool_call",
        /// A tool invocation result.
        ToolResult = "tool_result",
        /// Provider-encrypted reasoning state used only for stateless provider replay.
        Reasoning = "reasoning",
        /// Hidden keyed application context used for exact model replay.
        ModelContextUpdate = "model_context_update",
        /// A request for human approval.
        ApprovalRequest = "approval_request",
        /// A recorded approval decision.
        ApprovalResult = "approval_result",
        /// A reference to a persisted artifact.
        ArtifactReference = "artifact_reference",
        /// A durable reference to a background task.
        TaskReference = "task_reference",
        /// An error visible in conversation history.
        ErrorNotice = "error_notice",
    }
}

status_enum! {
    /// Execution status for a durable conversation item.
    pub enum ConversationItemStatus for "conversation_item_status" {
        /// The item has been created but work has not started.
        Pending = "pending",
        /// The item represents work in progress.
        Running = "running",
        /// The item completed successfully.
        Completed = "completed",
        /// The item failed.
        Failed = "failed",
        /// The item was cancelled.
        Cancelled = "cancelled",
        /// The item was interrupted.
        Interrupted = "interrupted",
    }
}

status_enum! {
    /// Lifecycle state for a derived conversation context summary.
    pub enum ConversationContextSummaryStatus for "conversation_context_summary_status" {
        /// Summary creation has been requested but is not active.
        Pending = "pending",
        /// Summary is the active checkpoint for its context profile.
        Active = "active",
        /// Summary creation failed.
        Failed = "failed",
        /// Summary was replaced by a newer active checkpoint.
        Superseded = "superseded",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_status_vocabulary_round_trips_every_wire_value_and_rejects_unknowns() {
        assert_vocabulary(
            AgentStatus::ALL,
            AgentStatus::as_str,
            AgentStatus::parse,
            "agent_status",
        );
        assert_vocabulary(
            ConversationTurnStatus::ALL,
            ConversationTurnStatus::as_str,
            ConversationTurnStatus::parse,
            "conversation_turn_status",
        );
        assert_vocabulary(
            ConversationItemKind::ALL,
            ConversationItemKind::as_str,
            ConversationItemKind::parse,
            "conversation_item_kind",
        );
        assert_vocabulary(
            ConversationItemStatus::ALL,
            ConversationItemStatus::as_str,
            ConversationItemStatus::parse,
            "conversation_item_status",
        );
        assert_vocabulary(
            ConversationContextSummaryStatus::ALL,
            ConversationContextSummaryStatus::as_str,
            ConversationContextSummaryStatus::parse,
            "conversation_context_summary_status",
        );
    }

    fn assert_vocabulary<T>(
        values: &[T],
        as_str: impl Fn(T) -> &'static str,
        parse: impl Fn(&str) -> Result<T, ConversationError>,
        kind: &'static str,
    ) where
        T: Copy + std::fmt::Debug + PartialEq,
    {
        for &value in values {
            assert_eq!(parse(as_str(value)), Ok(value));
        }
        assert_eq!(
            parse("unknown").expect_err("unknown value must fail"),
            ConversationError::InvalidEnum {
                kind,
                value: "unknown".to_string(),
            }
        );
    }
}
