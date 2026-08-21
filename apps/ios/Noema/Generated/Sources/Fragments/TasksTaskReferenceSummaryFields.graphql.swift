// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct TasksTaskReferenceSummaryFields: NoemaAPI.SelectionSet, Fragment {
  public static var fragmentDefinition: StaticString {
    #"fragment TasksTaskReferenceSummaryFields on TaskSummary { __typename taskId title stage { __typename stageId name behavior } completedAt currentRun { __typename runId kind activityLabel } attention { __typename title } }"#
  }

  @_spi(Unsafe) public let __data: DataDict
  @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

  @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskSummary }
  @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
    .field("__typename", String.self),
    .field("taskId", String.self),
    .field("title", String.self),
    .field("stage", Stage.self),
    .field("completedAt", String?.self),
    .field("currentRun", CurrentRun?.self),
    .field("attention", Attention?.self),
  ] }
  @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
    TasksTaskReferenceSummaryFields.self
  ] }

  /// Task identity.
  public var taskId: String { __data["taskId"] }
  /// Human title.
  public var title: String { __data["title"] }
  /// The only task-level state.
  public var stage: Stage { __data["stage"] }
  /// Completion timestamp, when any.
  public var completedAt: String? { __data["completedAt"] }
  /// Current run projection.
  public var currentRun: CurrentRun? { __data["currentRun"] }
  /// Derived attention.
  public var attention: Attention? { __data["attention"] }

  /// Stage
  ///
  /// Parent Type: `WorkflowStage`
  nonisolated public struct Stage: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.WorkflowStage }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("__typename", String.self),
      .field("stageId", String.self),
      .field("name", String.self),
      .field("behavior", GraphQLEnum<NoemaAPI.WorkflowStageBehavior>.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksTaskReferenceSummaryFields.Stage.self
    ] }

    /// Opaque stage identity.
    public var stageId: String { __data["stageId"] }
    /// Human-facing stage name.
    public var name: String { __data["name"] }
    /// Closed runtime behavior.
    public var behavior: GraphQLEnum<NoemaAPI.WorkflowStageBehavior> { __data["behavior"] }
  }

  /// CurrentRun
  ///
  /// Parent Type: `CurrentRunSummary`
  nonisolated public struct CurrentRun: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.CurrentRunSummary }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("__typename", String.self),
      .field("runId", String.self),
      .field("kind", GraphQLEnum<NoemaAPI.TaskRunKind>.self),
      .field("activityLabel", String.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksTaskReferenceSummaryFields.CurrentRun.self
    ] }

    /// Run identity.
    public var runId: String { __data["runId"] }
    /// Planner, Executor, or Reviewer.
    public var kind: GraphQLEnum<NoemaAPI.TaskRunKind> { __data["kind"] }
    /// Safe activity label.
    public var activityLabel: String { __data["activityLabel"] }
  }

  /// Attention
  ///
  /// Parent Type: `TaskAttention`
  nonisolated public struct Attention: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskAttention }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("__typename", String.self),
      .field("title", String.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksTaskReferenceSummaryFields.Attention.self
    ] }

    /// Stable UI title.
    public var title: String { __data["title"] }
  }
}
