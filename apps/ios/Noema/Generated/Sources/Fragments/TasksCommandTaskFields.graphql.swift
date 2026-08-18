// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct TasksCommandTaskFields: NoemaAPI.SelectionSet, Fragment {
  public static var fragmentDefinition: StaticString {
    #"fragment TasksCommandTaskFields on TaskDetail { __typename taskId title description executorAgentId executorBackend cwdOverride effectiveCwd effectiveCwdSource stage { __typename ...TasksStageFields } revision generation updatedAt schedule { __typename scheduledFor timeZone missedRunPolicy recurrenceId recurrenceRevision recurrenceScheduledFor } completedAt validActions activeGate { __typename ...TasksGateFields } currentRun { __typename ...TasksCurrentRunFields } }"#
  }

  @_spi(Unsafe) public let __data: DataDict
  @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

  @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskDetail }
  @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
    .field("__typename", String.self),
    .field("taskId", String.self),
    .field("title", String.self),
    .field("description", String.self),
    .field("executorAgentId", String.self),
    .field("executorBackend", String.self),
    .field("cwdOverride", String?.self),
    .field("effectiveCwd", String?.self),
    .field("effectiveCwdSource", String.self),
    .field("stage", Stage.self),
    .field("revision", Int.self),
    .field("generation", Int.self),
    .field("updatedAt", String.self),
    .field("schedule", Schedule?.self),
    .field("completedAt", String?.self),
    .field("validActions", [GraphQLEnum<NoemaAPI.ValidTaskAction>].self),
    .field("activeGate", ActiveGate?.self),
    .field("currentRun", CurrentRun?.self),
  ] }
  @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
    TasksCommandTaskFields.self
  ] }

  /// Task identity.
  public var taskId: String { __data["taskId"] }
  /// Full title.
  public var title: String { __data["title"] }
  /// Full description Markdown.
  public var description: String { __data["description"] }
  /// Assigned executor agent identity.
  public var executorAgentId: String { __data["executorAgentId"] }
  /// Assigned executor backend.
  public var executorBackend: String { __data["executorBackend"] }
  /// Explicit Task directory base override.
  public var cwdOverride: String? { __data["cwdOverride"] }
  /// Derived or frozen effective working directory.
  public var effectiveCwd: String? { __data["effectiveCwd"] }
  /// Effective working-directory source: task, project, or default.
  public var effectiveCwdSource: String { __data["effectiveCwdSource"] }
  /// The only task-level state.
  public var stage: Stage { __data["stage"] }
  /// Optimistic revision.
  public var revision: Int { __data["revision"] }
  /// Execution generation fence.
  public var generation: Int { __data["generation"] }
  /// Last update timestamp.
  public var updatedAt: String { __data["updatedAt"] }
  /// Optional future execution and recurrence provenance.
  public var schedule: Schedule? { __data["schedule"] }
  /// Completion timestamp, when any.
  public var completedAt: String? { __data["completedAt"] }
  /// Server-authorized actions.
  public var validActions: [GraphQLEnum<NoemaAPI.ValidTaskAction>] { __data["validActions"] }
  /// Open gate projection.
  public var activeGate: ActiveGate? { __data["activeGate"] }
  /// Current run projection.
  public var currentRun: CurrentRun? { __data["currentRun"] }

  /// Stage
  ///
  /// Parent Type: `WorkflowStage`
  nonisolated public struct Stage: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.WorkflowStage }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("__typename", String.self),
      .fragment(TasksStageFields.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksCommandTaskFields.Stage.self,
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

    public struct Fragments: FragmentContainer {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      public var tasksStageFields: TasksStageFields { _toFragment() }
    }
  }

  /// Schedule
  ///
  /// Parent Type: `TaskSchedule`
  nonisolated public struct Schedule: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskSchedule }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("__typename", String.self),
      .field("scheduledFor", String.self),
      .field("timeZone", String.self),
      .field("missedRunPolicy", GraphQLEnum<NoemaAPI.MissedRunPolicy>.self),
      .field("recurrenceId", String?.self),
      .field("recurrenceRevision", Int?.self),
      .field("recurrenceScheduledFor", String?.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksCommandTaskFields.Schedule.self
    ] }

    /// Exact UTC execution instant.
    public var scheduledFor: String { __data["scheduledFor"] }
    /// Authoring IANA timezone.
    public var timeZone: String { __data["timeZone"] }
    /// Missed-instant behavior.
    public var missedRunPolicy: GraphQLEnum<NoemaAPI.MissedRunPolicy> { __data["missedRunPolicy"] }
    /// Recurring template identity when Repeat is enabled.
    public var recurrenceId: String? { __data["recurrenceId"] }
    /// Recurring template revision snapshotted by this occurrence.
    public var recurrenceRevision: Int? { __data["recurrenceRevision"] }
    /// Exact recurrence slot represented by this task.
    public var recurrenceScheduledFor: String? { __data["recurrenceScheduledFor"] }
  }

  /// ActiveGate
  ///
  /// Parent Type: `TaskGate`
  nonisolated public struct ActiveGate: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskGate }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("__typename", String.self),
      .fragment(TasksGateFields.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksCommandTaskFields.ActiveGate.self,
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

    public struct Fragments: FragmentContainer {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      public var tasksGateFields: TasksGateFields { _toFragment() }
    }
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
      .fragment(TasksCurrentRunFields.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksCommandTaskFields.CurrentRun.self,
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
    /// Queue timestamp.
    public var queuedAt: String { __data["queuedAt"] }
    /// Start timestamp.
    public var startedAt: String? { __data["startedAt"] }
    /// Last update timestamp.
    public var updatedAt: String { __data["updatedAt"] }
    /// Safe activity label.
    public var activityLabel: String { __data["activityLabel"] }

    public struct Fragments: FragmentContainer {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      public var tasksCurrentRunFields: TasksCurrentRunFields { _toFragment() }
    }
  }
}
