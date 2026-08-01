// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Project reopen input.
nonisolated public struct ReopenProjectInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    projectId: String,
    expectedRevision: Int32,
    clientMutationId: String
  ) {
    __data = InputDict([
      "projectId": projectId,
      "expectedRevision": expectedRevision,
      "clientMutationId": clientMutationId
    ])
  }

  /// Project target.
  public var projectId: String {
    get { __data["projectId"] }
    set { __data["projectId"] = newValue }
  }

  /// Expected project revision.
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
