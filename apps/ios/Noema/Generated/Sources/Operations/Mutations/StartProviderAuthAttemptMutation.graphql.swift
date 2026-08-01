// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct StartProviderAuthAttemptMutation: GraphQLMutation {
  public static let operationName: String = "StartProviderAuthAttempt"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation StartProviderAuthAttempt($input: StartProviderAuthAttemptInput!) { startProviderAuthAttempt(input: $input) { __typename ...NativeProviderAuthAttemptFields } }"#,
      fragments: [NativeProviderAuthAttemptFields.self]
    ))

  public var input: StartProviderAuthAttemptInput

  public init(input: StartProviderAuthAttemptInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("startProviderAuthAttempt", StartProviderAuthAttempt.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      StartProviderAuthAttemptMutation.Data.self
    ] }

    /// Start a provider auth attempt.
    public var startProviderAuthAttempt: StartProviderAuthAttempt { __data["startProviderAuthAttempt"] }

    /// StartProviderAuthAttempt
    ///
    /// Parent Type: `ProviderAuthAttempt`
    nonisolated public struct StartProviderAuthAttempt: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.ProviderAuthAttempt }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .fragment(NativeProviderAuthAttemptFields.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        StartProviderAuthAttemptMutation.Data.StartProviderAuthAttempt.self,
        NativeProviderAuthAttemptFields.self
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
      /// Verification URL when available.
      public var verificationUrl: String? { __data["verificationUrl"] }
      /// User code when available.
      public var userCode: String? { __data["userCode"] }
      /// Static UI-safe instruction text.
      public var instructions: String? { __data["instructions"] }
      /// Stable non-secret error code.
      public var errorCode: String? { __data["errorCode"] }
      /// Failure message when available.
      public var errorMessage: String? { __data["errorMessage"] }

      public struct Fragments: FragmentContainer {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        public var nativeProviderAuthAttemptFields: NativeProviderAuthAttemptFields { _toFragment() }
      }
    }
  }
}
