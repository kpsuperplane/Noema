// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) import ApolloAPI

/// Review route that originated a durable action.
nonisolated public enum ExecutionReviewRoute: String, EnumType {
  case humanReview = "HUMAN_REVIEW"
  case llmReview = "LLM_REVIEW"
}
