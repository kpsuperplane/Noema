// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct MemoryPageQuery: GraphQLQuery {
  public static let operationName: String = "MemoryPage"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"query MemoryPage($pageId: String!) { memoryPage(pageId: $pageId) { __typename id path title icon body hash sources sourceReferences { __typename source excerpt } parent ancestors { __typename id path title } children { __typename id path title icon excerpt hash } } }"#
    ))

  public var pageId: String

  public init(pageId: String) {
    self.pageId = pageId
  }

  @_spi(Unsafe) public var __variables: Variables? { ["pageId": pageId] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.QueryRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("memoryPage", MemoryPage?.self, arguments: ["pageId": .variable("pageId")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      MemoryPageQuery.Data.self
    ] }

    /// Read one native Markdown memory page.
    public var memoryPage: MemoryPage? { __data["memoryPage"] }

    /// MemoryPage
    ///
    /// Parent Type: `GraphqlNativeMemoryPage`
    nonisolated public struct MemoryPage: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.GraphqlNativeMemoryPage }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("id", String.self),
        .field("path", String.self),
        .field("title", String.self),
        .field("icon", String.self),
        .field("body", String.self),
        .field("hash", String.self),
        .field("sources", [String].self),
        .field("sourceReferences", [SourceReference].self),
        .field("parent", String?.self),
        .field("ancestors", [Ancestor].self),
        .field("children", [Child].self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        MemoryPageQuery.Data.MemoryPage.self
      ] }

      public var id: String { __data["id"] }
      public var path: String { __data["path"] }
      public var title: String { __data["title"] }
      public var icon: String { __data["icon"] }
      public var body: String { __data["body"] }
      public var hash: String { __data["hash"] }
      public var sources: [String] { __data["sources"] }
      public var sourceReferences: [SourceReference] { __data["sourceReferences"] }
      public var parent: String? { __data["parent"] }
      public var ancestors: [Ancestor] { __data["ancestors"] }
      public var children: [Child] { __data["children"] }

      /// MemoryPage.SourceReference
      ///
      /// Parent Type: `GraphqlNativeMemorySourceReference`
      nonisolated public struct SourceReference: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.GraphqlNativeMemorySourceReference }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("source", String.self),
          .field("excerpt", String?.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          MemoryPageQuery.Data.MemoryPage.SourceReference.self
        ] }

        public var source: String { __data["source"] }
        public var excerpt: String? { __data["excerpt"] }
      }

      /// MemoryPage.Ancestor
      ///
      /// Parent Type: `GraphqlNativeMemoryPageRef`
      nonisolated public struct Ancestor: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.GraphqlNativeMemoryPageRef }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("id", String.self),
          .field("path", String.self),
          .field("title", String.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          MemoryPageQuery.Data.MemoryPage.Ancestor.self
        ] }

        public var id: String { __data["id"] }
        public var path: String { __data["path"] }
        public var title: String { __data["title"] }
      }

      /// MemoryPage.Child
      ///
      /// Parent Type: `GraphqlNativeMemoryPageRef`
      nonisolated public struct Child: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.GraphqlNativeMemoryPageRef }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("id", String.self),
          .field("path", String.self),
          .field("title", String.self),
          .field("icon", String.self),
          .field("excerpt", String.self),
          .field("hash", String.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          MemoryPageQuery.Data.MemoryPage.Child.self
        ] }

        public var id: String { __data["id"] }
        public var path: String { __data["path"] }
        public var title: String { __data["title"] }
        public var icon: String { __data["icon"] }
        public var excerpt: String { __data["excerpt"] }
        public var hash: String { __data["hash"] }
      }
    }
  }
}
