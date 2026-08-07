// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) import ApolloAPI

/// Closed server-authorized task action.
nonisolated public enum ValidTaskAction: String, EnumType {
  /// Edit Inbox capture fields.
  case edit = "EDIT"
  /// Authorize dispatch.
  case queue = "QUEUE"
  /// Add future execution.
  case schedule = "SCHEDULE"
  /// Replace one-time future execution.
  case reschedule = "RESCHEDULE"
  /// Remove one-time future execution.
  case unschedule = "UNSCHEDULE"
  /// Start scheduled work immediately.
  case runNow = "RUN_NOW"
  /// Answer a gate.
  case answer = "ANSWER"
  /// Retry a recovery gate.
  case retry = "RETRY"
  /// Cancel work.
  case cancel = "CANCEL"
  /// Reopen terminal history.
  case reopen = "REOPEN"
}
