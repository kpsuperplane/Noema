// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct TasksWorkspaceFields: NoemaAPI.SelectionSet, Fragment {
  public static var fragmentDefinition: StaticString {
    #"fragment TasksWorkspaceFields on Workspace { __typename workspaceId name description isPersonal }"#
  }

  @_spi(Unsafe) public let __data: DataDict
  @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

  @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.Workspace }
  @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
    .field("__typename", String.self),
    .field("workspaceId", String.self),
    .field("name", String.self),
    .field("description", String.self),
    .field("isPersonal", Bool.self),
  ] }
  @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
    TasksWorkspaceFields.self
  ] }

  /// Opaque workspace identity.
  public var workspaceId: String { __data["workspaceId"] }
  /// Human-readable name.
  public var name: String { __data["name"] }
  /// Descriptive text.
  public var description: String { __data["description"] }
  /// Whether this is the Personal workspace.
  public var isPersonal: Bool { __data["isPersonal"] }
}
