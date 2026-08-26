// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct ChatBootQuery: GraphQLQuery {
  public static let operationName: String = "ChatBoot"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"query ChatBoot { localStatus { __typename primaryAgentDisplayName } onboardingStatus { __typename isUserOnboarded steps { __typename id status providerKind providerAccountId accountKey displayName providerAccountStatus authMethod } } providerAccountCatalog { __typename providerKind displayName preferredAuthMethod supportedAuthMethods } providerAccounts { __typename providerAccountId providerKind accountKey displayName authMethod status isActive isDefault lastCheckedAt lastAuthenticatedAt lastErrorCode lastErrorMessage } }"#
    ))

  public init() {}

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.QueryRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("localStatus", LocalStatus.self),
      .field("onboardingStatus", OnboardingStatus.self),
      .field("providerAccountCatalog", [ProviderAccountCatalog].self),
      .field("providerAccounts", [ProviderAccount].self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      ChatBootQuery.Data.self
    ] }

    /// Return local Noema status.
    public var localStatus: LocalStatus { __data["localStatus"] }
    /// Return onboarding status.
    public var onboardingStatus: OnboardingStatus { __data["onboardingStatus"] }
    /// List provider account types that can be added in Settings.
    public var providerAccountCatalog: [ProviderAccountCatalog] { __data["providerAccountCatalog"] }
    /// List provider account metadata safe to show in Settings.
    public var providerAccounts: [ProviderAccount] { __data["providerAccounts"] }

    /// LocalStatus
    ///
    /// Parent Type: `LocalStatus`
    nonisolated public struct LocalStatus: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.LocalStatus }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("primaryAgentDisplayName", String?.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        ChatBootQuery.Data.LocalStatus.self
      ] }

      /// Current primary agent display name, if the agent has been named.
      public var primaryAgentDisplayName: String? { __data["primaryAgentDisplayName"] }
    }

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
        ChatBootQuery.Data.OnboardingStatus.self
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
          .field("providerKind", String.self),
          .field("providerAccountId", String.self),
          .field("accountKey", String.self),
          .field("displayName", String.self),
          .field("providerAccountStatus", GraphQLEnum<NoemaAPI.ProviderAccountStatus>.self),
          .field("authMethod", GraphQLEnum<NoemaAPI.ProviderAuthMethod>.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          ChatBootQuery.Data.OnboardingStatus.Step.self
        ] }

        /// Stable step id.
        public var id: String { __data["id"] }
        /// Step status.
        public var status: GraphQLEnum<NoemaAPI.OnboardingStepStatus> { __data["status"] }
        /// Provider family for this step.
        public var providerKind: String { __data["providerKind"] }
        /// Provider account id for this step.
        public var providerAccountId: String { __data["providerAccountId"] }
        /// Provider-local account key for this step.
        public var accountKey: String { __data["accountKey"] }
        /// Human-readable account name for this step.
        public var displayName: String { __data["displayName"] }
        /// Last known provider account status.
        public var providerAccountStatus: GraphQLEnum<NoemaAPI.ProviderAccountStatus> { __data["providerAccountStatus"] }
        /// Auth method when the step can start auth.
        public var authMethod: GraphQLEnum<NoemaAPI.ProviderAuthMethod> { __data["authMethod"] }
      }
    }

    /// ProviderAccountCatalog
    ///
    /// Parent Type: `ProviderAccountCatalogEntry`
    nonisolated public struct ProviderAccountCatalog: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.ProviderAccountCatalogEntry }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("providerKind", String.self),
        .field("displayName", String.self),
        .field("preferredAuthMethod", GraphQLEnum<NoemaAPI.ProviderAuthMethod>.self),
        .field("supportedAuthMethods", [GraphQLEnum<NoemaAPI.ProviderAuthMethod>].self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        ChatBootQuery.Data.ProviderAccountCatalog.self
      ] }

      public var providerKind: String { __data["providerKind"] }
      public var displayName: String { __data["displayName"] }
      public var preferredAuthMethod: GraphQLEnum<NoemaAPI.ProviderAuthMethod> { __data["preferredAuthMethod"] }
      public var supportedAuthMethods: [GraphQLEnum<NoemaAPI.ProviderAuthMethod>] { __data["supportedAuthMethods"] }
    }

    /// ProviderAccount
    ///
    /// Parent Type: `ProviderAccount`
    nonisolated public struct ProviderAccount: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.ProviderAccount }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("providerAccountId", String.self),
        .field("providerKind", String.self),
        .field("accountKey", String.self),
        .field("displayName", String.self),
        .field("authMethod", String.self),
        .field("status", GraphQLEnum<NoemaAPI.ProviderAccountStatus>.self),
        .field("isActive", Bool.self),
        .field("isDefault", Bool.self),
        .field("lastCheckedAt", String?.self),
        .field("lastAuthenticatedAt", String?.self),
        .field("lastErrorCode", String?.self),
        .field("lastErrorMessage", String?.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        ChatBootQuery.Data.ProviderAccount.self
      ] }

      /// Stable provider account id.
      public var providerAccountId: String { __data["providerAccountId"] }
      /// Provider family, such as `codex`.
      public var providerKind: String { __data["providerKind"] }
      /// Provider-local account key.
      public var accountKey: String { __data["accountKey"] }
      /// Human-readable account name.
      public var displayName: String { __data["displayName"] }
      /// Authentication method used for this account.
      public var authMethod: String { __data["authMethod"] }
      /// Last known account readiness status.
      public var status: GraphQLEnum<NoemaAPI.ProviderAccountStatus> { __data["status"] }
      /// Whether the account may be selected.
      public var isActive: Bool { __data["isActive"] }
      /// Whether the account is the default account for its provider.
      public var isDefault: Bool { __data["isDefault"] }
      /// Last time Noema checked the account status.
      public var lastCheckedAt: String? { __data["lastCheckedAt"] }
      /// Last time Noema observed successful authentication.
      public var lastAuthenticatedAt: String? { __data["lastAuthenticatedAt"] }
      /// Last non-secret provider error code.
      public var lastErrorCode: String? { __data["lastErrorCode"] }
      /// Last non-secret provider error message.
      public var lastErrorMessage: String? { __data["lastErrorMessage"] }
    }
  }
}
