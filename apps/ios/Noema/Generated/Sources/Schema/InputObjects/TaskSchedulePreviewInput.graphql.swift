// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Preview a local schedule.
nonisolated public struct TaskSchedulePreviewInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    startsAt: String,
    timeZone: String,
    cronExpression: GraphQLNullable<String> = nil
  ) {
    __data = InputDict([
      "startsAt": startsAt,
      "timeZone": timeZone,
      "cronExpression": cronExpression
    ])
  }

  /// Inclusive RFC3339 start instant.
  public var startsAt: String {
    get { __data["startsAt"] }
    set { __data["startsAt"] = newValue }
  }

  /// IANA timezone.
  public var timeZone: String {
    get { __data["timeZone"] }
    set { __data["timeZone"] = newValue }
  }

  /// Optional five-field cron expression; absent previews only the start.
  public var cronExpression: GraphQLNullable<String> {
    get { __data["cronExpression"] }
    set { __data["cronExpression"] = newValue }
  }
}
