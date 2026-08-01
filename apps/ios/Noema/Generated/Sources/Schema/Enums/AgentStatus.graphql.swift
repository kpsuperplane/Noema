// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) import ApolloAPI

/// Agent status exposed through GraphQL.
nonisolated public enum AgentStatus: String, EnumType {
  /// No agent work is active.
  case idle = "IDLE"
  /// Input has been accepted.
  case inputReceived = "INPUT_RECEIVED"
  /// Agent is producing or planning.
  case thinking = "THINKING"
  /// Agent is waiting on a tool invocation.
  case toolRunning = "TOOL_RUNNING"
  /// A newer turn is waiting for a previous turn.
  case waitingForPreviousTurnCompletion = "WAITING_FOR_PREVIOUS_TURN_COMPLETION"
  /// Agent is interrupting a previous turn.
  case interrupting = "INTERRUPTING"
  /// Conversation is in an error state.
  case error = "ERROR"
}
