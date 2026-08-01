// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct TasksCancelTaskMutation: GraphQLMutation {
  public static let operationName: String = "TasksCancelTask"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation TasksCancelTask($input: CancelTaskInput!) { cancelTask(input: $input) { __typename task { __typename ...TasksCommandTaskFields } eventCursor clientMutationId } }"#,
      fragments: [TasksCommandTaskFields.self, TasksCurrentRunFields.self, TasksGateFields.self, TasksStageFields.self]
    ))

  public var input: CancelTaskInput

  public init(input: CancelTaskInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("cancelTask", CancelTask.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksCancelTaskMutation.Data.self
    ] }

    /// Cancel a nonterminal task.
    public var cancelTask: CancelTask { __data["cancelTask"] }

    /// CancelTask
    ///
    /// Parent Type: `TaskCommandPayload`
    nonisolated public struct CancelTask: NoemaAPI.SelectionSet {
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
        TasksCancelTaskMutation.Data.CancelTask.self
      ] }

      /// Authoritative task projection.
      public var task: Task { __data["task"] }
      /// Cursor for the event committed by the command.
      public var eventCursor: String { __data["eventCursor"] }
      /// Echoed caller idempotency key.
      public var clientMutationId: String { __data["clientMutationId"] }

      /// CancelTask.Task
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
          TasksCancelTaskMutation.Data.CancelTask.Task.self,
          TasksCommandTaskFields.self
        ] }

        /// Task identity.
        public var taskId: String { __data["taskId"] }
        /// Full title.
        public var title: String { __data["title"] }
        /// Full description Markdown.
        public var description: String { __data["description"] }
        /// The only task-level state.
        public var stage: Stage { __data["stage"] }
        /// Optimistic revision.
        public var revision: Int { __data["revision"] }
        /// Execution generation fence.
        public var generation: Int { __data["generation"] }
        /// Last update timestamp.
        public var updatedAt: String { __data["updatedAt"] }
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

        public typealias ActiveGate = TasksCommandTaskFields.ActiveGate

        public typealias CurrentRun = TasksCommandTaskFields.CurrentRun
      }
    }
  }
}
