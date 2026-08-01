// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct TasksProjectFields: NoemaAPI.SelectionSet, Fragment {
  public static var fragmentDefinition: StaticString {
    #"fragment TasksProjectFields on Project { __typename projectId workspaceId name description revision archivedAt createdAt updatedAt }"#
  }

  @_spi(Unsafe) public let __data: DataDict
  @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

  @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.Project }
  @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
    .field("__typename", String.self),
    .field("projectId", String.self),
    .field("workspaceId", String.self),
    .field("name", String.self),
    .field("description", String.self),
    .field("revision", Int.self),
    .field("archivedAt", String?.self),
    .field("createdAt", String.self),
    .field("updatedAt", String.self),
  ] }
  @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
    TasksProjectFields.self
  ] }

  /// Opaque project identity.
  public var projectId: String { __data["projectId"] }
  /// Owning workspace.
  public var workspaceId: String { __data["workspaceId"] }
  /// Project name.
  public var name: String { __data["name"] }
  /// Project description.
  public var description: String { __data["description"] }
  /// Optimistic project revision.
  public var revision: Int { __data["revision"] }
  /// Archive timestamp, if archived.
  public var archivedAt: String? { __data["archivedAt"] }
  /// Creation timestamp.
  public var createdAt: String { __data["createdAt"] }
  /// Last update timestamp.
  public var updatedAt: String { __data["updatedAt"] }
}
