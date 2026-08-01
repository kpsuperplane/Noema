// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) import ApolloAPI

/// Onboarding step status exposed through GraphQL.
nonisolated public enum OnboardingStepStatus: String, EnumType {
  /// Step is complete.
  case complete = "COMPLETE"
  /// Step blocks the user from continuing.
  case blocked = "BLOCKED"
}
