// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) import ApolloAPI

/// Scope of a task connection.
nonisolated public enum TaskScope: String, EnumType {
  /// Nonterminal workflow stages.
  case active = "ACTIVE"
  /// Done and cancelled history.
  case terminal = "TERMINAL"
  /// Both active and terminal tasks.
  case all = "ALL"
}
