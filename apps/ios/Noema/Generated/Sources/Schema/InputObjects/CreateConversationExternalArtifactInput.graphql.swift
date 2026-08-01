// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Input for creating a conversation-owned external URL artifact.
nonisolated public struct CreateConversationExternalArtifactInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    conversationId: String,
    title: String,
    description: GraphQLNullable<String> = nil,
    artifactKind: String,
    externalUrl: String,
    mediaType: GraphQLNullable<String> = nil
  ) {
    __data = InputDict([
      "conversationId": conversationId,
      "title": title,
      "description": description,
      "artifactKind": artifactKind,
      "externalUrl": externalUrl,
      "mediaType": mediaType
    ])
  }

  /// Owning conversation id.
  public var conversationId: String {
    get { __data["conversationId"] }
    set { __data["conversationId"] = newValue }
  }

  /// Human-readable artifact title.
  public var title: String {
    get { __data["title"] }
    set { __data["title"] = newValue }
  }

  /// Optional artifact description.
  public var description: GraphQLNullable<String> {
    get { __data["description"] }
    set { __data["description"] = newValue }
  }

  /// Product-defined artifact kind label.
  public var artifactKind: String {
    get { __data["artifactKind"] }
    set { __data["artifactKind"] = newValue }
  }

  /// Durable external HTTP(S) URL for the initial version.
  public var externalUrl: String {
    get { __data["externalUrl"] }
    set { __data["externalUrl"] = newValue }
  }

  /// Optional media type for the external payload.
  public var mediaType: GraphQLNullable<String> {
    get { __data["mediaType"] }
    set { __data["mediaType"] = newValue }
  }
}
