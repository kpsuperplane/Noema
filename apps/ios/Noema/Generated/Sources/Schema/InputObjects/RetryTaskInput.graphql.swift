// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Retry an eligible Recovery gate.
nonisolated public struct RetryTaskInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    taskId: String,
    gateId: String,
    expectedRevision: Int32,
    expectedGeneration: Int32,
    retryNote: GraphQLNullable<String> = nil,
    clientMutationId: String
  ) {
    __data = InputDict([
      "taskId": taskId,
      "gateId": gateId,
      "expectedRevision": expectedRevision,
      "expectedGeneration": expectedGeneration,
      "retryNote": retryNote,
      "clientMutationId": clientMutationId
    ])
  }

  /// Task target.
  public var taskId: String {
    get { __data["taskId"] }
    set { __data["taskId"] = newValue }
  }

  /// Current open gate.
  public var gateId: String {
    get { __data["gateId"] }
    set { __data["gateId"] = newValue }
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

  /// Optional bounded retry note.
  public var retryNote: GraphQLNullable<String> {
    get { __data["retryNote"] }
    set { __data["retryNote"] = newValue }
  }

  /// Caller idempotency key.
  public var clientMutationId: String {
    get { __data["clientMutationId"] }
    set { __data["clientMutationId"] = newValue }
  }
}
