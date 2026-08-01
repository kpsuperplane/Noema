// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) import ApolloAPI

/// Durable MCP sign-in interruption state.
nonisolated public enum McpAuthenticationRequestState: String, EnumType {
  case awaitingUser = "AWAITING_USER"
  case authorizing = "AUTHORIZING"
  case resuming = "RESUMING"
  case completed = "COMPLETED"
  case cancelled = "CANCELLED"
  case superseded = "SUPERSEDED"
}
