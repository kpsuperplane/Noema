// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) import ApolloAPI

/// Durable state of one immutable action revision.
nonisolated public enum GovernedActionState: String, EnumType {
  case proposed = "PROPOSED"
  case awaitingApproval = "AWAITING_APPROVAL"
  case executable = "EXECUTABLE"
  case executing = "EXECUTING"
  case awaitingAuthentication = "AWAITING_AUTHENTICATION"
  case succeeded = "SUCCEEDED"
  case failed = "FAILED"
  case outcomeUncertain = "OUTCOME_UNCERTAIN"
  case declined = "DECLINED"
  case superseded = "SUPERSEDED"
  case cancelled = "CANCELLED"
}
