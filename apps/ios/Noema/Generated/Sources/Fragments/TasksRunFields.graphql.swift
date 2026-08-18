// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct TasksRunFields: NoemaAPI.SelectionSet, Fragment {
  public static var fragmentDefinition: StaticString {
    #"fragment TasksRunFields on TaskRun { __typename runId instanceName kind status agentId taskGeneration attemptIndex reviewRound parentRunId model { __typename providerKind providerAccountId providerInstanceKey selectionMode modelProfile reasoningEffort selectionSource } actualProviderKind actualModelProfile executionPolicy { __typename ...TasksPolicyFields } errorCode errorMessage providerCallCount toolCallCount inputTokens cachedInputTokens outputTokens activeMilliseconds queuedAt startedAt endedAt createdAt updatedAt }"#
  }

  @_spi(Unsafe) public let __data: DataDict
  @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

  @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskRun }
  @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
    .field("__typename", String.self),
    .field("runId", String.self),
    .field("instanceName", String.self),
    .field("kind", GraphQLEnum<NoemaAPI.TaskRunKind>.self),
    .field("status", GraphQLEnum<NoemaAPI.TaskRunStatus>.self),
    .field("agentId", String.self),
    .field("taskGeneration", Int.self),
    .field("attemptIndex", Int.self),
    .field("reviewRound", Int.self),
    .field("parentRunId", String?.self),
    .field("model", Model.self),
    .field("actualProviderKind", String?.self),
    .field("actualModelProfile", String?.self),
    .field("executionPolicy", ExecutionPolicy.self),
    .field("errorCode", String?.self),
    .field("errorMessage", String?.self),
    .field("providerCallCount", Int.self),
    .field("toolCallCount", Int.self),
    .field("inputTokens", Int.self),
    .field("cachedInputTokens", Int.self),
    .field("outputTokens", Int.self),
    .field("activeMilliseconds", Int.self),
    .field("queuedAt", String.self),
    .field("startedAt", String?.self),
    .field("endedAt", String?.self),
    .field("createdAt", String.self),
    .field("updatedAt", String.self),
  ] }
  @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
    TasksRunFields.self
  ] }

  /// Run identity.
  public var runId: String { __data["runId"] }
  /// Human-friendly instance identity.
  public var instanceName: String { __data["instanceName"] }
  /// Run role.
  public var kind: GraphQLEnum<NoemaAPI.TaskRunKind> { __data["kind"] }
  /// Run-local status.
  public var status: GraphQLEnum<NoemaAPI.TaskRunStatus> { __data["status"] }
  /// Agent identity.
  public var agentId: String { __data["agentId"] }
  /// Task generation.
  public var taskGeneration: Int { __data["taskGeneration"] }
  /// Attempt index.
  public var attemptIndex: Int { __data["attemptIndex"] }
  /// Review round.
  public var reviewRound: Int { __data["reviewRound"] }
  /// Parent run, when any.
  public var parentRunId: String? { __data["parentRunId"] }
  /// Requested model snapshot.
  public var model: Model { __data["model"] }
  /// Safe actual provider family.
  public var actualProviderKind: String? { __data["actualProviderKind"] }
  /// Safe actual model profile.
  public var actualModelProfile: String? { __data["actualModelProfile"] }
  /// Immutable policy snapshot.
  public var executionPolicy: ExecutionPolicy { __data["executionPolicy"] }
  /// Safe terminal error code.
  public var errorCode: String? { __data["errorCode"] }
  /// Safe terminal error message.
  public var errorMessage: String? { __data["errorMessage"] }
  /// Completed provider calls.
  public var providerCallCount: Int { __data["providerCallCount"] }
  /// Dispatched tool calls.
  public var toolCallCount: Int { __data["toolCallCount"] }
  /// Cumulative input tokens.
  public var inputTokens: Int { __data["inputTokens"] }
  /// Cumulative cached-input tokens.
  public var cachedInputTokens: Int { __data["cachedInputTokens"] }
  /// Cumulative output tokens.
  public var outputTokens: Int { __data["outputTokens"] }
  /// Active execution duration in milliseconds.
  public var activeMilliseconds: Int { __data["activeMilliseconds"] }
  /// Queue timestamp.
  public var queuedAt: String { __data["queuedAt"] }
  /// Start timestamp.
  public var startedAt: String? { __data["startedAt"] }
  /// End timestamp.
  public var endedAt: String? { __data["endedAt"] }
  /// Creation timestamp.
  public var createdAt: String { __data["createdAt"] }
  /// Last update timestamp.
  public var updatedAt: String { __data["updatedAt"] }

  /// Model
  ///
  /// Parent Type: `TaskModelSnapshot`
  nonisolated public struct Model: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskModelSnapshot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("__typename", String.self),
      .field("providerKind", String.self),
      .field("providerAccountId", String.self),
      .field("providerInstanceKey", String?.self),
      .field("selectionMode", GraphQLEnum<NoemaAPI.ProviderSelectionMode>.self),
      .field("modelProfile", String?.self),
      .field("reasoningEffort", GraphQLEnum<NoemaAPI.ReasoningEffort>?.self),
      .field("selectionSource", String?.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksRunFields.Model.self
    ] }

    /// Provider family.
    public var providerKind: String { __data["providerKind"] }
    /// Provider account identity.
    public var providerAccountId: String { __data["providerAccountId"] }
    /// Exact process/provider instance identity.
    public var providerInstanceKey: String? { __data["providerInstanceKey"] }
    /// Selection mode.
    public var selectionMode: GraphQLEnum<NoemaAPI.ProviderSelectionMode> { __data["selectionMode"] }
    /// Explicit model profile, when used.
    public var modelProfile: String? { __data["modelProfile"] }
    /// Reasoning effort, when selected.
    public var reasoningEffort: GraphQLEnum<NoemaAPI.ReasoningEffort>? { __data["reasoningEffort"] }
    /// Selection provenance.
    public var selectionSource: String? { __data["selectionSource"] }
  }

  /// ExecutionPolicy
  ///
  /// Parent Type: `TaskExecutionPolicy`
  nonisolated public struct ExecutionPolicy: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskExecutionPolicy }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("__typename", String.self),
      .fragment(TasksPolicyFields.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksRunFields.ExecutionPolicy.self,
      TasksPolicyFields.self
    ] }

    /// Provider continuation bound.
    public var maxProviderContinuations: Int { __data["maxProviderContinuations"] }
    /// Tool-call bound.
    public var maxToolCalls: Int { __data["maxToolCalls"] }
    /// Active-minute bound.
    public var maxActiveMinutes: Int { __data["maxActiveMinutes"] }
    /// Progress-audit interval.
    public var progressAuditInterval: Int { __data["progressAuditInterval"] }
    /// Automatic retry bound.
    public var maxAutomaticRetries: Int { __data["maxAutomaticRetries"] }
    /// Review-round bound.
    public var maxReviewRounds: Int { __data["maxReviewRounds"] }

    public struct Fragments: FragmentContainer {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      public var tasksPolicyFields: TasksPolicyFields { _toFragment() }
    }
  }
}
