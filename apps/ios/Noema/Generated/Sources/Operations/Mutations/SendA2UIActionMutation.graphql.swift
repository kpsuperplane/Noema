// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct SendA2UIActionMutation: GraphQLMutation {
  public static let operationName: String = "SendA2UIAction"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation SendA2UIAction($input: ProviderInteractionActionInput!) { sendA2UIAction(input: $input) { __typename conversationId clientMessageId } }"#
    ))

  public var input: ProviderInteractionActionInput

  public init(input: ProviderInteractionActionInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("sendA2UIAction", SendA2UIAction.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      SendA2UIActionMutation.Data.self
    ] }

    /// Submit one exact action to a pending A2UI surface.
    public var sendA2UIAction: SendA2UIAction { __data["sendA2UIAction"] }

    /// SendA2UIAction
    ///
    /// Parent Type: `TurnAccepted`
    nonisolated public struct SendA2UIAction: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TurnAccepted }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("conversationId", String.self),
        .field("clientMessageId", String?.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SendA2UIActionMutation.Data.SendA2UIAction.self
      ] }

      /// Durable Noema conversation id.
      public var conversationId: String { __data["conversationId"] }
      /// Frontend-generated id for optimistic UI correlation.
      public var clientMessageId: String? { __data["clientMessageId"] }
    }
  }
}
