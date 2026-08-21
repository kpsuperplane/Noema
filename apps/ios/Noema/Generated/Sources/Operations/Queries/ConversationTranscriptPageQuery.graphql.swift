// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct ConversationTranscriptPageQuery: GraphQLQuery {
  public static let operationName: String = "ConversationTranscriptPage"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"query ConversationTranscriptPage($input: ConversationTranscriptPageInput!) { conversationTranscriptPage(input: $input) { __typename ...NativeConversationPageFields } }"#,
      fragments: [NativeConversationItemFields.self, NativeConversationPageFields.self, TasksTaskReferenceSummaryFields.self]
    ))

  public var input: ConversationTranscriptPageInput

  public init(input: ConversationTranscriptPageInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.QueryRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("conversationTranscriptPage", ConversationTranscriptPage.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      ConversationTranscriptPageQuery.Data.self
    ] }

    /// Return a cursor-based page of visible conversation transcript items.
    public var conversationTranscriptPage: ConversationTranscriptPage { __data["conversationTranscriptPage"] }

    /// ConversationTranscriptPage
    ///
    /// Parent Type: `ConversationTranscriptPage`
    nonisolated public struct ConversationTranscriptPage: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.ConversationTranscriptPage }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .fragment(NativeConversationPageFields.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        ConversationTranscriptPageQuery.Data.ConversationTranscriptPage.self,
        NativeConversationPageFields.self
      ] }

      /// Visible transcript items in display order.
      public var items: [Item] { __data["items"] }
      /// Paging metadata for older reads.
      public var pageInfo: PageInfo { __data["pageInfo"] }

      public struct Fragments: FragmentContainer {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        public var nativeConversationPageFields: NativeConversationPageFields { _toFragment() }
      }

      public typealias Item = NativeConversationPageFields.Item

      public typealias PageInfo = NativeConversationPageFields.PageInfo
    }
  }
}
