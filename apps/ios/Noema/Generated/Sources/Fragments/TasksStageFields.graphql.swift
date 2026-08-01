// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct TasksStageFields: NoemaAPI.SelectionSet, Fragment {
  public static var fragmentDefinition: StaticString {
    #"fragment TasksStageFields on WorkflowStage { __typename stageId workflowId key name displayOrder behavior }"#
  }

  @_spi(Unsafe) public let __data: DataDict
  @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

  @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.WorkflowStage }
  @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
    .field("__typename", String.self),
    .field("stageId", String.self),
    .field("workflowId", String.self),
    .field("key", String.self),
    .field("name", String.self),
    .field("displayOrder", Int.self),
    .field("behavior", GraphQLEnum<NoemaAPI.WorkflowStageBehavior>.self),
  ] }
  @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
    TasksStageFields.self
  ] }

  /// Opaque stage identity.
  public var stageId: String { __data["stageId"] }
  /// Owning workflow.
  public var workflowId: String { __data["workflowId"] }
  /// Machine-stable key.
  public var key: String { __data["key"] }
  /// Human-facing stage name.
  public var name: String { __data["name"] }
  /// Display ordering.
  public var displayOrder: Int { __data["displayOrder"] }
  /// Closed runtime behavior.
  public var behavior: GraphQLEnum<NoemaAPI.WorkflowStageBehavior> { __data["behavior"] }
}
