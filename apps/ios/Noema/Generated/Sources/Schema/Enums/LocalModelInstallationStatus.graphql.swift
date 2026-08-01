// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) import ApolloAPI

/// Current local-model installation state.
nonisolated public enum LocalModelInstallationStatus: String, EnumType {
  /// The installation is queued.
  case queued = "QUEUED"
  /// Model bytes are being downloaded or copied.
  case downloading = "DOWNLOADING"
  /// The artifact checksum is being verified.
  case verifying = "VERIFYING"
  /// The model was installed atomically.
  case installed = "INSTALLED"
  /// The operation was cancelled.
  case cancelled = "CANCELLED"
  /// The operation failed and may be retried.
  case failed = "FAILED"
}
