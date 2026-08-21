// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct TasksEventsSubscription: GraphQLSubscription {
  public static let operationName: String = "TasksEvents"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"subscription TasksEvents($workspaceId: String!, $after: String) { tasksEvents(workspaceId: $workspaceId, after: $after) { __typename ...TasksEventFields task { __typename ...TasksTaskReferenceSummaryFields } } }"#,
      fragments: [TasksEventFields.self, TasksTaskReferenceSummaryFields.self]
    ))

  public var workspaceId: String
  public var after: GraphQLNullable<String>

  public init(
    workspaceId: String,
    after: GraphQLNullable<String>
  ) {
    self.workspaceId = workspaceId
    self.after = after
  }

  @_spi(Unsafe) public var __variables: Variables? { [
    "workspaceId": workspaceId,
    "after": after
  ] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.SubscriptionRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("tasksEvents", TasksEvents.self, arguments: [
        "workspaceId": .variable("workspaceId"),
        "after": .variable("after")
      ]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksEventsSubscription.Data.self
    ] }

    /// Replay and stream workspace-scoped Tasks events.
    public var tasksEvents: TasksEvents { __data["tasksEvents"] }

    /// TasksEvents
    ///
    /// Parent Type: `TasksEvent`
    nonisolated public struct TasksEvents: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TasksEvent }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("task", Task?.self),
        .fragment(TasksEventFields.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        TasksEventsSubscription.Data.TasksEvents.self,
        TasksEventFields.self
      ] }

      /// Current task summary when this event has a task.
      public var task: Task? { __data["task"] }
      /// Opaque global event cursor.
      public var cursor: String { __data["cursor"] }
      /// Event identity.
      public var eventId: String { __data["eventId"] }
      /// Closed dotted event kind.
      public var kind: String { __data["kind"] }
      /// Event timestamp.
      public var occurredAt: String { __data["occurredAt"] }
      /// Workspace linkage.
      public var workspaceId: String { __data["workspaceId"] }
      /// Project linkage.
      public var projectId: String? { __data["projectId"] }
      /// Task linkage.
      public var taskId: String? { __data["taskId"] }
      /// Run linkage.
      public var runId: String? { __data["runId"] }
      /// Safe audit actor.
      public var actor: String { __data["actor"] }
      /// Causation identity.
      public var causationId: String? { __data["causationId"] }
      /// Correlation identity.
      public var correlationId: String { __data["correlationId"] }
      /// Bounded safe JSON payload.
      public var payload: NoemaAPI.JSON { __data["payload"] }

      public struct Fragments: FragmentContainer {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        public var tasksEventFields: TasksEventFields { _toFragment() }
      }

      /// TasksEvents.Task
      ///
      /// Parent Type: `TaskSummary`
      nonisolated public struct Task: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskSummary }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .fragment(TasksTaskReferenceSummaryFields.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          TasksEventsSubscription.Data.TasksEvents.Task.self,
          TasksTaskReferenceSummaryFields.self
        ] }

        /// Task identity.
        public var taskId: String { __data["taskId"] }
        /// Human title.
        public var title: String { __data["title"] }
        /// The only task-level state.
        public var stage: Stage { __data["stage"] }
        /// Completion timestamp, when any.
        public var completedAt: String? { __data["completedAt"] }
        /// Current run projection.
        public var currentRun: CurrentRun? { __data["currentRun"] }
        /// Derived attention.
        public var attention: Attention? { __data["attention"] }

        public struct Fragments: FragmentContainer {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          public var tasksTaskReferenceSummaryFields: TasksTaskReferenceSummaryFields { _toFragment() }
        }

        public typealias Stage = TasksTaskReferenceSummaryFields.Stage

        public typealias CurrentRun = TasksTaskReferenceSummaryFields.CurrentRun

        public typealias Attention = TasksTaskReferenceSummaryFields.Attention
      }
    }
  }
}
