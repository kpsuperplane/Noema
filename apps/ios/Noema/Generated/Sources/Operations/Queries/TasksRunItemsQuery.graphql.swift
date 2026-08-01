// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct TasksRunItemsQuery: GraphQLQuery {
  public static let operationName: String = "TasksRunItems"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"query TasksRunItems($runId: String!, $first: Int = 50, $after: String) { taskRunItems(runId: $runId, first: $first, after: $after) { __typename edges { __typename cursor node { __typename itemId runId cursor sequenceIndex roundIndex kind status correlationId parentItemId contentText payload createdAt updatedAt } } pageInfo { __typename ...TasksPageInfoFields } } }"#,
      fragments: [TasksPageInfoFields.self]
    ))

  public var runId: String
  public var first: GraphQLNullable<Int32>
  public var after: GraphQLNullable<String>

  public init(
    runId: String,
    first: GraphQLNullable<Int32> = 50,
    after: GraphQLNullable<String>
  ) {
    self.runId = runId
    self.first = first
    self.after = after
  }

  @_spi(Unsafe) public var __variables: Variables? { [
    "runId": runId,
    "first": first,
    "after": after
  ] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.QueryRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("taskRunItems", TaskRunItems.self, arguments: [
        "runId": .variable("runId"),
        "first": .variable("first"),
        "after": .variable("after")
      ]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksRunItemsQuery.Data.self
    ] }

    /// Return a newest-page, owner-authorized task-run transcript connection.
    public var taskRunItems: TaskRunItems { __data["taskRunItems"] }

    /// TaskRunItems
    ///
    /// Parent Type: `TaskRunItemConnection`
    nonisolated public struct TaskRunItems: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskRunItemConnection }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("edges", [Edge].self),
        .field("pageInfo", PageInfo.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        TasksRunItemsQuery.Data.TaskRunItems.self
      ] }

      /// Ordered history edges.
      public var edges: [Edge] { __data["edges"] }
      /// Pagination metadata.
      public var pageInfo: PageInfo { __data["pageInfo"] }

      /// TaskRunItems.Edge
      ///
      /// Parent Type: `TaskRunItemEdge`
      nonisolated public struct Edge: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskRunItemEdge }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("cursor", String.self),
          .field("node", Node.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          TasksRunItemsQuery.Data.TaskRunItems.Edge.self
        ] }

        /// Opaque history cursor.
        public var cursor: String { __data["cursor"] }
        /// Connection node.
        public var node: Node { __data["node"] }

        /// TaskRunItems.Edge.Node
        ///
        /// Parent Type: `TaskRunItem`
        nonisolated public struct Node: NoemaAPI.SelectionSet {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskRunItem }
          @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .field("itemId", String.self),
            .field("runId", String.self),
            .field("cursor", String.self),
            .field("sequenceIndex", Int.self),
            .field("roundIndex", Int.self),
            .field("kind", GraphQLEnum<NoemaAPI.TaskRunItemKind>.self),
            .field("status", GraphQLEnum<NoemaAPI.TaskRunItemStatus>.self),
            .field("correlationId", String?.self),
            .field("parentItemId", String?.self),
            .field("contentText", String?.self),
            .field("payload", NoemaAPI.JSON.self),
            .field("createdAt", String.self),
            .field("updatedAt", String.self),
          ] }
          @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
            TasksRunItemsQuery.Data.TaskRunItems.Edge.Node.self
          ] }

          /// Item identity.
          public var itemId: String { __data["itemId"] }
          /// Run identity.
          public var runId: String { __data["runId"] }
          /// Opaque transcript cursor.
          public var cursor: String { __data["cursor"] }
          /// Sequence index.
          public var sequenceIndex: Int { __data["sequenceIndex"] }
          /// Provider round.
          public var roundIndex: Int { __data["roundIndex"] }
          /// Transcript kind.
          public var kind: GraphQLEnum<NoemaAPI.TaskRunItemKind> { __data["kind"] }
          /// Item status.
          public var status: GraphQLEnum<NoemaAPI.TaskRunItemStatus> { __data["status"] }
          /// Correlation identity, when present.
          public var correlationId: String? { __data["correlationId"] }
          /// Parent item, when present.
          public var parentItemId: String? { __data["parentItemId"] }
          /// Safe content text.
          public var contentText: String? { __data["contentText"] }
          /// Safe structured payload.
          public var payload: NoemaAPI.JSON { __data["payload"] }
          /// Creation timestamp.
          public var createdAt: String { __data["createdAt"] }
          /// Last update timestamp.
          public var updatedAt: String { __data["updatedAt"] }
        }
      }

      /// TaskRunItems.PageInfo
      ///
      /// Parent Type: `PageInfo`
      nonisolated public struct PageInfo: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.PageInfo }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .fragment(TasksPageInfoFields.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          TasksRunItemsQuery.Data.TaskRunItems.PageInfo.self,
          TasksPageInfoFields.self
        ] }

        /// Cursor for the final edge in this page.
        public var endCursor: String? { __data["endCursor"] }
        /// Whether another page exists.
        public var hasNextPage: Bool { __data["hasNextPage"] }

        public struct Fragments: FragmentContainer {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          public var tasksPageInfoFields: TasksPageInfoFields { _toFragment() }
        }
      }
    }
  }
}
