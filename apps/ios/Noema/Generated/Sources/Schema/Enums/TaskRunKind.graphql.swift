// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) import ApolloAPI

/// Role of one bounded task run.
nonisolated public enum TaskRunKind: String, EnumType {
  case planner = "PLANNER"
  case executor = "EXECUTOR"
  case reviewer = "REVIEWER"
}
