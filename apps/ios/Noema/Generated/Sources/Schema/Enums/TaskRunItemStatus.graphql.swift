// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) import ApolloAPI

/// Closed durable task-run transcript item status.
nonisolated public enum TaskRunItemStatus: String, EnumType {
  case pending = "PENDING"
  case running = "RUNNING"
  case completed = "COMPLETED"
  case failed = "FAILED"
  case cancelled = "CANCELLED"
  case skipped = "SKIPPED"
}
