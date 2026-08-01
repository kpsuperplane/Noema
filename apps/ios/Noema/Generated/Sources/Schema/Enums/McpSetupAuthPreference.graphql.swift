// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) import ApolloAPI

/// Authentication behavior for an otherwise successful anonymous setup.
nonisolated public enum McpSetupAuthPreference: String, EnumType {
  /// Offer browser authentication before persisting the server.
  case promptIfAvailable = "PROMPT_IF_AVAILABLE"
  /// Persist only the anonymously visible tools.
  case useAnonymous = "USE_ANONYMOUS"
}
