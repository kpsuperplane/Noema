// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) import ApolloAPI

/// Why a task currently needs the owner.
nonisolated public enum TaskAttentionKind: String, EnumType {
  /// Missing or ambiguous information is required.
  case clarificationRequired = "CLARIFICATION_REQUIRED"
  /// A governed human decision is required.
  case approvalRequired = "APPROVAL_REQUIRED"
  /// Automated work requires a recovery choice.
  case recoveryRequired = "RECOVERY_REQUIRED"
}
