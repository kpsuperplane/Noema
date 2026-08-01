// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) import ApolloAPI

/// Immutable reviewer disposition.
nonisolated public enum TaskReviewVerdict: String, EnumType {
  case approve = "APPROVE"
  case requestChanges = "REQUEST_CHANGES"
  case needsHuman = "NEEDS_HUMAN"
}
