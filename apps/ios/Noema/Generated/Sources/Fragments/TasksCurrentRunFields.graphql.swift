// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct TasksCurrentRunFields: NoemaAPI.SelectionSet, Fragment {
  public static var fragmentDefinition: StaticString {
    #"fragment TasksCurrentRunFields on CurrentRunSummary { __typename runId instanceName kind status attemptIndex contractId queuedAt startedAt updatedAt activityLabel }"#
  }

  @_spi(Unsafe) public let __data: DataDict
  @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

  @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.CurrentRunSummary }
  @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
    .field("__typename", String.self),
    .field("runId", String.self),
    .field("instanceName", String.self),
    .field("kind", GraphQLEnum<NoemaAPI.TaskRunKind>.self),
    .field("status", GraphQLEnum<NoemaAPI.TaskRunStatus>.self),
    .field("attemptIndex", Int.self),
    .field("contractId", String?.self),
    .field("queuedAt", String.self),
    .field("startedAt", String?.self),
    .field("updatedAt", String.self),
    .field("activityLabel", String.self),
  ] }
  @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
    TasksCurrentRunFields.self
  ] }

  /// Run identity.
  public var runId: String { __data["runId"] }
  /// Human-friendly instance identity.
  public var instanceName: String { __data["instanceName"] }
  /// Planner, Executor, or Reviewer.
  public var kind: GraphQLEnum<NoemaAPI.TaskRunKind> { __data["kind"] }
  /// Run-local queue/lease status.
  public var status: GraphQLEnum<NoemaAPI.TaskRunStatus> { __data["status"] }
  /// Lineage attempt.
  public var attemptIndex: Int { __data["attemptIndex"] }
  /// Contract identity, absent for Planner.
  public var contractId: String? { __data["contractId"] }
  /// Queue timestamp.
  public var queuedAt: String { __data["queuedAt"] }
  /// Start timestamp.
  public var startedAt: String? { __data["startedAt"] }
  /// Last update timestamp.
  public var updatedAt: String { __data["updatedAt"] }
  /// Safe activity label.
  public var activityLabel: String { __data["activityLabel"] }
}
