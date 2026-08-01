// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) import ApolloAPI

/// Provider auth attempt status.
nonisolated public enum ProviderAuthAttemptStatus: String, EnumType {
  /// Attempt is starting.
  case starting = "STARTING"
  /// Waiting for the user.
  case waitingForUser = "WAITING_FOR_USER"
  /// Attempt completed.
  case completed = "COMPLETED"
  /// Attempt failed.
  case failed = "FAILED"
  /// Attempt expired.
  case expired = "EXPIRED"
  /// Attempt was cancelled.
  case cancelled = "CANCELLED"
}
