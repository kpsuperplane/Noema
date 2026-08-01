// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) import ApolloAPI

/// Category for one cursor-bearing local-model event.
nonisolated public enum LocalModelEventKind: String, EnumType {
  /// An installation projection changed.
  case installationUpdated = "INSTALLATION_UPDATED"
  /// Download or copy progress advanced.
  case transferProgress = "TRANSFER_PROGRESS"
  /// The active local model changed.
  case activeModelChanged = "ACTIVE_MODEL_CHANGED"
  /// The supervised runtime state changed.
  case runtimeChanged = "RUNTIME_CHANGED"
  /// The system default model preference changed.
  case defaultPreferenceChanged = "DEFAULT_PREFERENCE_CHANGED"
}
