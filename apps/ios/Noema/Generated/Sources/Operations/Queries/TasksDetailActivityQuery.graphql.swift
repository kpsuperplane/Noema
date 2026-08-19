// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct TasksDetailActivityQuery: GraphQLQuery {
  public static let operationName: String = "TasksDetailActivity"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"query TasksDetailActivity($taskId: String!) { task(taskId: $taskId) { __typename ...TasksDetailActivityFields } }"#,
      fragments: [TasksDetailActivityFields.self, TasksPolicyFields.self, TasksRunFields.self]
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
      TasksDetailActivityQuery.Data.self
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
        .fragment(TasksDetailActivityFields.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        TasksDetailActivityQuery.Data.Task.self,
        TasksDetailActivityFields.self
      ] }

      /// Task identity.
      public var taskId: String { __data["taskId"] }
      /// Every distinct agent instance that contributed to this task.
      public var contributorInstanceNames: [String] { __data["contributorInstanceNames"] }
      /// Bounded recent human messages.
      public var messages: [Message] { __data["messages"] }
      /// Bounded recent task runs.
      public var runs: [Run] { __data["runs"] }

      public struct Fragments: FragmentContainer {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        public var tasksDetailActivityFields: TasksDetailActivityFields { _toFragment() }
      }

      public typealias Message = TasksDetailActivityFields.Message

      public typealias Run = TasksDetailActivityFields.Run
    }
  }
}
