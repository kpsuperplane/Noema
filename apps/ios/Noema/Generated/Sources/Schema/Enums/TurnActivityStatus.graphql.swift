// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) import ApolloAPI

/// Turn activity status exposed through GraphQL.
nonisolated public enum TurnActivityStatus: String, EnumType {
  /// Activity started.
  case started = "STARTED"
  /// Activity completed.
  case completed = "COMPLETED"
  /// Activity failed.
  case failed = "FAILED"
}
