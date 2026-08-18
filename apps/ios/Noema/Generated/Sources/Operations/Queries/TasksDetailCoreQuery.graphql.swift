// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct TasksDetailCoreQuery: GraphQLQuery {
  public static let operationName: String = "TasksDetailCore"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"query TasksDetailCore($taskId: String!) { task(taskId: $taskId) { __typename ...TasksDetailCoreFields } }"#,
      fragments: [TasksCommandTaskFields.self, TasksCurrentRunFields.self, TasksDetailCoreFields.self, TasksGateFields.self, TasksProjectFields.self, TasksStageFields.self]
    ))

  public var taskId: String

  public init(taskId: String) {
    self.taskId = taskId
  }

  @_spi(Unsafe) public var __variables: Variables? { ["taskId": taskId] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.QueryRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("task", Task.self, arguments: ["taskId": .variable("taskId")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksDetailCoreQuery.Data.self
    ] }

    /// Return one owner-authorized task detail.
    public var task: Task { __data["task"] }

    /// Task
    ///
    /// Parent Type: `TaskDetail`
    nonisolated public struct Task: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskDetail }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .fragment(TasksDetailCoreFields.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        TasksDetailCoreQuery.Data.Task.self,
        TasksDetailCoreFields.self,
        TasksCommandTaskFields.self
      ] }

      /// Nullable project placement.
      public var project: Project? { __data["project"] }
      /// Creation timestamp.
      public var createdAt: String { __data["createdAt"] }
      /// Safe provenance.
      public var source: Source { __data["source"] }
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

        public var tasksDetailCoreFields: TasksDetailCoreFields { _toFragment() }
        public var tasksCommandTaskFields: TasksCommandTaskFields { _toFragment() }
      }

      public typealias Project = TasksDetailCoreFields.Project

      public typealias Source = TasksDetailCoreFields.Source

      public typealias Stage = TasksCommandTaskFields.Stage

      public typealias Schedule = TasksCommandTaskFields.Schedule

      public typealias ActiveGate = TasksCommandTaskFields.ActiveGate

      public typealias CurrentRun = TasksCommandTaskFields.CurrentRun
    }
  }
}
