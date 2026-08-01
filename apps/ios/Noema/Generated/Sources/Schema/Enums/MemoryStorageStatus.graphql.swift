// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) import ApolloAPI

/// Memory storage readiness shown by clients.
nonisolated public enum MemoryStorageStatus: String, EnumType {
  /// The local Markdown memory tree is ready.
  case ready = "READY"
  /// The local Markdown memory tree is initializing.
  case initializing = "INITIALIZING"
  /// Markdown memory writes and retrieval are not available yet.
  case unavailable = "UNAVAILABLE"
}
