// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct TasksTaskWorkspaceFileQuery: GraphQLQuery {
  public static let operationName: String = "TasksTaskWorkspaceFile"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"query TasksTaskWorkspaceFile($taskId: String!, $path: String!) { taskWorkspaceFile(taskId: $taskId, path: $path) { __typename path content } }"#
    ))

  public var taskId: String
  public var path: String

  public init(
    taskId: String,
    path: String
  ) {
    self.taskId = taskId
    self.path = path
  }

  @_spi(Unsafe) public var __variables: Variables? { [
    "taskId": taskId,
    "path": path
  ] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.QueryRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("taskWorkspaceFile", TaskWorkspaceFile.self, arguments: [
        "taskId": .variable("taskId"),
        "path": .variable("path")
      ]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksTaskWorkspaceFileQuery.Data.self
    ] }

    /// Return one bounded UTF-8 file from an owner-authorized Task workspace.
    public var taskWorkspaceFile: TaskWorkspaceFile { __data["taskWorkspaceFile"] }

    /// TaskWorkspaceFile
    ///
    /// Parent Type: `TaskWorkspaceFileText`
    nonisolated public struct TaskWorkspaceFile: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskWorkspaceFileText }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("path", String.self),
        .field("content", String.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        TasksTaskWorkspaceFileQuery.Data.TaskWorkspaceFile.self
      ] }

      /// Path relative to the Task working directory.
      public var path: String { __data["path"] }
      /// Exact UTF-8 text.
      public var content: String { __data["content"] }
    }
  }
}
