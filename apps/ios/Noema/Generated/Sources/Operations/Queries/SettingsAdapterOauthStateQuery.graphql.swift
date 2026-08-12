// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct SettingsAdapterOauthStateQuery: GraphQLQuery {
  public static let operationName: String = "SettingsAdapterOauthState"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"query SettingsAdapterOauthState { adapterOauthState { __typename profiles { __typename profileDigest profileId displayName grantAudience credentialSetup { __typename ...AdapterCredentialSetupFields } } applications { __typename applicationId profileDigest providerDisplayName callbackMode clientId projectLabel revision status grantCount accountCount } accounts { __typename accountId profileDigest accountLabel revision grantIds } grants { __typename grantId applicationId accountId accountLabel providerDisplayName audience desiredScopes grantedScopes authorityRevision tokenRevision status connectionIds } } }"#,
      fragments: [AdapterCredentialSetupFields.self]
    ))

  public init() {}

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.QueryRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("adapterOauthState", AdapterOauthState.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      SettingsAdapterOauthStateQuery.Data.self
    ] }

    /// Return the reusable OAuth application and account hierarchy.
    public var adapterOauthState: AdapterOauthState { __data["adapterOauthState"] }

    /// AdapterOauthState
    ///
    /// Parent Type: `AdapterOauthState`
    nonisolated public struct AdapterOauthState: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AdapterOauthState }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("profiles", [Profile].self),
        .field("applications", [Application].self),
        .field("accounts", [Account].self),
        .field("grants", [Grant].self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SettingsAdapterOauthStateQuery.Data.AdapterOauthState.self
      ] }

      public var profiles: [Profile] { __data["profiles"] }
      public var applications: [Application] { __data["applications"] }
      public var accounts: [Account] { __data["accounts"] }
      public var grants: [Grant] { __data["grants"] }

      /// AdapterOauthState.Profile
      ///
      /// Parent Type: `AdapterOauthProfile`
      nonisolated public struct Profile: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AdapterOauthProfile }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("profileDigest", String.self),
          .field("profileId", String.self),
          .field("displayName", String.self),
          .field("grantAudience", String.self),
          .field("credentialSetup", CredentialSetup?.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          SettingsAdapterOauthStateQuery.Data.AdapterOauthState.Profile.self
        ] }

        public var profileDigest: String { __data["profileDigest"] }
        public var profileId: String { __data["profileId"] }
        public var displayName: String { __data["displayName"] }
        public var grantAudience: String { __data["grantAudience"] }
        public var credentialSetup: CredentialSetup? { __data["credentialSetup"] }

        /// AdapterOauthState.Profile.CredentialSetup
        ///
        /// Parent Type: `AdapterCredentialSetup`
        nonisolated public struct CredentialSetup: NoemaAPI.SelectionSet {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AdapterCredentialSetup }
          @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .fragment(AdapterCredentialSetupFields.self),
          ] }
          @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
            SettingsAdapterOauthStateQuery.Data.AdapterOauthState.Profile.CredentialSetup.self,
            AdapterCredentialSetupFields.self
          ] }

          public var credentialType: String { __data["credentialType"] }
          public var setupUrl: String { __data["setupUrl"] }
          public var instructions: [String] { __data["instructions"] }
          public var inputKind: String { __data["inputKind"] }
          public var fields: [Field] { __data["fields"] }
          public var documentMediaType: String? { __data["documentMediaType"] }
          public var redirectUri: String? { __data["redirectUri"] }
          public var normalizationTransform: NormalizationTransform? { __data["normalizationTransform"] }
          public var requestAuthTransform: RequestAuthTransform? { __data["requestAuthTransform"] }

          public struct Fragments: FragmentContainer {
            @_spi(Unsafe) public let __data: DataDict
            @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

            public var adapterCredentialSetupFields: AdapterCredentialSetupFields { _toFragment() }
          }

          public typealias Field = AdapterCredentialSetupFields.Field

          public typealias NormalizationTransform = AdapterCredentialSetupFields.NormalizationTransform

          public typealias RequestAuthTransform = AdapterCredentialSetupFields.RequestAuthTransform
        }
      }

      /// AdapterOauthState.Application
      ///
      /// Parent Type: `AdapterOauthApplication`
      nonisolated public struct Application: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AdapterOauthApplication }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("applicationId", String.self),
          .field("profileDigest", String.self),
          .field("providerDisplayName", String.self),
          .field("callbackMode", String.self),
          .field("clientId", String.self),
          .field("projectLabel", String?.self),
          .field("revision", Int.self),
          .field("status", String.self),
          .field("grantCount", Int.self),
          .field("accountCount", Int.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          SettingsAdapterOauthStateQuery.Data.AdapterOauthState.Application.self
        ] }

        public var applicationId: String { __data["applicationId"] }
        public var profileDigest: String { __data["profileDigest"] }
        public var providerDisplayName: String { __data["providerDisplayName"] }
        public var callbackMode: String { __data["callbackMode"] }
        public var clientId: String { __data["clientId"] }
        public var projectLabel: String? { __data["projectLabel"] }
        public var revision: Int { __data["revision"] }
        public var status: String { __data["status"] }
        public var grantCount: Int { __data["grantCount"] }
        public var accountCount: Int { __data["accountCount"] }
      }

      /// AdapterOauthState.Account
      ///
      /// Parent Type: `AdapterExternalAccount`
      nonisolated public struct Account: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AdapterExternalAccount }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("accountId", String.self),
          .field("profileDigest", String.self),
          .field("accountLabel", String?.self),
          .field("revision", Int.self),
          .field("grantIds", [String].self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          SettingsAdapterOauthStateQuery.Data.AdapterOauthState.Account.self
        ] }

        public var accountId: String { __data["accountId"] }
        public var profileDigest: String { __data["profileDigest"] }
        public var accountLabel: String? { __data["accountLabel"] }
        public var revision: Int { __data["revision"] }
        public var grantIds: [String] { __data["grantIds"] }
      }

      /// AdapterOauthState.Grant
      ///
      /// Parent Type: `AdapterAuthorizationGrant`
      nonisolated public struct Grant: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AdapterAuthorizationGrant }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("grantId", String.self),
          .field("applicationId", String.self),
          .field("accountId", String?.self),
          .field("accountLabel", String?.self),
          .field("providerDisplayName", String.self),
          .field("audience", String.self),
          .field("desiredScopes", [String].self),
          .field("grantedScopes", [String].self),
          .field("authorityRevision", Int.self),
          .field("tokenRevision", Int.self),
          .field("status", String.self),
          .field("connectionIds", [String].self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          SettingsAdapterOauthStateQuery.Data.AdapterOauthState.Grant.self
        ] }

        public var grantId: String { __data["grantId"] }
        public var applicationId: String { __data["applicationId"] }
        public var accountId: String? { __data["accountId"] }
        public var accountLabel: String? { __data["accountLabel"] }
        public var providerDisplayName: String { __data["providerDisplayName"] }
        public var audience: String { __data["audience"] }
        public var desiredScopes: [String] { __data["desiredScopes"] }
        public var grantedScopes: [String] { __data["grantedScopes"] }
        public var authorityRevision: Int { __data["authorityRevision"] }
        public var tokenRevision: Int { __data["tokenRevision"] }
        public var status: String { __data["status"] }
        public var connectionIds: [String] { __data["connectionIds"] }
      }
    }
  }
}
