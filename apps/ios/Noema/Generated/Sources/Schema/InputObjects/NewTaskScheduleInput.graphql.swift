// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Future execution attached directly to an Inbox task.
nonisolated public struct NewTaskScheduleInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    scheduledFor: String,
    timeZone: String,
    missedRunPolicy: GraphQLNullable<GraphQLEnum<MissedRunPolicy>> = nil,
    recurrence: GraphQLNullable<NewTaskRecurrenceInput> = nil
  ) {
    __data = InputDict([
      "scheduledFor": scheduledFor,
      "timeZone": timeZone,
      "missedRunPolicy": missedRunPolicy,
      "recurrence": recurrence
    ])
  }

  /// Exact RFC3339 UTC execution instant.
  public var scheduledFor: String {
    get { __data["scheduledFor"] }
    set { __data["scheduledFor"] = newValue }
  }

  /// Validated authoring IANA timezone.
  public var timeZone: String {
    get { __data["timeZone"] }
    set { __data["timeZone"] = newValue }
  }

  /// Restart/missed-window behavior.
  public var missedRunPolicy: GraphQLNullable<GraphQLEnum<MissedRunPolicy>> {
    get { __data["missedRunPolicy"] }
    set { __data["missedRunPolicy"] = newValue }
  }

  /// Present only when Repeat is enabled.
  public var recurrence: GraphQLNullable<NewTaskRecurrenceInput> {
    get { __data["recurrence"] }
    set { __data["recurrence"] = newValue }
  }
}
