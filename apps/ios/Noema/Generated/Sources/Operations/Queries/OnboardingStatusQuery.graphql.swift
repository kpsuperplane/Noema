// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct OnboardingStatusQuery: GraphQLQuery {
  public static let operationName: String = "OnboardingStatus"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"query OnboardingStatus { onboardingStatus { __typename isUserOnboarded steps { __typename id status providerKind providerAccountId accountKey displayName providerAccountStatus authMethod } } }"#
    ))

  public init() {}

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.QueryRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("onboardingStatus", OnboardingStatus.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      OnboardingStatusQuery.Data.self
    ] }

    /// Return onboarding status.
    public var onboardingStatus: OnboardingStatus { __data["onboardingStatus"] }

    /// OnboardingStatus
    ///
    /// Parent Type: `OnboardingStatus`
    nonisolated public struct OnboardingStatus: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.OnboardingStatus }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("isUserOnboarded", Bool.self),
        .field("steps", [Step].self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        OnboardingStatusQuery.Data.OnboardingStatus.self
      ] }

      /// Whether chat can start.
      public var isUserOnboarded: Bool { __data["isUserOnboarded"] }
      /// Ordered onboarding steps.
      public var steps: [Step] { __data["steps"] }

      /// OnboardingStatus.Step
      ///
      /// Parent Type: `OnboardingStep`
      nonisolated public struct Step: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.OnboardingStep }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("id", String.self),
          .field("status", GraphQLEnum<NoemaAPI.OnboardingStepStatus>.self),
          .field("providerKind", String?.self),
          .field("providerAccountId", String?.self),
          .field("accountKey", String?.self),
          .field("displayName", String?.self),
          .field("providerAccountStatus", GraphQLEnum<NoemaAPI.ProviderAccountStatus>?.self),
          .field("authMethod", GraphQLEnum<NoemaAPI.ProviderAuthMethod>?.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          OnboardingStatusQuery.Data.OnboardingStatus.Step.self
        ] }

        /// Stable step id.
        public var id: String { __data["id"] }
        /// Step status.
        public var status: GraphQLEnum<NoemaAPI.OnboardingStepStatus> { __data["status"] }
        /// Provider family when the step is provider-backed.
        public var providerKind: String? { __data["providerKind"] }
        /// Provider account id when the step is provider-backed.
        public var providerAccountId: String? { __data["providerAccountId"] }
        /// Provider-local account key when the step is provider-backed.
        public var accountKey: String? { __data["accountKey"] }
        /// Human-readable account name when the step is provider-backed.
        public var displayName: String? { __data["displayName"] }
        /// Last known provider account status.
        public var providerAccountStatus: GraphQLEnum<NoemaAPI.ProviderAccountStatus>? { __data["providerAccountStatus"] }
        /// Auth method when the step can start auth.
        public var authMethod: GraphQLEnum<NoemaAPI.ProviderAuthMethod>? { __data["authMethod"] }
      }
    }
  }
}
