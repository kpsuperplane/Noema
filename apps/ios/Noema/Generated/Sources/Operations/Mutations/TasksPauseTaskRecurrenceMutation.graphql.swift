// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct TasksPauseTaskRecurrenceMutation: GraphQLMutation {
  public static let operationName: String = "TasksPauseTaskRecurrence"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation TasksPauseTaskRecurrence($input: TaskRecurrenceCommandInput!) { pauseTaskRecurrence(input: $input) { __typename eventCursor clientMutationId } }"#
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
      .field("pauseTaskRecurrence", PauseTaskRecurrence.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksPauseTaskRecurrenceMutation.Data.self
    ] }

    /// Pause a recurring task.
    public var pauseTaskRecurrence: PauseTaskRecurrence { __data["pauseTaskRecurrence"] }

    /// PauseTaskRecurrence
    ///
    /// Parent Type: `TaskCommandPayload`
    nonisolated public struct PauseTaskRecurrence: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskCommandPayload }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("eventCursor", String.self),
        .field("clientMutationId", String.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        TasksPauseTaskRecurrenceMutation.Data.PauseTaskRecurrence.self
      ] }

      /// Cursor for the event committed by the command.
      public var eventCursor: String { __data["eventCursor"] }
      /// Echoed caller idempotency key.
      public var clientMutationId: String { __data["clientMutationId"] }
    }
  }
}
