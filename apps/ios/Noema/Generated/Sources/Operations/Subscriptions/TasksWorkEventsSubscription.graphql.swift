// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct TasksWorkEventsSubscription: GraphQLSubscription {
  public static let operationName: String = "TasksWorkEvents"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"subscription TasksWorkEvents($workspaceId: String!, $after: String) { workEvents(workspaceId: $workspaceId, after: $after) { __typename ...TasksEventFields } }"#,
      fragments: [TasksEventFields.self]
    ))

  public var workspaceId: String
  public var after: GraphQLNullable<String>

  public init(
    workspaceId: String,
    after: GraphQLNullable<String>
  ) {
    self.workspaceId = workspaceId
    self.after = after
  }

  @_spi(Unsafe) public var __variables: Variables? { [
    "workspaceId": workspaceId,
    "after": after
  ] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.SubscriptionRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("workEvents", WorkEvents.self, arguments: [
        "workspaceId": .variable("workspaceId"),
        "after": .variable("after")
      ]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksWorkEventsSubscription.Data.self
    ] }

    /// Replay and stream the workspace-scoped durable Work ledger.
    public var workEvents: WorkEvents { __data["workEvents"] }

    /// WorkEvents
    ///
    /// Parent Type: `WorkEvent`
    nonisolated public struct WorkEvents: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.WorkEvent }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .fragment(TasksEventFields.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        TasksWorkEventsSubscription.Data.WorkEvents.self,
        TasksEventFields.self
      ] }

      /// Opaque global event cursor.
      public var cursor: String { __data["cursor"] }
      /// Event identity.
      public var eventId: String { __data["eventId"] }
      /// Closed dotted event kind.
      public var kind: String { __data["kind"] }
      /// Event timestamp.
      public var occurredAt: String { __data["occurredAt"] }
      /// Workspace linkage.
      public var workspaceId: String { __data["workspaceId"] }
      /// Project linkage.
      public var projectId: String? { __data["projectId"] }
      /// Task linkage.
      public var taskId: String? { __data["taskId"] }
      /// Run linkage.
      public var runId: String? { __data["runId"] }
      /// Safe audit actor.
      public var actor: String { __data["actor"] }
      /// Causation identity.
      public var causationId: String? { __data["causationId"] }
      /// Correlation identity.
      public var correlationId: String { __data["correlationId"] }
      /// Bounded safe JSON payload.
      public var payload: NoemaAPI.JSON { __data["payload"] }

      public struct Fragments: FragmentContainer {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        public var tasksEventFields: TasksEventFields { _toFragment() }
      }
    }
  }
}
