// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct TasksSkipTaskRecurrenceNextMutation: GraphQLMutation {
  public static let operationName: String = "TasksSkipTaskRecurrenceNext"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation TasksSkipTaskRecurrenceNext($input: TaskRecurrenceCommandInput!) { skipTaskRecurrenceNext(input: $input) { __typename eventCursor clientMutationId } }"#
    ))

  public var input: TaskRecurrenceCommandInput

  public init(input: TaskRecurrenceCommandInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("skipTaskRecurrenceNext", SkipTaskRecurrenceNext.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksSkipTaskRecurrenceNextMutation.Data.self
    ] }

    /// Skip the next exact recurring slot.
    public var skipTaskRecurrenceNext: SkipTaskRecurrenceNext { __data["skipTaskRecurrenceNext"] }

    /// SkipTaskRecurrenceNext
    ///
    /// Parent Type: `TaskCommandPayload`
    nonisolated public struct SkipTaskRecurrenceNext: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskCommandPayload }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("eventCursor", String.self),
        .field("clientMutationId", String.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        TasksSkipTaskRecurrenceNextMutation.Data.SkipTaskRecurrenceNext.self
      ] }

      /// Cursor for the event committed by the command.
      public var eventCursor: String { __data["eventCursor"] }
      /// Echoed caller idempotency key.
      public var clientMutationId: String { __data["clientMutationId"] }
    }
  }
}
