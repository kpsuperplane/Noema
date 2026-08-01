// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Board/list task filter input.
nonisolated public struct WorkTasksInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    workspaceId: String,
    projectId: GraphQLNullable<String> = nil,
    stageIds: GraphQLNullable<[String]> = nil,
    stageBehaviors: GraphQLNullable<[GraphQLEnum<WorkflowStageBehavior>]> = nil,
    text: GraphQLNullable<String> = nil,
    attentionOnly: Bool? = nil,
    scope: GraphQLEnum<WorkTaskScope>? = nil
  ) {
    __data = InputDict([
      "workspaceId": workspaceId,
      "projectId": projectId,
      "stageIds": stageIds,
      "stageBehaviors": stageBehaviors,
      "text": text,
      "attentionOnly": attentionOnly ?? GraphQLNullable.none,
      "scope": scope ?? GraphQLNullable.none
    ])
  }

  /// Workspace scope.
  public var workspaceId: String {
    get { __data["workspaceId"] }
    set { __data["workspaceId"] = newValue }
  }

  /// Optional project scope.
  public var projectId: GraphQLNullable<String> {
    get { __data["projectId"] }
    set { __data["projectId"] = newValue }
  }

  /// Optional explicit stage identities.
  public var stageIds: GraphQLNullable<[String]> {
    get { __data["stageIds"] }
    set { __data["stageIds"] = newValue }
  }

  /// Optional semantic stage behavior filter.
  public var stageBehaviors: GraphQLNullable<[GraphQLEnum<WorkflowStageBehavior>]> {
    get { __data["stageBehaviors"] }
    set { __data["stageBehaviors"] = newValue }
  }

  /// Case-insensitive title/description search.
  public var text: GraphQLNullable<String> {
    get { __data["text"] }
    set { __data["text"] = newValue }
  }

  /// Restrict results to unresolved human attention.
  public var attentionOnly: Bool? {
    get { __data["attentionOnly"] }
    set { __data["attentionOnly"] = newValue }
  }

  /// Active, terminal, or combined scope.
  public var scope: GraphQLEnum<WorkTaskScope>? {
    get { __data["scope"] }
    set { __data["scope"] = newValue }
  }
}
