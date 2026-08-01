// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Existing project update input.
nonisolated public struct UpdateProjectInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    projectId: String,
    expectedRevision: Int32,
    name: GraphQLNullable<String> = nil,
    description: GraphQLNullable<String> = nil,
    clientMutationId: String
  ) {
    __data = InputDict([
      "projectId": projectId,
      "expectedRevision": expectedRevision,
      "name": name,
      "description": description,
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

  /// Optional replacement name.
  public var name: GraphQLNullable<String> {
    get { __data["name"] }
    set { __data["name"] = newValue }
  }

  /// Optional replacement description.
  public var description: GraphQLNullable<String> {
    get { __data["description"] }
    set { __data["description"] = newValue }
  }

  /// Caller idempotency key.
  public var clientMutationId: String {
    get { __data["clientMutationId"] }
    set { __data["clientMutationId"] = newValue }
  }
}
