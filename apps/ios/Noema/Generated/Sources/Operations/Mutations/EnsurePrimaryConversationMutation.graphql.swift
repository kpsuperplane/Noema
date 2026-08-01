// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct EnsurePrimaryConversationMutation: GraphQLMutation {
  public static let operationName: String = "EnsurePrimaryConversation"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation EnsurePrimaryConversation { ensurePrimaryConversation { __typename conversationId provider } }"#
    ))

  public init() {}

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("ensurePrimaryConversation", EnsurePrimaryConversation.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      EnsurePrimaryConversationMutation.Data.self
    ] }

    /// Ensure the primary conversation exists.
    public var ensurePrimaryConversation: EnsurePrimaryConversation { __data["ensurePrimaryConversation"] }

    /// EnsurePrimaryConversation
    ///
    /// Parent Type: `PrimaryConversation`
    nonisolated public struct EnsurePrimaryConversation: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.PrimaryConversation }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("conversationId", String.self),
        .field("provider", String.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        EnsurePrimaryConversationMutation.Data.EnsurePrimaryConversation.self
      ] }

      /// Durable Noema conversation id.
      public var conversationId: String { __data["conversationId"] }
      /// Provider used for the conversation.
      public var provider: String { __data["provider"] }
    }
  }
}
