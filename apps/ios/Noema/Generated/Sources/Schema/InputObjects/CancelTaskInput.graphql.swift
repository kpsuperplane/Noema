// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Cancel a task.
nonisolated public struct CancelTaskInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    taskId: String,
    expectedRevision: Int32,
    expectedGeneration: Int32,
    reason: GraphQLNullable<String> = nil,
    clientMutationId: String
  ) {
    __data = InputDict([
      "taskId": taskId,
      "expectedRevision": expectedRevision,
      "expectedGeneration": expectedGeneration,
      "reason": reason,
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

  /// Optional safe cancellation reason.
  public var reason: GraphQLNullable<String> {
    get { __data["reason"] }
    set { __data["reason"] = newValue }
  }

  /// Caller idempotency key.
  public var clientMutationId: String {
    get { __data["clientMutationId"] }
    set { __data["clientMutationId"] = newValue }
  }
}
