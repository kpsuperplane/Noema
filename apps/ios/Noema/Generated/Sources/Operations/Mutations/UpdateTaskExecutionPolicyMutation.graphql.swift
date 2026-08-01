// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct UpdateTaskExecutionPolicyMutation: GraphQLMutation {
  public static let operationName: String = "UpdateTaskExecutionPolicy"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation UpdateTaskExecutionPolicy($input: TaskExecutionPolicyInput!) { updateTaskExecutionPolicy(input: $input) { __typename maxProviderContinuations maxToolCalls maxActiveMinutes progressAuditInterval maxAutomaticRetries maxReviewRounds } }"#
    ))

  public var input: TaskExecutionPolicyInput

  public init(input: TaskExecutionPolicyInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("updateTaskExecutionPolicy", UpdateTaskExecutionPolicy.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      UpdateTaskExecutionPolicyMutation.Data.self
    ] }

    /// Replace user-controlled task safety limits while retaining Work-owned bounds.
    public var updateTaskExecutionPolicy: UpdateTaskExecutionPolicy { __data["updateTaskExecutionPolicy"] }

    /// UpdateTaskExecutionPolicy
    ///
    /// Parent Type: `TaskExecutionPolicy`
    nonisolated public struct UpdateTaskExecutionPolicy: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskExecutionPolicy }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("maxProviderContinuations", Int.self),
        .field("maxToolCalls", Int.self),
        .field("maxActiveMinutes", Int.self),
        .field("progressAuditInterval", Int.self),
        .field("maxAutomaticRetries", Int.self),
        .field("maxReviewRounds", Int.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        UpdateTaskExecutionPolicyMutation.Data.UpdateTaskExecutionPolicy.self
      ] }

      /// Provider continuation bound.
      public var maxProviderContinuations: Int { __data["maxProviderContinuations"] }
      /// Tool-call bound.
      public var maxToolCalls: Int { __data["maxToolCalls"] }
      /// Active-minute bound.
      public var maxActiveMinutes: Int { __data["maxActiveMinutes"] }
      /// Progress-audit interval.
      public var progressAuditInterval: Int { __data["progressAuditInterval"] }
      /// Automatic retry bound.
      public var maxAutomaticRetries: Int { __data["maxAutomaticRetries"] }
      /// Review-round bound.
      public var maxReviewRounds: Int { __data["maxReviewRounds"] }
    }
  }
}
