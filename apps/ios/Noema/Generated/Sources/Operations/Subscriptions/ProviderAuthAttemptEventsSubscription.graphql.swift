// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct ProviderAuthAttemptEventsSubscription: GraphQLSubscription {
  public static let operationName: String = "ProviderAuthAttemptEvents"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"subscription ProviderAuthAttemptEvents($attemptId: String!) { providerAuthAttemptEvents(attemptId: $attemptId) { __typename ...NativeProviderAuthAttemptFields } }"#,
      fragments: [NativeProviderAuthAttemptFields.self]
    ))

  public var attemptId: String

  public init(attemptId: String) {
    self.attemptId = attemptId
  }

  @_spi(Unsafe) public var __variables: Variables? { ["attemptId": attemptId] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.SubscriptionRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("providerAuthAttemptEvents", ProviderAuthAttemptEvents.self, arguments: ["attemptId": .variable("attemptId")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      ProviderAuthAttemptEventsSubscription.Data.self
    ] }

    /// Stream pushed updates for one provider authentication attempt.
    public var providerAuthAttemptEvents: ProviderAuthAttemptEvents { __data["providerAuthAttemptEvents"] }

    /// ProviderAuthAttemptEvents
    ///
    /// Parent Type: `ProviderAuthAttempt`
    nonisolated public struct ProviderAuthAttemptEvents: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.ProviderAuthAttempt }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .fragment(NativeProviderAuthAttemptFields.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        ProviderAuthAttemptEventsSubscription.Data.ProviderAuthAttemptEvents.self,
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
