// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) import ApolloAPI

/// Structured approval decision for an Approval gate.
nonisolated public enum ApprovalDecision: String, EnumType {
  /// Explicitly approve the gated action.
  case approved = "APPROVED"
  /// Explicitly decline the gated action.
  case declined = "DECLINED"
}
