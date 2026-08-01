// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct SendConversationTurnMutation: GraphQLMutation {
  public static let operationName: String = "SendConversationTurn"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation SendConversationTurn($input: SendConversationTurnInput!) { sendConversationTurn(input: $input) { __typename conversationId clientMessageId } }"#
    ))

  public var input: SendConversationTurnInput

  public init(input: SendConversationTurnInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("sendConversationTurn", SendConversationTurn.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      SendConversationTurnMutation.Data.self
    ] }

    /// Send a conversation turn.
    public var sendConversationTurn: SendConversationTurn { __data["sendConversationTurn"] }

    /// SendConversationTurn
    ///
    /// Parent Type: `TurnAccepted`
    nonisolated public struct SendConversationTurn: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TurnAccepted }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("conversationId", String.self),
        .field("clientMessageId", String?.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SendConversationTurnMutation.Data.SendConversationTurn.self
      ] }

      /// Durable Noema conversation id.
      public var conversationId: String { __data["conversationId"] }
      /// Frontend-generated id for optimistic UI correlation.
      public var clientMessageId: String? { __data["clientMessageId"] }
    }
  }
}
