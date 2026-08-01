// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct PrimaryConversationQuery: GraphQLQuery {
  public static let operationName: String = "PrimaryConversation"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"query PrimaryConversation { primaryConversation { __typename conversationId provider } }"#
    ))

  public init() {}

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.QueryRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("primaryConversation", PrimaryConversation?.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      PrimaryConversationQuery.Data.self
    ] }

    /// Return the primary conversation identity without creating it or replaying transcript.
    public var primaryConversation: PrimaryConversation? { __data["primaryConversation"] }

    /// PrimaryConversation
    ///
    /// Parent Type: `PrimaryConversation`
    nonisolated public struct PrimaryConversation: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.PrimaryConversation }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("conversationId", String.self),
        .field("provider", String.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        PrimaryConversationQuery.Data.PrimaryConversation.self
      ] }

      /// Durable Noema conversation id.
      public var conversationId: String { __data["conversationId"] }
      /// Provider used for the conversation.
      public var provider: String { __data["provider"] }
    }
  }
}
