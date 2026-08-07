// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct TasksSchedulePreviewQuery: GraphQLQuery {
  public static let operationName: String = "TasksSchedulePreview"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"query TasksSchedulePreview($input: TaskSchedulePreviewInput!) { taskSchedulePreview(input: $input) { __typename resolvedStart occurrences } }"#
    ))

  public var input: TaskSchedulePreviewInput

  public init(input: TaskSchedulePreviewInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.QueryRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("taskSchedulePreview", TaskSchedulePreview.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksSchedulePreviewQuery.Data.self
    ] }

    /// Preview resolved future schedule instants.
    public var taskSchedulePreview: TaskSchedulePreview { __data["taskSchedulePreview"] }

    /// TaskSchedulePreview
    ///
    /// Parent Type: `TaskSchedulePreview`
    nonisolated public struct TaskSchedulePreview: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskSchedulePreview }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("resolvedStart", String.self),
        .field("occurrences", [String].self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        TasksSchedulePreviewQuery.Data.TaskSchedulePreview.self
      ] }

      /// Resolved inclusive start instant.
      public var resolvedStart: String { __data["resolvedStart"] }
      /// Up to five future UTC instants.
      public var occurrences: [String] { __data["occurrences"] }
    }
  }
}
