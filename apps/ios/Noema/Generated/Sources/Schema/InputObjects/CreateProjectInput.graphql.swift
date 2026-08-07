// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// New project input.
nonisolated public struct CreateProjectInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    workspaceId: String,
    name: String,
    description: String? = nil,
    folder: GraphQLNullable<String> = nil,
    clientMutationId: String
  ) {
    __data = InputDict([
      "workspaceId": workspaceId,
      "name": name,
      "description": description ?? GraphQLNullable.none,
      "folder": folder,
      "clientMutationId": clientMutationId
    ])
  }

  /// Workspace owning the project.
  public var workspaceId: String {
    get { __data["workspaceId"] }
    set { __data["workspaceId"] = newValue }
  }

  /// Nonblank project name.
  public var name: String {
    get { __data["name"] }
    set { __data["name"] = newValue }
  }

  /// Descriptive project text.
  public var description: String? {
    get { __data["description"] }
    set { __data["description"] = newValue }
  }

  /// Optional absolute project working folder.
  public var folder: GraphQLNullable<String> {
    get { __data["folder"] }
    set { __data["folder"] = newValue }
  }

  /// Caller idempotency key.
  public var clientMutationId: String {
    get { __data["clientMutationId"] }
    set { __data["clientMutationId"] = newValue }
  }
}
