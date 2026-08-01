// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct TasksGateFields: NoemaAPI.SelectionSet, Fragment {
  public static var fragmentDefinition: StaticString {
    #"fragment TasksGateFields on TaskGate { __typename gateId taskGeneration kind state recoveryReason retryRunKind prompt contextMarkdown suggestedAnswers openedBy originatingRunId openedAt resolvedBy resolvedAt resolution }"#
  }

  @_spi(Unsafe) public let __data: DataDict
  @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

  @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskGate }
  @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
    .field("__typename", String.self),
    .field("gateId", String.self),
    .field("taskGeneration", Int.self),
    .field("kind", GraphQLEnum<NoemaAPI.TaskGateKind>.self),
    .field("state", GraphQLEnum<NoemaAPI.TaskGateState>.self),
    .field("recoveryReason", GraphQLEnum<NoemaAPI.TaskRecoveryReason>?.self),
    .field("retryRunKind", GraphQLEnum<NoemaAPI.TaskRunKind>?.self),
    .field("prompt", String.self),
    .field("contextMarkdown", String.self),
    .field("suggestedAnswers", [String].self),
    .field("openedBy", String.self),
    .field("originatingRunId", String?.self),
    .field("openedAt", String.self),
    .field("resolvedBy", String?.self),
    .field("resolvedAt", String?.self),
    .field("resolution", String?.self),
  ] }
  @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
    TasksGateFields.self
  ] }

  /// Gate identity.
  public var gateId: String { __data["gateId"] }
  /// Task generation.
  public var taskGeneration: Int { __data["taskGeneration"] }
  /// Gate kind.
  public var kind: GraphQLEnum<NoemaAPI.TaskGateKind> { __data["kind"] }
  /// Gate state.
  public var state: GraphQLEnum<NoemaAPI.TaskGateState> { __data["state"] }
  /// Recovery reason, when any.
  public var recoveryReason: GraphQLEnum<NoemaAPI.TaskRecoveryReason>? { __data["recoveryReason"] }
  /// Explicit recovery continuation role, when any.
  public var retryRunKind: GraphQLEnum<NoemaAPI.TaskRunKind>? { __data["retryRunKind"] }
  /// Human prompt.
  public var prompt: String { __data["prompt"] }
  /// Bounded context.
  public var contextMarkdown: String { __data["contextMarkdown"] }
  /// Optional direct answers.
  public var suggestedAnswers: [String] { __data["suggestedAnswers"] }
  /// Opener actor.
  public var openedBy: String { __data["openedBy"] }
  /// Opening run, when any.
  public var originatingRunId: String? { __data["originatingRunId"] }
  /// Open timestamp.
  public var openedAt: String { __data["openedAt"] }
  /// Resolver actor, when resolved.
  public var resolvedBy: String? { __data["resolvedBy"] }
  /// Resolution timestamp, when resolved.
  public var resolvedAt: String? { __data["resolvedAt"] }
  /// Resolution message identity, when resolved.
  public var resolution: String? { __data["resolution"] }
}
