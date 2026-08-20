// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Capture a task in Inbox.
nonisolated public struct CaptureTaskInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    workspaceId: String,
    projectId: GraphQLNullable<String> = nil,
    title: String,
    taskDocument: String? = nil,
    schedule: GraphQLNullable<NewTaskScheduleInput> = nil,
    executorAgentId: GraphQLNullable<String> = nil,
    cwdOverride: GraphQLNullable<String> = nil,
    clientMutationId: String
  ) {
    __data = InputDict([
      "workspaceId": workspaceId,
      "projectId": projectId,
      "title": title,
      "taskDocument": taskDocument ?? GraphQLNullable.none,
      "schedule": schedule,
      "executorAgentId": executorAgentId,
      "cwdOverride": cwdOverride,
      "clientMutationId": clientMutationId
    ])
  }

  /// Workspace target.
  public var workspaceId: String {
    get { __data["workspaceId"] }
    set { __data["workspaceId"] = newValue }
  }

  /// Optional project association.
  public var projectId: GraphQLNullable<String> {
    get { __data["projectId"] }
    set { __data["projectId"] = newValue }
  }

  /// Concise title.
  public var title: String {
    get { __data["title"] }
    set { __data["title"] = newValue }
  }

  /// Exact initial TASK.md content.
  public var taskDocument: String? {
    get { __data["taskDocument"] }
    set { __data["taskDocument"] = newValue }
  }

  /// Optional one-time or repeating execution schedule.
  public var schedule: GraphQLNullable<NewTaskScheduleInput> {
    get { __data["schedule"] }
    set { __data["schedule"] = newValue }
  }

  /// Optional configured executor agent; omitted uses Noema's built-in executor.
  public var executorAgentId: GraphQLNullable<String> {
    get { __data["executorAgentId"] }
    set { __data["executorAgentId"] = newValue }
  }

  /// Optional absolute Task directory base override.
  public var cwdOverride: GraphQLNullable<String> {
    get { __data["cwdOverride"] }
    set { __data["cwdOverride"] = newValue }
  }

  /// Caller idempotency key.
  public var clientMutationId: String {
    get { __data["clientMutationId"] }
    set { __data["clientMutationId"] = newValue }
  }
}
