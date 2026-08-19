// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct TasksDetailActivityFields: NoemaAPI.SelectionSet, Fragment {
  public static var fragmentDefinition: StaticString {
    #"fragment TasksDetailActivityFields on TaskDetail { __typename taskId contributorInstanceNames messages { __typename messageId bodyMarkdown author createdAt } runs { __typename ...TasksRunFields } }"#
  }

  @_spi(Unsafe) public let __data: DataDict
  @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

  @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskDetail }
  @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
    .field("__typename", String.self),
    .field("taskId", String.self),
    .field("contributorInstanceNames", [String].self),
    .field("messages", [Message].self),
    .field("runs", [Run].self),
  ] }
  @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
    TasksDetailActivityFields.self
  ] }

  /// Task identity.
  public var taskId: String { __data["taskId"] }
  /// Every distinct agent instance that contributed to this task.
  public var contributorInstanceNames: [String] { __data["contributorInstanceNames"] }
  /// Bounded recent human messages.
  public var messages: [Message] { __data["messages"] }
  /// Bounded recent task runs.
  public var runs: [Run] { __data["runs"] }

  /// Message
  ///
  /// Parent Type: `TaskMessage`
  nonisolated public struct Message: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskMessage }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("__typename", String.self),
      .field("messageId", String.self),
      .field("bodyMarkdown", String.self),
      .field("author", String.self),
      .field("createdAt", String.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksDetailActivityFields.Message.self
    ] }

    /// Message identity.
    public var messageId: String { __data["messageId"] }
    /// Safe Markdown body.
    public var bodyMarkdown: String { __data["bodyMarkdown"] }
    /// Author actor.
    public var author: String { __data["author"] }
    /// Creation timestamp.
    public var createdAt: String { __data["createdAt"] }
  }

  /// Run
  ///
  /// Parent Type: `TaskRun`
  nonisolated public struct Run: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskRun }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("__typename", String.self),
      .fragment(TasksRunFields.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksDetailActivityFields.Run.self,
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

    public struct Fragments: FragmentContainer {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      public var tasksRunFields: TasksRunFields { _toFragment() }
    }

    public typealias Model = TasksRunFields.Model

    public typealias ExecutionPolicy = TasksRunFields.ExecutionPolicy
  }
}
