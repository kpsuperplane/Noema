// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) import ApolloAPI

/// Recurring overlap behavior.
nonisolated public enum OverlapPolicy: String, EnumType {
  case skip = "SKIP"
  case queueOne = "QUEUE_ONE"
  case allow = "ALLOW"
}
