// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Input for sending a conversation turn.
nonisolated public struct SendConversationTurnInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    conversationId: String,
    input: String,
    clientMessageId: GraphQLNullable<String> = nil,
    clientTimeZone: GraphQLNullable<String> = nil
  ) {
    __data = InputDict([
      "conversationId": conversationId,
      "input": input,
      "clientMessageId": clientMessageId,
      "clientTimeZone": clientTimeZone
    ])
  }

  /// Durable Noema conversation id.
  public var conversationId: String {
    get { __data["conversationId"] }
    set { __data["conversationId"] = newValue }
  }

  /// User input.
  public var input: String {
    get { __data["input"] }
    set { __data["input"] = newValue }
  }

  /// Frontend-generated id for optimistic UI correlation.
  public var clientMessageId: GraphQLNullable<String> {
    get { __data["clientMessageId"] }
    set { __data["clientMessageId"] = newValue }
  }

  /// Validated device IANA timezone for time-sensitive reasoning and scheduling.
  public var clientTimeZone: GraphQLNullable<String> {
    get { __data["clientTimeZone"] }
    set { __data["clientTimeZone"] = newValue }
  }
}
