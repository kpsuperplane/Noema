// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct TasksEventFields: NoemaAPI.SelectionSet, Fragment {
  public static var fragmentDefinition: StaticString {
    #"fragment TasksEventFields on TasksEvent { __typename cursor eventId kind occurredAt workspaceId projectId taskId runId actor causationId correlationId payload }"#
  }

  @_spi(Unsafe) public let __data: DataDict
  @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

  @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TasksEvent }
  @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
    .field("__typename", String.self),
    .field("cursor", String.self),
    .field("eventId", String.self),
    .field("kind", String.self),
    .field("occurredAt", String.self),
    .field("workspaceId", String.self),
    .field("projectId", String?.self),
    .field("taskId", String?.self),
    .field("runId", String?.self),
    .field("actor", String.self),
    .field("causationId", String?.self),
    .field("correlationId", String.self),
    .field("payload", NoemaAPI.JSON.self),
  ] }
  @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
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
}
