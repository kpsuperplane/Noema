// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) import ApolloAPI

/// Human gate lifecycle.
nonisolated public enum TaskGateState: String, EnumType {
  case open = "OPEN"
  case resolved = "RESOLVED"
  case superseded = "SUPERSEDED"
}
