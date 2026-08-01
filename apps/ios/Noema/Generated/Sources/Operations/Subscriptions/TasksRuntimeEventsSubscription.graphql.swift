// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct TasksRuntimeEventsSubscription: GraphQLSubscription {
  public static let operationName: String = "TasksRuntimeEvents"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"subscription TasksRuntimeEvents($taskId: String!) { taskRuntimeEvents(taskId: $taskId) { __typename taskId runId } }"#
    ))

  public var taskId: String

  public init(taskId: String) {
    self.taskId = taskId
  }

  @_spi(Unsafe) public var __variables: Variables? { ["taskId": taskId] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.SubscriptionRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("taskRuntimeEvents", TaskRuntimeEvents.self, arguments: ["taskId": .variable("taskId")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksRuntimeEventsSubscription.Data.self
    ] }

    /// Wake subscribers when live task-run transcript items are persisted.
    public var taskRuntimeEvents: TaskRuntimeEvents { __data["taskRuntimeEvents"] }

    /// TaskRuntimeEvents
    ///
    /// Parent Type: `GraphqlTaskRuntimeEvent`
    nonisolated public struct TaskRuntimeEvents: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.GraphqlTaskRuntimeEvent }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("taskId", String.self),
        .field("runId", String?.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        TasksRuntimeEventsSubscription.Data.TaskRuntimeEvents.self
      ] }

      public var taskId: String { __data["taskId"] }
      public var runId: String? { __data["runId"] }
    }
  }
}
