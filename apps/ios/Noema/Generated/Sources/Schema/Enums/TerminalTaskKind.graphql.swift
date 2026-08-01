// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) import ApolloAPI

/// Terminal history filter.
nonisolated public enum TerminalTaskKind: String, EnumType {
  /// Completed tasks only.
  case completed = "COMPLETED"
  /// Cancelled tasks only.
  case cancelled = "CANCELLED"
  /// Both terminal kinds.
  case all = "ALL"
}
