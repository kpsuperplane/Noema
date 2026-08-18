// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct TasksListQuery: GraphQLQuery {
  public static let operationName: String = "TasksList"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"query TasksList($input: TaskListInput!, $first: Int = 50, $after: String) { tasks(input: $input, first: $first, after: $after) { __typename edges { __typename cursor node { __typename ...TasksTaskSummaryFields } } pageInfo { __typename ...TasksPageInfoFields } } }"#,
      fragments: [TasksCurrentRunFields.self, TasksGateFields.self, TasksPageInfoFields.self, TasksProjectFields.self, TasksStageFields.self, TasksTaskSummaryFields.self, TasksWorkspaceFields.self]
    ))

  public var input: TaskListInput
  public var first: GraphQLNullable<Int32>
  public var after: GraphQLNullable<String>

  public init(
    input: TaskListInput,
    first: GraphQLNullable<Int32> = 50,
    after: GraphQLNullable<String>
  ) {
    self.input = input
    self.first = first
    self.after = after
  }

  @_spi(Unsafe) public var __variables: Variables? { [
    "input": input,
    "first": first,
    "after": after
  ] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.QueryRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("tasks", Tasks.self, arguments: [
        "input": .variable("input"),
        "first": .variable("first"),
        "after": .variable("after")
      ]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksListQuery.Data.self
    ] }

    /// List tasks through one bounded store query.
    public var tasks: Tasks { __data["tasks"] }

    /// Tasks
    ///
    /// Parent Type: `TaskConnection`
    nonisolated public struct Tasks: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskConnection }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("edges", [Edge].self),
        .field("pageInfo", PageInfo.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        TasksListQuery.Data.Tasks.self
      ] }

      /// Ordered task edges.
      public var edges: [Edge] { __data["edges"] }
      /// Pagination metadata.
      public var pageInfo: PageInfo { __data["pageInfo"] }

      /// Tasks.Edge
      ///
      /// Parent Type: `TaskEdge`
      nonisolated public struct Edge: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskEdge }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("cursor", String.self),
          .field("node", Node.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          TasksListQuery.Data.Tasks.Edge.self
        ] }

        /// Opaque keyset cursor.
        public var cursor: String { __data["cursor"] }
        /// Connection node.
        public var node: Node { __data["node"] }

        /// Tasks.Edge.Node
        ///
        /// Parent Type: `TaskSummary`
        nonisolated public struct Node: NoemaAPI.SelectionSet {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskSummary }
          @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .fragment(TasksTaskSummaryFields.self),
          ] }
          @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
            TasksListQuery.Data.Tasks.Edge.Node.self,
            TasksTaskSummaryFields.self
          ] }

          /// Task identity.
          public var taskId: String { __data["taskId"] }
          /// Workspace placement.
          public var workspace: Workspace { __data["workspace"] }
          /// Nullable project placement.
          public var project: Project? { __data["project"] }
          /// Human title.
          public var title: String { __data["title"] }
          /// Bounded description preview.
          public var descriptionPreview: String { __data["descriptionPreview"] }
          /// Assigned executor agent identity.
          public var executorAgentId: String { __data["executorAgentId"] }
          /// Assigned executor backend.
          public var executorBackend: String { __data["executorBackend"] }
          /// Explicit Task directory base override.
          public var cwdOverride: String? { __data["cwdOverride"] }
          /// Derived effective working directory when already frozen or explicitly configured.
          public var effectiveCwd: String? { __data["effectiveCwd"] }
          /// Effective working-directory source: task, project, or default.
          public var effectiveCwdSource: String { __data["effectiveCwdSource"] }
          /// Optional future execution and recurrence provenance.
          public var schedule: Schedule? { __data["schedule"] }
          /// The only task-level state.
          public var stage: Stage { __data["stage"] }
          /// Optimistic revision.
          public var revision: Int { __data["revision"] }
          /// Execution generation fence.
          public var generation: Int { __data["generation"] }
          /// Creation timestamp.
          public var createdAt: String { __data["createdAt"] }
          /// Last update timestamp.
          public var updatedAt: String { __data["updatedAt"] }
          /// Completion timestamp, when any.
          public var completedAt: String? { __data["completedAt"] }
          /// Current run projection.
          public var currentRun: CurrentRun? { __data["currentRun"] }
          /// Open gate projection.
          public var activeGate: ActiveGate? { __data["activeGate"] }
          /// Derived attention.
          public var attention: Attention? { __data["attention"] }
          /// Server-authorized actions.
          public var validActions: [GraphQLEnum<NoemaAPI.ValidTaskAction>] { __data["validActions"] }

          public struct Fragments: FragmentContainer {
            @_spi(Unsafe) public let __data: DataDict
            @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

            public var tasksTaskSummaryFields: TasksTaskSummaryFields { _toFragment() }
          }

          public typealias Workspace = TasksTaskSummaryFields.Workspace

          public typealias Project = TasksTaskSummaryFields.Project

          public typealias Schedule = TasksTaskSummaryFields.Schedule

          public typealias Stage = TasksTaskSummaryFields.Stage

          public typealias CurrentRun = TasksTaskSummaryFields.CurrentRun

          public typealias ActiveGate = TasksTaskSummaryFields.ActiveGate

          public typealias Attention = TasksTaskSummaryFields.Attention
        }
      }

      /// Tasks.PageInfo
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
          TasksListQuery.Data.Tasks.PageInfo.self,
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
