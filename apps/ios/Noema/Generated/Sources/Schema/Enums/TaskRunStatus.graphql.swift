// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) import ApolloAPI

/// Run-local queue and lease status.
nonisolated public enum TaskRunStatus: String, EnumType {
  case queued = "QUEUED"
  case leased = "LEASED"
  case running = "RUNNING"
  case completed = "COMPLETED"
  case waitingForApproval = "WAITING_FOR_APPROVAL"
  case interrupted = "INTERRUPTED"
  case failed = "FAILED"
  case cancelled = "CANCELLED"
}
