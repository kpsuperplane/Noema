// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) import ApolloAPI

/// Complexity selected for a new execution contract.
nonisolated public enum TaskComplexity: String, EnumType {
  /// Small, low-risk work.
  case simple = "SIMPLE"
  /// Typical multi-step work.
  case medium = "MEDIUM"
  /// Large or reasoning-intensive work.
  case difficult = "DIFFICULT"
}
