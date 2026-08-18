// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct TasksRunScheduledTaskNowMutation: GraphQLMutation {
  public static let operationName: String = "TasksRunScheduledTaskNow"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation TasksRunScheduledTaskNow($input: RunScheduledTaskNowInput!) { runScheduledTaskNow(input: $input) { __typename task { __typename ...TasksCommandTaskFields } eventCursor clientMutationId } }"#,
      fragments: [TasksCommandTaskFields.self, TasksCurrentRunFields.self, TasksGateFields.self, TasksStageFields.self]
    ))

  public var input: RunScheduledTaskNowInput

  public init(input: RunScheduledTaskNowInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("runScheduledTaskNow", RunScheduledTaskNow.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksRunScheduledTaskNowMutation.Data.self
    ] }

    /// Start an already scheduled Inbox task immediately.
    public var runScheduledTaskNow: RunScheduledTaskNow { __data["runScheduledTaskNow"] }

    /// RunScheduledTaskNow
    ///
    /// Parent Type: `TaskCommandPayload`
    nonisolated public struct RunScheduledTaskNow: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskCommandPayload }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("task", Task.self),
        .field("eventCursor", String.self),
        .field("clientMutationId", String.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        TasksRunScheduledTaskNowMutation.Data.RunScheduledTaskNow.self
      ] }

      /// Authoritative task projection.
      public var task: Task { __data["task"] }
      /// Cursor for the event committed by the command.
      public var eventCursor: String { __data["eventCursor"] }
      /// Echoed caller idempotency key.
      public var clientMutationId: String { __data["clientMutationId"] }

      /// RunScheduledTaskNow.Task
      ///
      /// Parent Type: `TaskDetail`
      nonisolated public struct Task: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskDetail }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .fragment(TasksCommandTaskFields.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          TasksRunScheduledTaskNowMutation.Data.RunScheduledTaskNow.Task.self,
          TasksCommandTaskFields.self
        ] }

        /// Task identity.
        public var taskId: String { __data["taskId"] }
        /// Full title.
        public var title: String { __data["title"] }
        /// Full description Markdown.
        public var description: String { __data["description"] }
        /// Assigned executor agent identity.
        public var executorAgentId: String { __data["executorAgentId"] }
        /// Assigned executor backend.
        public var executorBackend: String { __data["executorBackend"] }
        /// Explicit Task directory base override.
        public var cwdOverride: String? { __data["cwdOverride"] }
        /// Derived or frozen effective working directory.
        public var effectiveCwd: String? { __data["effectiveCwd"] }
        /// Effective working-directory source: task, project, or default.
        public var effectiveCwdSource: String { __data["effectiveCwdSource"] }
        /// The only task-level state.
        public var stage: Stage { __data["stage"] }
        /// Optimistic revision.
        public var revision: Int { __data["revision"] }
        /// Execution generation fence.
        public var generation: Int { __data["generation"] }
        /// Last update timestamp.
        public var updatedAt: String { __data["updatedAt"] }
        /// Optional future execution and recurrence provenance.
        public var schedule: Schedule? { __data["schedule"] }
        /// Completion timestamp, when any.
        public var completedAt: String? { __data["completedAt"] }
        /// Server-authorized actions.
        public var validActions: [GraphQLEnum<NoemaAPI.ValidTaskAction>] { __data["validActions"] }
        /// Open gate projection.
        public var activeGate: ActiveGate? { __data["activeGate"] }
        /// Current run projection.
        public var currentRun: CurrentRun? { __data["currentRun"] }

        public struct Fragments: FragmentContainer {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          public var tasksCommandTaskFields: TasksCommandTaskFields { _toFragment() }
        }

        public typealias Stage = TasksCommandTaskFields.Stage

        public typealias Schedule = TasksCommandTaskFields.Schedule

        public typealias ActiveGate = TasksCommandTaskFields.ActiveGate

        public typealias CurrentRun = TasksCommandTaskFields.CurrentRun
      }
    }
  }
}
