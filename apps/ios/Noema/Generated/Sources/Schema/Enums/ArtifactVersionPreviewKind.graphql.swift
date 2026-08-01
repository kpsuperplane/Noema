// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) import ApolloAPI

/// Preview renderer selected for an artifact version detail panel.
nonisolated public enum ArtifactVersionPreviewKind: String, EnumType {
  /// Local Markdown bytes are available as UTF-8 text.
  case markdown = "MARKDOWN"
  /// Local plain-text bytes are available as UTF-8 text.
  case plainText = "PLAIN_TEXT"
  /// The version exists but this first slice cannot render it inline.
  case unsupported = "UNSUPPORTED"
  /// The version points at an external URL.
  case external = "EXTERNAL"
}
