// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Remove future execution from an Inbox task.
nonisolated public struct UnscheduleTaskInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    taskId: String,
    expectedRevision: Int32,
    expectedGeneration: Int32,
    clientMutationId: String
  ) {
    __data = InputDict([
      "taskId": taskId,
      "expectedRevision": expectedRevision,
      "expectedGeneration": expectedGeneration,
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

  /// Caller idempotency key.
  public var clientMutationId: String {
    get { __data["clientMutationId"] }
    set { __data["clientMutationId"] = newValue }
  }
}
