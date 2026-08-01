// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) import ApolloAPI

/// Provenance for an installed local model.
nonisolated public enum LocalModelSourceKind: String, EnumType {
  /// Artifact from Noema's bundled curated catalog.
  case catalog = "CATALOG"
  /// User-supplied public Hugging Face GGUF.
  case publicGguf = "PUBLIC_GGUF"
  /// GGUF imported from the local filesystem.
  case localFile = "LOCAL_FILE"
}
