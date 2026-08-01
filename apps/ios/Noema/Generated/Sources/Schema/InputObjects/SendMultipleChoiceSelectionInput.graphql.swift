// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Input for sending a multiple-choice selection.
nonisolated public struct SendMultipleChoiceSelectionInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    conversationId: String,
    promptItemId: String,
    selectedOptionIds: [String],
    clientMessageId: GraphQLNullable<String> = nil
  ) {
    __data = InputDict([
      "conversationId": conversationId,
      "promptItemId": promptItemId,
      "selectedOptionIds": selectedOptionIds,
      "clientMessageId": clientMessageId
    ])
  }

  /// Durable Noema conversation id.
  public var conversationId: String {
    get { __data["conversationId"] }
    set { __data["conversationId"] = newValue }
  }

  /// Durable multiple-choice prompt item id.
  public var promptItemId: String {
    get { __data["promptItemId"] }
    set { __data["promptItemId"] = newValue }
  }

  /// Selected prompt option ids.
  public var selectedOptionIds: [String] {
    get { __data["selectedOptionIds"] }
    set { __data["selectedOptionIds"] = newValue }
  }

  /// Frontend-generated id for optimistic UI correlation.
  public var clientMessageId: GraphQLNullable<String> {
    get { __data["clientMessageId"] }
    set { __data["clientMessageId"] = newValue }
  }
}
