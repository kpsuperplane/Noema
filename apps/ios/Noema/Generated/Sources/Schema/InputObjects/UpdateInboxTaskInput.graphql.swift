// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Inbox task edit input.
nonisolated public struct UpdateInboxTaskInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    taskId: String,
    expectedRevision: Int32,
    expectedGeneration: Int32,
    title: GraphQLNullable<String> = nil,
    description: GraphQLNullable<String> = nil,
    projectId: GraphQLNullable<String> = nil,
    clearProject: GraphQLNullable<Bool> = nil,
    executorAgentId: GraphQLNullable<String> = nil,
    cwdOverride: GraphQLNullable<String> = nil,
    clearCwdOverride: GraphQLNullable<Bool> = nil,
    clientMutationId: String
  ) {
    __data = InputDict([
      "taskId": taskId,
      "expectedRevision": expectedRevision,
      "expectedGeneration": expectedGeneration,
      "title": title,
      "description": description,
      "projectId": projectId,
      "clearProject": clearProject,
      "executorAgentId": executorAgentId,
      "cwdOverride": cwdOverride,
      "clearCwdOverride": clearCwdOverride,
      "clientMutationId": clientMutationId
    ])
  }

  /// Task target.
  public var taskId: String {
    get { __data["taskId"] }
    set { __data["taskId"] = newValue }
  }

  /// Expected current revision.
  public var expectedRevision: Int32 {
    get { __data["expectedRevision"] }
    set { __data["expectedRevision"] = newValue }
  }

  /// Expected execution generation.
  public var expectedGeneration: Int32 {
    get { __data["expectedGeneration"] }
    set { __data["expectedGeneration"] = newValue }
  }

  /// Optional replacement title.
  public var title: GraphQLNullable<String> {
    get { __data["title"] }
    set { __data["title"] = newValue }
  }

  /// Optional replacement description.
  public var description: GraphQLNullable<String> {
    get { __data["description"] }
    set { __data["description"] = newValue }
  }

  /// Optional project assignment. Null means omitted unless clearProject is true.
  public var projectId: GraphQLNullable<String> {
    get { __data["projectId"] }
    set { __data["projectId"] = newValue }
  }

  /// Explicitly clear the project association.
  public var clearProject: GraphQLNullable<Bool> {
    get { __data["clearProject"] }
    set { __data["clearProject"] = newValue }
  }

  /// Optional configured executor agent.
  public var executorAgentId: GraphQLNullable<String> {
    get { __data["executorAgentId"] }
    set { __data["executorAgentId"] = newValue }
  }

  /// Optional replacement absolute task working-directory override.
  public var cwdOverride: GraphQLNullable<String> {
    get { __data["cwdOverride"] }
    set { __data["cwdOverride"] = newValue }
  }

  /// Explicitly clear the task working-directory override.
  public var clearCwdOverride: GraphQLNullable<Bool> {
    get { __data["clearCwdOverride"] }
    set { __data["clearCwdOverride"] = newValue }
  }

  /// Caller idempotency key.
  public var clientMutationId: String {
    get { __data["clientMutationId"] }
    set { __data["clientMutationId"] = newValue }
  }
}
