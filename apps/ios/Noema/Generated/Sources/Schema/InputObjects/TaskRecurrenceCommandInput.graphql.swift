// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Fenced recurring lifecycle command.
nonisolated public struct TaskRecurrenceCommandInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    recurrenceId: String,
    expectedRevision: Int32,
    clientMutationId: String
  ) {
    __data = InputDict([
      "recurrenceId": recurrenceId,
      "expectedRevision": expectedRevision,
      "clientMutationId": clientMutationId
    ])
  }

  /// Recurring template target.
  public var recurrenceId: String {
    get { __data["recurrenceId"] }
    set { __data["recurrenceId"] = newValue }
  }

  /// Expected template revision.
  public var expectedRevision: Int32 {
    get { __data["expectedRevision"] }
    set { __data["expectedRevision"] = newValue }
  }

  /// Caller idempotency key.
  public var clientMutationId: String {
    get { __data["clientMutationId"] }
    set { __data["clientMutationId"] = newValue }
  }
}
