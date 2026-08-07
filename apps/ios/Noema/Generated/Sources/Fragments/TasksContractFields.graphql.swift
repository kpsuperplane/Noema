// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct TasksContractFields: NoemaAPI.SelectionSet, Fragment {
  public static var fragmentDefinition: StaticString {
    #"fragment TasksContractFields on TaskExecutionContract { __typename requestMarkdown criteria { __typename criterionId ordinal description expectedEvidence } complexity executionPolicy { __typename ...TasksPolicyFields } executorAgentId executorBackend effectiveCwd }"#
  }

  @_spi(Unsafe) public let __data: DataDict
  @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

  @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskExecutionContract }
  @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
    .field("__typename", String.self),
    .field("requestMarkdown", String.self),
    .field("criteria", [Criterium].self),
    .field("complexity", GraphQLEnum<NoemaAPI.TaskComplexity>.self),
    .field("executionPolicy", ExecutionPolicy.self),
    .field("executorAgentId", String.self),
    .field("executorBackend", String.self),
    .field("effectiveCwd", String?.self),
  ] }
  @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
    TasksContractFields.self
  ] }

  /// Immutable request Markdown.
  public var requestMarkdown: String { __data["requestMarkdown"] }
  /// Exact criteria.
  public var criteria: [Criterium] { __data["criteria"] }
  /// Complexity tier.
  public var complexity: GraphQLEnum<NoemaAPI.TaskComplexity> { __data["complexity"] }
  /// Execution policy snapshot.
  public var executionPolicy: ExecutionPolicy { __data["executionPolicy"] }
  /// Assigned executor agent identity.
  public var executorAgentId: String { __data["executorAgentId"] }
  /// Executor backend.
  public var executorBackend: String { __data["executorBackend"] }
  /// Frozen effective working directory.
  public var effectiveCwd: String? { __data["effectiveCwd"] }

  /// Criterium
  ///
  /// Parent Type: `TaskValidationCriterion`
  nonisolated public struct Criterium: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskValidationCriterion }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("__typename", String.self),
      .field("criterionId", String.self),
      .field("ordinal", Int.self),
      .field("description", String.self),
      .field("expectedEvidence", String?.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksContractFields.Criterium.self
    ] }

    /// Criterion identity.
    public var criterionId: String { __data["criterionId"] }
    /// One-based order.
    public var ordinal: Int { __data["ordinal"] }
    /// Criterion description.
    public var description: String { __data["description"] }
    /// Optional expected evidence.
    public var expectedEvidence: String? { __data["expectedEvidence"] }
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
      TasksContractFields.ExecutionPolicy.self,
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
