// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Optional Repeat configuration for scheduled work.
nonisolated public struct NewTaskRecurrenceInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    startsAt: String,
    cronExpression: String,
    overlapPolicy: GraphQLNullable<GraphQLEnum<OverlapPolicy>> = nil
  ) {
    __data = InputDict([
      "startsAt": startsAt,
      "cronExpression": cronExpression,
      "overlapPolicy": overlapPolicy
    ])
  }

  /// Inclusive RFC3339 lower bound for cron matches.
  public var startsAt: String {
    get { __data["startsAt"] }
    set { __data["startsAt"] = newValue }
  }

  /// Five-field cron expression.
  public var cronExpression: String {
    get { __data["cronExpression"] }
    set { __data["cronExpression"] = newValue }
  }

  /// Behavior while another occurrence is nonterminal.
  public var overlapPolicy: GraphQLNullable<GraphQLEnum<OverlapPolicy>> {
    get { __data["overlapPolicy"] }
    set { __data["overlapPolicy"] = newValue }
  }
}
