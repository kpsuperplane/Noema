// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct TasksEndTaskRecurrenceMutation: GraphQLMutation {
  public static let operationName: String = "TasksEndTaskRecurrence"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation TasksEndTaskRecurrence($input: TaskRecurrenceCommandInput!) { endTaskRecurrence(input: $input) { __typename eventCursor clientMutationId } }"#
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
      .field("endTaskRecurrence", EndTaskRecurrence.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksEndTaskRecurrenceMutation.Data.self
    ] }

    /// End future recurrence without changing active occurrences.
    public var endTaskRecurrence: EndTaskRecurrence { __data["endTaskRecurrence"] }

    /// EndTaskRecurrence
    ///
    /// Parent Type: `TaskCommandPayload`
    nonisolated public struct EndTaskRecurrence: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskCommandPayload }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("eventCursor", String.self),
        .field("clientMutationId", String.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        TasksEndTaskRecurrenceMutation.Data.EndTaskRecurrence.self
      ] }

      /// Cursor for the event committed by the command.
      public var eventCursor: String { __data["eventCursor"] }
      /// Echoed caller idempotency key.
      public var clientMutationId: String { __data["clientMutationId"] }
    }
  }
}
