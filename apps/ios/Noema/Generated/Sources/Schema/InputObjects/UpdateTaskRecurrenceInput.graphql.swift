// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Edit future authority for a recurring task.
nonisolated public struct UpdateTaskRecurrenceInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    recurrenceId: String,
    expectedRevision: Int32,
    title: GraphQLNullable<String> = nil,
    description: GraphQLNullable<String> = nil,
    projectId: GraphQLNullable<String> = nil,
    clearProject: GraphQLNullable<Bool> = nil,
    startsAt: GraphQLNullable<String> = nil,
    cronExpression: GraphQLNullable<String> = nil,
    timeZone: GraphQLNullable<String> = nil,
    missedRunPolicy: GraphQLNullable<GraphQLEnum<MissedRunPolicy>> = nil,
    overlapPolicy: GraphQLNullable<GraphQLEnum<OverlapPolicy>> = nil,
    clientMutationId: String
  ) {
    __data = InputDict([
      "recurrenceId": recurrenceId,
      "expectedRevision": expectedRevision,
      "title": title,
      "description": description,
      "projectId": projectId,
      "clearProject": clearProject,
      "startsAt": startsAt,
      "cronExpression": cronExpression,
      "timeZone": timeZone,
      "missedRunPolicy": missedRunPolicy,
      "overlapPolicy": overlapPolicy,
      "clientMutationId": clientMutationId
    ])
  }

  /// Recurring template target.
  public var recurrenceId: String {
    get { __data["recurrenceId"] }
    set { __data["recurrenceId"] = newValue }
  }

  /// Expected template revision.
  public var expectedRevision: Int32 {
    get { __data["expectedRevision"] }
    set { __data["expectedRevision"] = newValue }
  }

  /// Optional future title snapshot.
  public var title: GraphQLNullable<String> {
    get { __data["title"] }
    set { __data["title"] = newValue }
  }

  /// Optional future description snapshot.
  public var description: GraphQLNullable<String> {
    get { __data["description"] }
    set { __data["description"] = newValue }
  }

  /// Optional project assignment.
  public var projectId: GraphQLNullable<String> {
    get { __data["projectId"] }
    set { __data["projectId"] = newValue }
  }

  /// Explicitly clear project assignment.
  public var clearProject: GraphQLNullable<Bool> {
    get { __data["clearProject"] }
    set { __data["clearProject"] = newValue }
  }

  /// Optional inclusive RFC3339 start bound.
  public var startsAt: GraphQLNullable<String> {
    get { __data["startsAt"] }
    set { __data["startsAt"] = newValue }
  }

  /// Optional five-field cron expression.
  public var cronExpression: GraphQLNullable<String> {
    get { __data["cronExpression"] }
    set { __data["cronExpression"] = newValue }
  }

  /// Optional IANA timezone.
  public var timeZone: GraphQLNullable<String> {
    get { __data["timeZone"] }
    set { __data["timeZone"] = newValue }
  }

  /// Optional missed-window behavior.
  public var missedRunPolicy: GraphQLNullable<GraphQLEnum<MissedRunPolicy>> {
    get { __data["missedRunPolicy"] }
    set { __data["missedRunPolicy"] = newValue }
  }

  /// Optional overlap behavior.
  public var overlapPolicy: GraphQLNullable<GraphQLEnum<OverlapPolicy>> {
    get { __data["overlapPolicy"] }
    set { __data["overlapPolicy"] = newValue }
  }

  /// Caller idempotency key.
  public var clientMutationId: String {
    get { __data["clientMutationId"] }
    set { __data["clientMutationId"] = newValue }
  }
}
