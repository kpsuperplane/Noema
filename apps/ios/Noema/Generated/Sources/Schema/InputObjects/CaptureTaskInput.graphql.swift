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
    description: String? = nil,
    clientMutationId: String
  ) {
    __data = InputDict([
      "workspaceId": workspaceId,
      "projectId": projectId,
      "title": title,
      "description": description ?? GraphQLNullable.none,
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

  /// Fuller capture description.
  public var description: String? {
    get { __data["description"] }
    set { __data["description"] = newValue }
  }

  /// Caller idempotency key.
  public var clientMutationId: String {
    get { __data["clientMutationId"] }
    set { __data["clientMutationId"] = newValue }
  }
}
