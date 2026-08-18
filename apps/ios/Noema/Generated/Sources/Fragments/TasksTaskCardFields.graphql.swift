// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct TasksTaskCardFields: NoemaAPI.SelectionSet, Fragment {
  public static var fragmentDefinition: StaticString {
    #"fragment TasksTaskCardFields on TaskCard { __typename taskId workspace { __typename ...TasksWorkspaceFields } project { __typename ...TasksProjectFields } title descriptionPreview executorAgentId executorBackend cwdOverride effectiveCwd effectiveCwdSource schedule { __typename scheduledFor timeZone missedRunPolicy recurrenceId recurrenceRevision recurrenceScheduledFor } stage { __typename ...TasksStageFields } revision generation createdAt updatedAt completedAt currentRun { __typename ...TasksCurrentRunFields } activeGate { __typename ...TasksGateFields } validActions }"#
  }

  @_spi(Unsafe) public let __data: DataDict
  @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

  @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskCard }
  @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
    .field("__typename", String.self),
    .field("taskId", String.self),
    .field("workspace", Workspace.self),
    .field("project", Project?.self),
    .field("title", String.self),
    .field("descriptionPreview", String.self),
    .field("executorAgentId", String.self),
    .field("executorBackend", String.self),
    .field("cwdOverride", String?.self),
    .field("effectiveCwd", String?.self),
    .field("effectiveCwdSource", String.self),
    .field("schedule", Schedule?.self),
    .field("stage", Stage.self),
    .field("revision", Int.self),
    .field("generation", Int.self),
    .field("createdAt", String.self),
    .field("updatedAt", String.self),
    .field("completedAt", String?.self),
    .field("currentRun", CurrentRun?.self),
    .field("activeGate", ActiveGate?.self),
    .field("validActions", [GraphQLEnum<NoemaAPI.ValidTaskAction>].self),
  ] }
  @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
    TasksTaskCardFields.self
  ] }

  /// Task identity.
  public var taskId: String { __data["taskId"] }
  /// Workspace placement.
  public var workspace: Workspace { __data["workspace"] }
  /// Nullable project placement.
  public var project: Project? { __data["project"] }
  /// Human title.
  public var title: String { __data["title"] }
  /// Bounded description preview.
  public var descriptionPreview: String { __data["descriptionPreview"] }
  /// Assigned executor agent identity.
  public var executorAgentId: String { __data["executorAgentId"] }
  /// Assigned executor backend.
  public var executorBackend: String { __data["executorBackend"] }
  /// Explicit Task directory base override.
  public var cwdOverride: String? { __data["cwdOverride"] }
  /// Derived effective working directory when already frozen or explicitly configured.
  public var effectiveCwd: String? { __data["effectiveCwd"] }
  /// Effective working-directory source: task, project, or default.
  public var effectiveCwdSource: String { __data["effectiveCwdSource"] }
  /// Optional future execution and recurrence provenance.
  public var schedule: Schedule? { __data["schedule"] }
  /// The only task-level state.
  public var stage: Stage { __data["stage"] }
  /// Optimistic revision.
  public var revision: Int { __data["revision"] }
  /// Execution generation fence.
  public var generation: Int { __data["generation"] }
  /// Creation timestamp.
  public var createdAt: String { __data["createdAt"] }
  /// Last update timestamp.
  public var updatedAt: String { __data["updatedAt"] }
  /// Completion timestamp, when any.
  public var completedAt: String? { __data["completedAt"] }
  /// Current run projection.
  public var currentRun: CurrentRun? { __data["currentRun"] }
  /// Open gate projection.
  public var activeGate: ActiveGate? { __data["activeGate"] }
  /// Server-authorized actions.
  public var validActions: [GraphQLEnum<NoemaAPI.ValidTaskAction>] { __data["validActions"] }

  /// Workspace
  ///
  /// Parent Type: `Workspace`
  nonisolated public struct Workspace: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.Workspace }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("__typename", String.self),
      .fragment(TasksWorkspaceFields.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksTaskCardFields.Workspace.self,
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
    /// Membership role for the authenticated owner.
    public var membershipRole: GraphQLEnum<NoemaAPI.WorkspaceMembershipRole> { __data["membershipRole"] }

    public struct Fragments: FragmentContainer {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      public var tasksWorkspaceFields: TasksWorkspaceFields { _toFragment() }
    }
  }

  /// Project
  ///
  /// Parent Type: `Project`
  nonisolated public struct Project: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.Project }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("__typename", String.self),
      .fragment(TasksProjectFields.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksTaskCardFields.Project.self,
      TasksProjectFields.self
    ] }

    /// Opaque project identity.
    public var projectId: String { __data["projectId"] }
    /// Owning workspace.
    public var workspaceId: String { __data["workspaceId"] }
    /// Project name.
    public var name: String { __data["name"] }
    /// Project description.
    public var description: String { __data["description"] }
    /// Optional absolute project working folder.
    public var folder: String? { __data["folder"] }
    /// Optimistic project revision.
    public var revision: Int { __data["revision"] }
    /// Archive timestamp, if archived.
    public var archivedAt: String? { __data["archivedAt"] }
    /// Creation timestamp.
    public var createdAt: String { __data["createdAt"] }
    /// Last update timestamp.
    public var updatedAt: String { __data["updatedAt"] }

    public struct Fragments: FragmentContainer {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      public var tasksProjectFields: TasksProjectFields { _toFragment() }
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
      TasksTaskCardFields.Schedule.self
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
      TasksTaskCardFields.Stage.self,
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
      TasksTaskCardFields.CurrentRun.self,
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
      TasksTaskCardFields.ActiveGate.self,
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
}
