// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct TasksOverviewQuery: GraphQLQuery {
  public static let operationName: String = "TasksOverview"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"query TasksOverview($workspaceId: String!, $projectId: String) { tasksOverview(workspaceId: $workspaceId, projectId: $projectId) { __typename workspace { __typename ...TasksWorkspaceFields } workflow { __typename workflowId name stages { __typename ...TasksStageFields } } boardColumns { __typename stage { __typename ...TasksStageFields } taskCount } recentTasks { __typename edges { __typename cursor node { __typename ...TasksTaskSummaryFields } } pageInfo { __typename ...TasksPageInfoFields } } needsYouCount } }"#,
      fragments: [TasksCurrentRunFields.self, TasksGateFields.self, TasksPageInfoFields.self, TasksProjectFields.self, TasksReviewSummaryFields.self, TasksStageFields.self, TasksTaskSummaryFields.self, TasksWorkspaceFields.self]
    ))

  public var workspaceId: String
  public var projectId: GraphQLNullable<String>

  public init(
    workspaceId: String,
    projectId: GraphQLNullable<String>
  ) {
    self.workspaceId = workspaceId
    self.projectId = projectId
  }

  @_spi(Unsafe) public var __variables: Variables? { [
    "workspaceId": workspaceId,
    "projectId": projectId
  ] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.QueryRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("tasksOverview", TasksOverview.self, arguments: [
        "workspaceId": .variable("workspaceId"),
        "projectId": .variable("projectId")
      ]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksOverviewQuery.Data.self
    ] }

    /// Return one coherent Tasks overview.
    public var tasksOverview: TasksOverview { __data["tasksOverview"] }

    /// TasksOverview
    ///
    /// Parent Type: `TasksOverview`
    nonisolated public struct TasksOverview: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TasksOverview }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("workspace", Workspace.self),
        .field("workflow", Workflow.self),
        .field("boardColumns", [BoardColumn].self),
        .field("recentTasks", RecentTasks.self),
        .field("needsYouCount", Int.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        TasksOverviewQuery.Data.TasksOverview.self
      ] }

      /// Authorized workspace metadata.
      public var workspace: Workspace { __data["workspace"] }
      /// Default workflow and all ordered stages.
      public var workflow: Workflow { __data["workflow"] }
      /// Board-visible columns in workflow order.
      public var boardColumns: [BoardColumn] { __data["boardColumns"] }
      /// Bounded recent active task cards.
      public var recentTasks: RecentTasks { __data["recentTasks"] }
      /// Active cards requiring human attention.
      public var needsYouCount: Int { __data["needsYouCount"] }

      /// TasksOverview.Workspace
      ///
      /// Parent Type: `Workspace`
      nonisolated public struct Workspace: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.Workspace }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .fragment(TasksWorkspaceFields.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          TasksOverviewQuery.Data.TasksOverview.Workspace.self,
          TasksWorkspaceFields.self
        ] }

        /// Opaque workspace identity.
        public var workspaceId: String { __data["workspaceId"] }
        /// Human-readable name.
        public var name: String { __data["name"] }
        /// Descriptive text.
        public var description: String { __data["description"] }
        /// Whether this is the Personal workspace.
        public var isPersonal: Bool { __data["isPersonal"] }
        /// Membership role for the authenticated owner.
        public var membershipRole: GraphQLEnum<NoemaAPI.WorkspaceMembershipRole> { __data["membershipRole"] }

        public struct Fragments: FragmentContainer {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          public var tasksWorkspaceFields: TasksWorkspaceFields { _toFragment() }
        }
      }

      /// TasksOverview.Workflow
      ///
      /// Parent Type: `Workflow`
      nonisolated public struct Workflow: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.Workflow }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("workflowId", String.self),
          .field("name", String.self),
          .field("stages", [Stage].self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          TasksOverviewQuery.Data.TasksOverview.Workflow.self
        ] }

        /// Opaque workflow identity.
        public var workflowId: String { __data["workflowId"] }
        /// Workflow name.
        public var name: String { __data["name"] }
        /// Stage definitions in display order.
        public var stages: [Stage] { __data["stages"] }

        /// TasksOverview.Workflow.Stage
        ///
        /// Parent Type: `WorkflowStage`
        nonisolated public struct Stage: NoemaAPI.SelectionSet {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.WorkflowStage }
          @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .fragment(TasksStageFields.self),
          ] }
          @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
            TasksOverviewQuery.Data.TasksOverview.Workflow.Stage.self,
            TasksStageFields.self
          ] }

          /// Opaque stage identity.
          public var stageId: String { __data["stageId"] }
          /// Owning workflow.
          public var workflowId: String { __data["workflowId"] }
          /// Machine-stable key.
          public var key: String { __data["key"] }
          /// Human-facing stage name.
          public var name: String { __data["name"] }
          /// Display ordering.
          public var displayOrder: Int { __data["displayOrder"] }
          /// Closed runtime behavior.
          public var behavior: GraphQLEnum<NoemaAPI.WorkflowStageBehavior> { __data["behavior"] }

          public struct Fragments: FragmentContainer {
            @_spi(Unsafe) public let __data: DataDict
            @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

            public var tasksStageFields: TasksStageFields { _toFragment() }
          }
        }
      }

      /// TasksOverview.BoardColumn
      ///
      /// Parent Type: `TaskStageColumn`
      nonisolated public struct BoardColumn: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskStageColumn }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("stage", Stage.self),
          .field("taskCount", Int.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          TasksOverviewQuery.Data.TasksOverview.BoardColumn.self
        ] }

        /// Stage metadata used to render the column.
        public var stage: Stage { __data["stage"] }
        /// Number of active tasks in this stage and project scope.
        public var taskCount: Int { __data["taskCount"] }

        /// TasksOverview.BoardColumn.Stage
        ///
        /// Parent Type: `WorkflowStage`
        nonisolated public struct Stage: NoemaAPI.SelectionSet {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.WorkflowStage }
          @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .fragment(TasksStageFields.self),
          ] }
          @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
            TasksOverviewQuery.Data.TasksOverview.BoardColumn.Stage.self,
            TasksStageFields.self
          ] }

          /// Opaque stage identity.
          public var stageId: String { __data["stageId"] }
          /// Owning workflow.
          public var workflowId: String { __data["workflowId"] }
          /// Machine-stable key.
          public var key: String { __data["key"] }
          /// Human-facing stage name.
          public var name: String { __data["name"] }
          /// Display ordering.
          public var displayOrder: Int { __data["displayOrder"] }
          /// Closed runtime behavior.
          public var behavior: GraphQLEnum<NoemaAPI.WorkflowStageBehavior> { __data["behavior"] }

          public struct Fragments: FragmentContainer {
            @_spi(Unsafe) public let __data: DataDict
            @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

            public var tasksStageFields: TasksStageFields { _toFragment() }
          }
        }
      }

      /// TasksOverview.RecentTasks
      ///
      /// Parent Type: `TaskConnection`
      nonisolated public struct RecentTasks: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskConnection }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("edges", [Edge].self),
          .field("pageInfo", PageInfo.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          TasksOverviewQuery.Data.TasksOverview.RecentTasks.self
        ] }

        /// Ordered task edges.
        public var edges: [Edge] { __data["edges"] }
        /// Pagination metadata.
        public var pageInfo: PageInfo { __data["pageInfo"] }

        /// TasksOverview.RecentTasks.Edge
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
            TasksOverviewQuery.Data.TasksOverview.RecentTasks.Edge.self
          ] }

          /// Opaque keyset cursor.
          public var cursor: String { __data["cursor"] }
          /// Connection node.
          public var node: Node { __data["node"] }

          /// TasksOverview.RecentTasks.Edge.Node
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
              TasksOverviewQuery.Data.TasksOverview.RecentTasks.Edge.Node.self,
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
            /// Explicit task working-directory override.
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
            /// Latest review projection.
            public var latestReview: LatestReview? { __data["latestReview"] }
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

            public typealias LatestReview = TasksTaskSummaryFields.LatestReview

            public typealias Attention = TasksTaskSummaryFields.Attention
          }
        }

        /// TasksOverview.RecentTasks.PageInfo
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
            TasksOverviewQuery.Data.TasksOverview.RecentTasks.PageInfo.self,
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
}
