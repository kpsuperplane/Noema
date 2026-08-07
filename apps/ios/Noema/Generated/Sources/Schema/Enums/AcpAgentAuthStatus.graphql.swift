// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) import ApolloAPI

/// Agent-managed ACP authentication state.
nonisolated public enum AcpAgentAuthStatus: String, EnumType {
  case unknown = "UNKNOWN"
  case none = "NONE"
  case required = "REQUIRED"
  case authenticated = "AUTHENTICATED"
  case failed = "FAILED"
}
