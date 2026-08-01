// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct CancelProviderAuthAttemptMutation: GraphQLMutation {
  public static let operationName: String = "CancelProviderAuthAttempt"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation CancelProviderAuthAttempt($input: CancelProviderAuthAttemptInput!) { cancelProviderAuthAttempt(input: $input) { __typename attemptId providerKind providerAccountId method status errorCode errorMessage } }"#
    ))

  public var input: CancelProviderAuthAttemptInput

  public init(input: CancelProviderAuthAttemptInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("cancelProviderAuthAttempt", CancelProviderAuthAttempt?.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      CancelProviderAuthAttemptMutation.Data.self
    ] }

    /// Cancel a pending provider auth attempt.
    public var cancelProviderAuthAttempt: CancelProviderAuthAttempt? { __data["cancelProviderAuthAttempt"] }

    /// CancelProviderAuthAttempt
    ///
    /// Parent Type: `ProviderAuthAttempt`
    nonisolated public struct CancelProviderAuthAttempt: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.ProviderAuthAttempt }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("attemptId", String.self),
        .field("providerKind", String.self),
        .field("providerAccountId", String.self),
        .field("method", GraphQLEnum<NoemaAPI.ProviderAuthMethod>.self),
        .field("status", GraphQLEnum<NoemaAPI.ProviderAuthAttemptStatus>.self),
        .field("errorCode", String?.self),
        .field("errorMessage", String?.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        CancelProviderAuthAttemptMutation.Data.CancelProviderAuthAttempt.self
      ] }

      /// Short-lived auth attempt id.
      public var attemptId: String { __data["attemptId"] }
      /// Provider family.
      public var providerKind: String { __data["providerKind"] }
      /// Provider account id.
      public var providerAccountId: String { __data["providerAccountId"] }
      /// Provider account auth method.
      public var method: GraphQLEnum<NoemaAPI.ProviderAuthMethod> { __data["method"] }
      /// Attempt status.
      public var status: GraphQLEnum<NoemaAPI.ProviderAuthAttemptStatus> { __data["status"] }
      /// Stable non-secret error code.
      public var errorCode: String? { __data["errorCode"] }
      /// Failure message when available.
      public var errorMessage: String? { __data["errorMessage"] }
    }
  }
}
