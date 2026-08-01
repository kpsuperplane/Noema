// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) import ApolloAPI

/// Closed reason for a Recovery gate.
nonisolated public enum TaskRecoveryReason: String, EnumType {
  case infrastructureRetriesExhausted = "INFRASTRUCTURE_RETRIES_EXHAUSTED"
  case reviewRoundsExhausted = "REVIEW_ROUNDS_EXHAUSTED"
  case unsafeEffectUncertain = "UNSAFE_EFFECT_UNCERTAIN"
  case configurationUnavailable = "CONFIGURATION_UNAVAILABLE"
  case invariantFault = "INVARIANT_FAULT"
}
