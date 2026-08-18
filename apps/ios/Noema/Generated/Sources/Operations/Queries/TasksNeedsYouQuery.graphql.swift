// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct TasksNeedsYouQuery: GraphQLQuery {
  public static let operationName: String = "TasksNeedsYou"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"query TasksNeedsYou($workspaceId: String!, $projectId: String, $first: Int = 50, $after: String) { needsYou( workspaceId: $workspaceId projectId: $projectId first: $first after: $after ) { __typename edges { __typename cursor node { __typename kind title summary validActions gate { __typename ...TasksGateFields } task { __typename ...TasksTaskCardFields } } } pageInfo { __typename ...TasksPageInfoFields } } }"#,
      fragments: [TasksCurrentRunFields.self, TasksGateFields.self, TasksPageInfoFields.self, TasksProjectFields.self, TasksStageFields.self, TasksTaskCardFields.self, TasksWorkspaceFields.self]
    ))

  public var workspaceId: String
  public var projectId: GraphQLNullable<String>
  public var first: GraphQLNullable<Int32>
  public var after: GraphQLNullable<String>

  public init(
    workspaceId: String,
    projectId: GraphQLNullable<String>,
    first: GraphQLNullable<Int32> = 50,
    after: GraphQLNullable<String>
  ) {
    self.workspaceId = workspaceId
    self.projectId = projectId
    self.first = first
    self.after = after
  }

  @_spi(Unsafe) public var __variables: Variables? { [
    "workspaceId": workspaceId,
    "projectId": projectId,
    "first": first,
    "after": after
  ] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.QueryRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("needsYou", NeedsYou.self, arguments: [
        "workspaceId": .variable("workspaceId"),
        "projectId": .variable("projectId"),
        "first": .variable("first"),
        "after": .variable("after")
      ]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksNeedsYouQuery.Data.self
    ] }

    /// Return unresolved gate/review attention cards.
    public var needsYou: NeedsYou { __data["needsYou"] }

    /// NeedsYou
    ///
    /// Parent Type: `TaskAttentionConnection`
    nonisolated public struct NeedsYou: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskAttentionConnection }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("edges", [Edge].self),
        .field("pageInfo", PageInfo.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        TasksNeedsYouQuery.Data.NeedsYou.self
      ] }

      /// Ordered attention edges.
      public var edges: [Edge] { __data["edges"] }
      /// Pagination metadata.
      public var pageInfo: PageInfo { __data["pageInfo"] }

      /// NeedsYou.Edge
      ///
      /// Parent Type: `TaskAttentionEdge`
      nonisolated public struct Edge: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskAttentionEdge }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("cursor", String.self),
          .field("node", Node.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          TasksNeedsYouQuery.Data.NeedsYou.Edge.self
        ] }

        /// Opaque attention cursor.
        public var cursor: String { __data["cursor"] }
        /// Connection node.
        public var node: Node { __data["node"] }

        /// NeedsYou.Edge.Node
        ///
        /// Parent Type: `TaskAttention`
        nonisolated public struct Node: NoemaAPI.SelectionSet {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskAttention }
          @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .field("kind", GraphQLEnum<NoemaAPI.TaskAttentionKind>.self),
            .field("title", String.self),
            .field("summary", String.self),
            .field("validActions", [GraphQLEnum<NoemaAPI.ValidTaskAction>].self),
            .field("gate", Gate?.self),
            .field("task", Task.self),
          ] }
          @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
            TasksNeedsYouQuery.Data.NeedsYou.Edge.Node.self
          ] }

          /// Clarification, approval, recovery, or review readiness.
          public var kind: GraphQLEnum<NoemaAPI.TaskAttentionKind> { __data["kind"] }
          /// Stable UI title.
          public var title: String { __data["title"] }
          /// Safe summary.
          public var summary: String { __data["summary"] }
          /// Server-authorized actions.
          public var validActions: [GraphQLEnum<NoemaAPI.ValidTaskAction>] { __data["validActions"] }
          /// Complete related gate evidence, when any.
          public var gate: Gate? { __data["gate"] }
          /// Authoritative task card without recursively embedding attention.
          public var task: Task { __data["task"] }

          /// NeedsYou.Edge.Node.Gate
          ///
          /// Parent Type: `TaskGate`
          nonisolated public struct Gate: NoemaAPI.SelectionSet {
            @_spi(Unsafe) public let __data: DataDict
            @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

            @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskGate }
            @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
              .field("__typename", String.self),
              .fragment(TasksGateFields.self),
            ] }
            @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
              TasksNeedsYouQuery.Data.NeedsYou.Edge.Node.Gate.self,
              TasksGateFields.self
            ] }

            /// Gate identity.
            public var gateId: String { __data["gateId"] }
            /// Task generation.
            public var taskGeneration: Int { __data["taskGeneration"] }
            /// Gate kind.
            public var kind: GraphQLEnum<NoemaAPI.TaskGateKind> { __data["kind"] }
            /// Gate state.
            public var state: GraphQLEnum<NoemaAPI.TaskGateState> { __data["state"] }
            /// Recovery reason, when any.
            public var recoveryReason: GraphQLEnum<NoemaAPI.TaskRecoveryReason>? { __data["recoveryReason"] }
            /// Explicit recovery continuation role, when any.
            public var retryRunKind: GraphQLEnum<NoemaAPI.TaskRunKind>? { __data["retryRunKind"] }
            /// Human prompt.
            public var prompt: String { __data["prompt"] }
            /// Bounded context.
            public var contextMarkdown: String { __data["contextMarkdown"] }
            /// Optional direct answers.
            public var suggestedAnswers: [String] { __data["suggestedAnswers"] }
            /// Opener actor.
            public var openedBy: String { __data["openedBy"] }
            /// Opening run, when any.
            public var originatingRunId: String? { __data["originatingRunId"] }
            /// Open timestamp.
            public var openedAt: String { __data["openedAt"] }
            /// Resolver actor, when resolved.
            public var resolvedBy: String? { __data["resolvedBy"] }
            /// Resolution timestamp, when resolved.
            public var resolvedAt: String? { __data["resolvedAt"] }
            /// Resolution message identity, when resolved.
            public var resolution: String? { __data["resolution"] }

            public struct Fragments: FragmentContainer {
              @_spi(Unsafe) public let __data: DataDict
              @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

              public var tasksGateFields: TasksGateFields { _toFragment() }
            }
          }

          /// NeedsYou.Edge.Node.Task
          ///
          /// Parent Type: `TaskCard`
          nonisolated public struct Task: NoemaAPI.SelectionSet {
            @_spi(Unsafe) public let __data: DataDict
            @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

            @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskCard }
            @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
              .field("__typename", String.self),
              .fragment(TasksTaskCardFields.self),
            ] }
            @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
              TasksNeedsYouQuery.Data.NeedsYou.Edge.Node.Task.self,
              TasksTaskCardFields.self
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
            /// Server-authorized actions.
            public var validActions: [GraphQLEnum<NoemaAPI.ValidTaskAction>] { __data["validActions"] }

            public struct Fragments: FragmentContainer {
              @_spi(Unsafe) public let __data: DataDict
              @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

              public var tasksTaskCardFields: TasksTaskCardFields { _toFragment() }
            }

            public typealias Workspace = TasksTaskCardFields.Workspace

            public typealias Project = TasksTaskCardFields.Project

            public typealias Schedule = TasksTaskCardFields.Schedule

            public typealias Stage = TasksTaskCardFields.Stage

            public typealias CurrentRun = TasksTaskCardFields.CurrentRun

            public typealias ActiveGate = TasksTaskCardFields.ActiveGate
          }
        }
      }

      /// NeedsYou.PageInfo
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
          TasksNeedsYouQuery.Data.NeedsYou.PageInfo.self,
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
