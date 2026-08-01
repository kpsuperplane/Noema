// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) import ApolloAPI

/// Current llama.cpp process state.
nonisolated public enum LocalModelRuntimeStatus: String, EnumType {
  /// No installed model is selected.
  case inactive = "INACTIVE"
  /// The runtime is starting or loading weights.
  case starting = "STARTING"
  /// The runtime is ready for inference.
  case running = "RUNNING"
  /// The runtime is stopping.
  case stopping = "STOPPING"
  /// The runtime failed and may be retried.
  case failed = "FAILED"
}
