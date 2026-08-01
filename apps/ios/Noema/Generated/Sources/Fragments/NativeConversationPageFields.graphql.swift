// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct NativeConversationPageFields: NoemaAPI.SelectionSet, Fragment {
  public static var fragmentDefinition: StaticString {
    #"fragment NativeConversationPageFields on ConversationTranscriptPage { __typename items { __typename ...NativeConversationItemFields } pageInfo { __typename beforeCursor hasMoreBefore limit } }"#
  }

  @_spi(Unsafe) public let __data: DataDict
  @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

  @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.ConversationTranscriptPage }
  @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
    .field("__typename", String.self),
    .field("items", [Item].self),
    .field("pageInfo", PageInfo.self),
  ] }
  @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
    NativeConversationPageFields.self
  ] }

  /// Visible transcript items in display order.
  public var items: [Item] { __data["items"] }
  /// Paging metadata for older reads.
  public var pageInfo: PageInfo { __data["pageInfo"] }

  /// Item
  ///
  /// Parent Type: `ConversationItem`
  nonisolated public struct Item: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.ConversationItem }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("__typename", String.self),
      .fragment(NativeConversationItemFields.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      NativeConversationPageFields.Item.self,
      NativeConversationItemFields.self
    ] }

    /// Durable conversation item id.
    public var itemId: String { __data["itemId"] }
    /// Opaque durable pagination cursor.
    public var cursor: String { __data["cursor"] }
    /// Durable conversation turn id.
    public var turnId: String? { __data["turnId"] }
    /// Structured durable item metadata.
    public var metadata: NoemaAPI.JSON { __data["metadata"] }
    /// Transcript item to render.
    public var item: Item { __data["item"] }

    public struct Fragments: FragmentContainer {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      public var nativeConversationItemFields: NativeConversationItemFields { _toFragment() }
    }

    public typealias Item = NativeConversationItemFields.Item
  }

  /// PageInfo
  ///
  /// Parent Type: `ConversationTranscriptPageInfo`
  nonisolated public struct PageInfo: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.ConversationTranscriptPageInfo }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("__typename", String.self),
      .field("beforeCursor", String?.self),
      .field("hasMoreBefore", Bool.self),
      .field("limit", Int.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      NativeConversationPageFields.PageInfo.self
    ] }

    /// Cursor before the first returned item.
    public var beforeCursor: String? { __data["beforeCursor"] }
    /// Whether more visible items exist before this page.
    public var hasMoreBefore: Bool { __data["hasMoreBefore"] }
    /// Applied page size.
    public var limit: Int { __data["limit"] }
  }
}
