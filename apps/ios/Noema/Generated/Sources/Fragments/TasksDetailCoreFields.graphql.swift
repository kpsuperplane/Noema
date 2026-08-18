// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct TasksDetailCoreFields: NoemaAPI.SelectionSet, Fragment {
  public static var fragmentDefinition: StaticString {
    #"fragment TasksDetailCoreFields on TaskDetail { __typename ...TasksCommandTaskFields project { __typename ...TasksProjectFields } createdAt source { __typename conversationId } }"#
  }

  @_spi(Unsafe) public let __data: DataDict
  @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

  @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskDetail }
  @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
    .field("__typename", String.self),
    .field("project", Project?.self),
    .field("createdAt", String.self),
    .field("source", Source.self),
    .fragment(TasksCommandTaskFields.self),
  ] }
  @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
    TasksDetailCoreFields.self,
    TasksCommandTaskFields.self
  ] }

  /// Nullable project placement.
  public var project: Project? { __data["project"] }
  /// Creation timestamp.
  public var createdAt: String { __data["createdAt"] }
  /// Safe provenance.
  public var source: Source { __data["source"] }
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

  public struct Fragments: FragmentContainer {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    public var tasksCommandTaskFields: TasksCommandTaskFields { _toFragment() }
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
      TasksDetailCoreFields.Project.self,
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

  /// Source
  ///
  /// Parent Type: `TaskSource`
  nonisolated public struct Source: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskSource }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("__typename", String.self),
      .field("conversationId", String?.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksDetailCoreFields.Source.self
    ] }

    /// Source conversation.
    public var conversationId: String? { __data["conversationId"] }
  }

  public typealias Stage = TasksCommandTaskFields.Stage

  public typealias Schedule = TasksCommandTaskFields.Schedule

  public typealias ActiveGate = TasksCommandTaskFields.ActiveGate

  public typealias CurrentRun = TasksCommandTaskFields.CurrentRun
}
