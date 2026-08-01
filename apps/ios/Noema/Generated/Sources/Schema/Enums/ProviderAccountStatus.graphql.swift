// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) import ApolloAPI

/// Provider account status exposed through GraphQL.
nonisolated public enum ProviderAccountStatus: String, EnumType {
  /// Status has not been checked.
  case unknown = "UNKNOWN"
  /// Status is being checked.
  case checking = "CHECKING"
  /// Account is authenticated.
  case authenticated = "AUTHENTICATED"
  /// Account is unauthenticated.
  case unauthenticated = "UNAUTHENTICATED"
  /// Provider is unavailable.
  case unavailable = "UNAVAILABLE"
}
