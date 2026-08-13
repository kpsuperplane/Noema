// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct MemoryTreeQuery: GraphQLQuery {
  public static let operationName: String = "MemoryTree"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"query MemoryTree { memoryTree { __typename root { __typename id path title icon body hash citations { __typename sources { __typename source kind excerpt createdAt } } parent ancestors { __typename id path title } children { __typename id path title icon excerpt hash } } pages { __typename id path title icon excerpt hash } pendingCount updateStatus { __typename state active lastConsolidatedSequence lastConsolidatedItem error updatedAt } } }"#
    ))

  public init() {}

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.QueryRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("memoryTree", MemoryTree.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      MemoryTreeQuery.Data.self
    ] }

    /// Return the canonical native Markdown memory tree.
    public var memoryTree: MemoryTree { __data["memoryTree"] }

    /// MemoryTree
    ///
    /// Parent Type: `GraphqlNativeMemoryTree`
    nonisolated public struct MemoryTree: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.GraphqlNativeMemoryTree }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("root", Root?.self),
        .field("pages", [Page].self),
        .field("pendingCount", Int.self),
        .field("updateStatus", UpdateStatus.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        MemoryTreeQuery.Data.MemoryTree.self
      ] }

      public var root: Root? { __data["root"] }
      public var pages: [Page] { __data["pages"] }
      public var pendingCount: Int { __data["pendingCount"] }
      public var updateStatus: UpdateStatus { __data["updateStatus"] }

      /// MemoryTree.Root
      ///
      /// Parent Type: `GraphqlNativeMemoryPage`
      nonisolated public struct Root: NoemaAPI.SelectionSet {
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
          .field("citations", [Citation].self),
          .field("parent", String?.self),
          .field("ancestors", [Ancestor].self),
          .field("children", [Child].self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          MemoryTreeQuery.Data.MemoryTree.Root.self
        ] }

        public var id: String { __data["id"] }
        public var path: String { __data["path"] }
        public var title: String { __data["title"] }
        public var icon: String { __data["icon"] }
        public var body: String { __data["body"] }
        public var hash: String { __data["hash"] }
        public var citations: [Citation] { __data["citations"] }
        public var parent: String? { __data["parent"] }
        public var ancestors: [Ancestor] { __data["ancestors"] }
        public var children: [Child] { __data["children"] }

        /// MemoryTree.Root.Citation
        ///
        /// Parent Type: `GraphqlNativeMemoryCitation`
        nonisolated public struct Citation: NoemaAPI.SelectionSet {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.GraphqlNativeMemoryCitation }
          @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .field("sources", [Source].self),
          ] }
          @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
            MemoryTreeQuery.Data.MemoryTree.Root.Citation.self
          ] }

          public var sources: [Source] { __data["sources"] }

          /// MemoryTree.Root.Citation.Source
          ///
          /// Parent Type: `GraphqlNativeMemorySourceReference`
          nonisolated public struct Source: NoemaAPI.SelectionSet {
            @_spi(Unsafe) public let __data: DataDict
            @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

            @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.GraphqlNativeMemorySourceReference }
            @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
              .field("__typename", String.self),
              .field("source", String.self),
              .field("kind", GraphQLEnum<NoemaAPI.GraphqlNativeMemorySourceKind>.self),
              .field("excerpt", String?.self),
              .field("createdAt", String?.self),
            ] }
            @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
              MemoryTreeQuery.Data.MemoryTree.Root.Citation.Source.self
            ] }

            public var source: String { __data["source"] }
            public var kind: GraphQLEnum<NoemaAPI.GraphqlNativeMemorySourceKind> { __data["kind"] }
            public var excerpt: String? { __data["excerpt"] }
            public var createdAt: String? { __data["createdAt"] }
          }
        }

        /// MemoryTree.Root.Ancestor
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
            MemoryTreeQuery.Data.MemoryTree.Root.Ancestor.self
          ] }

          public var id: String { __data["id"] }
          public var path: String { __data["path"] }
          public var title: String { __data["title"] }
        }

        /// MemoryTree.Root.Child
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
            MemoryTreeQuery.Data.MemoryTree.Root.Child.self
          ] }

          public var id: String { __data["id"] }
          public var path: String { __data["path"] }
          public var title: String { __data["title"] }
          public var icon: String { __data["icon"] }
          public var excerpt: String { __data["excerpt"] }
          public var hash: String { __data["hash"] }
        }
      }

      /// MemoryTree.Page
      ///
      /// Parent Type: `GraphqlNativeMemoryPageRef`
      nonisolated public struct Page: NoemaAPI.SelectionSet {
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
          MemoryTreeQuery.Data.MemoryTree.Page.self
        ] }

        public var id: String { __data["id"] }
        public var path: String { __data["path"] }
        public var title: String { __data["title"] }
        public var icon: String { __data["icon"] }
        public var excerpt: String { __data["excerpt"] }
        public var hash: String { __data["hash"] }
      }

      /// MemoryTree.UpdateStatus
      ///
      /// Parent Type: `GraphqlNativeMemoryUpdateStatus`
      nonisolated public struct UpdateStatus: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.GraphqlNativeMemoryUpdateStatus }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("state", String.self),
          .field("active", Bool.self),
          .field("lastConsolidatedSequence", Int.self),
          .field("lastConsolidatedItem", String?.self),
          .field("error", String?.self),
          .field("updatedAt", String?.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          MemoryTreeQuery.Data.MemoryTree.UpdateStatus.self
        ] }

        public var state: String { __data["state"] }
        public var active: Bool { __data["active"] }
        public var lastConsolidatedSequence: Int { __data["lastConsolidatedSequence"] }
        public var lastConsolidatedItem: String? { __data["lastConsolidatedItem"] }
        public var error: String? { __data["error"] }
        public var updatedAt: String? { __data["updatedAt"] }
      }
    }
  }
}
