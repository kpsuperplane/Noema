// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) import ApolloAPI

/// Artifact storage kind exposed through GraphQL.
nonisolated public enum ArtifactStorageKind: String, EnumType {
  /// Artifact bytes live in the local Noema filesystem.
  case localFile = "LOCAL_FILE"
  /// Artifact content is referenced by an external durable URL.
  case externalUrl = "EXTERNAL_URL"
}
