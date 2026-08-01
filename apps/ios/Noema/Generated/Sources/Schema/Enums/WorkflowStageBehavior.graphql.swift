// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) import ApolloAPI

/// Runtime behavior of one workflow stage.
nonisolated public enum WorkflowStageBehavior: String, EnumType {
  /// Captured work not yet authorized to run.
  case intake = "INTAKE"
  /// Authorized work awaiting a worker.
  case dispatch = "DISPATCH"
  /// Planning, execution, or automated review is active.
  case active = "ACTIVE"
  /// A human gate is open.
  case humanGate = "HUMAN_GATE"
  /// Reviewer-approved terminal work.
  case terminalSuccess = "TERMINAL_SUCCESS"
  /// Human-cancelled terminal work.
  case terminalCancelled = "TERMINAL_CANCELLED"
}
