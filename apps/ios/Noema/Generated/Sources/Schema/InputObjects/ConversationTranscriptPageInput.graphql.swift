// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Input for reading a visible transcript page.
nonisolated public struct ConversationTranscriptPageInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    conversationId: String,
    cursor: GraphQLNullable<String> = nil,
    limit: GraphQLNullable<Int32> = nil
  ) {
    __data = InputDict([
      "conversationId": conversationId,
      "cursor": cursor,
      "limit": limit
    ])
  }

  /// Durable Noema conversation id.
  public var conversationId: String {
    get { __data["conversationId"] }
    set { __data["conversationId"] = newValue }
  }

  /// Opaque cursor. When omitted, reads the latest page.
  public var cursor: GraphQLNullable<String> {
    get { __data["cursor"] }
    set { __data["cursor"] = newValue }
  }

  /// Page size. Defaults to 80 and must be within 1..=200.
  public var limit: GraphQLNullable<Int32> {
    get { __data["limit"] }
    set { __data["limit"] = newValue }
  }
}
