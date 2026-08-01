// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct ProviderAccountsQuery: GraphQLQuery {
  public static let operationName: String = "ProviderAccounts"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"query ProviderAccounts { providerAccountCatalog { __typename providerKind displayName preferredAuthMethod supportedAuthMethods capabilities { __typename capabilityId status reliabilityContract dataFlowClass features { __typename citations directUrlFetch jsRendering authenticatedContext resultPersistence } } } providerAccounts { __typename providerAccountId providerKind accountKey displayName authMethod status isActive isDefault lastCheckedAt lastAuthenticatedAt lastErrorCode lastErrorMessage capabilities { __typename capabilityId status reliabilityContract dataFlowClass features { __typename citations directUrlFetch jsRendering authenticatedContext resultPersistence } } } }"#
    ))

  public init() {}

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.QueryRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("providerAccountCatalog", [ProviderAccountCatalog].self),
      .field("providerAccounts", [ProviderAccount].self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      ProviderAccountsQuery.Data.self
    ] }

    /// List provider account types that can be added in Settings.
    public var providerAccountCatalog: [ProviderAccountCatalog] { __data["providerAccountCatalog"] }
    /// List provider account metadata safe to show in Settings.
    public var providerAccounts: [ProviderAccount] { __data["providerAccounts"] }

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
        .field("capabilities", [Capability].self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        ProviderAccountsQuery.Data.ProviderAccountCatalog.self
      ] }

      public var providerKind: String { __data["providerKind"] }
      public var displayName: String { __data["displayName"] }
      public var preferredAuthMethod: GraphQLEnum<NoemaAPI.ProviderAuthMethod> { __data["preferredAuthMethod"] }
      public var supportedAuthMethods: [GraphQLEnum<NoemaAPI.ProviderAuthMethod>] { __data["supportedAuthMethods"] }
      public var capabilities: [Capability] { __data["capabilities"] }

      /// ProviderAccountCatalog.Capability
      ///
      /// Parent Type: `ProviderCapability`
      nonisolated public struct Capability: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.ProviderCapability }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("capabilityId", String.self),
          .field("status", String.self),
          .field("reliabilityContract", String.self),
          .field("dataFlowClass", String.self),
          .field("features", Features.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          ProviderAccountsQuery.Data.ProviderAccountCatalog.Capability.self
        ] }

        public var capabilityId: String { __data["capabilityId"] }
        public var status: String { __data["status"] }
        public var reliabilityContract: String { __data["reliabilityContract"] }
        public var dataFlowClass: String { __data["dataFlowClass"] }
        public var features: Features { __data["features"] }

        /// ProviderAccountCatalog.Capability.Features
        ///
        /// Parent Type: `CapabilityFeatures`
        nonisolated public struct Features: NoemaAPI.SelectionSet {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.CapabilityFeatures }
          @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .field("citations", Bool.self),
            .field("directUrlFetch", Bool.self),
            .field("jsRendering", Bool.self),
            .field("authenticatedContext", Bool.self),
            .field("resultPersistence", String.self),
          ] }
          @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
            ProviderAccountsQuery.Data.ProviderAccountCatalog.Capability.Features.self
          ] }

          public var citations: Bool { __data["citations"] }
          public var directUrlFetch: Bool { __data["directUrlFetch"] }
          public var jsRendering: Bool { __data["jsRendering"] }
          public var authenticatedContext: Bool { __data["authenticatedContext"] }
          public var resultPersistence: String { __data["resultPersistence"] }
        }
      }
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
        .field("capabilities", [Capability].self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        ProviderAccountsQuery.Data.ProviderAccount.self
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
      /// Provider capabilities available through this account.
      public var capabilities: [Capability] { __data["capabilities"] }

      /// ProviderAccount.Capability
      ///
      /// Parent Type: `ProviderCapability`
      nonisolated public struct Capability: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.ProviderCapability }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("capabilityId", String.self),
          .field("status", String.self),
          .field("reliabilityContract", String.self),
          .field("dataFlowClass", String.self),
          .field("features", Features.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          ProviderAccountsQuery.Data.ProviderAccount.Capability.self
        ] }

        public var capabilityId: String { __data["capabilityId"] }
        public var status: String { __data["status"] }
        public var reliabilityContract: String { __data["reliabilityContract"] }
        public var dataFlowClass: String { __data["dataFlowClass"] }
        public var features: Features { __data["features"] }

        /// ProviderAccount.Capability.Features
        ///
        /// Parent Type: `CapabilityFeatures`
        nonisolated public struct Features: NoemaAPI.SelectionSet {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.CapabilityFeatures }
          @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .field("citations", Bool.self),
            .field("directUrlFetch", Bool.self),
            .field("jsRendering", Bool.self),
            .field("authenticatedContext", Bool.self),
            .field("resultPersistence", String.self),
          ] }
          @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
            ProviderAccountsQuery.Data.ProviderAccount.Capability.Features.self
          ] }

          public var citations: Bool { __data["citations"] }
          public var directUrlFetch: Bool { __data["directUrlFetch"] }
          public var jsRendering: Bool { __data["jsRendering"] }
          public var authenticatedContext: Bool { __data["authenticatedContext"] }
          public var resultPersistence: String { __data["resultPersistence"] }
        }
      }
    }
  }
}
