// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct SettingsAdapterDefinitionsQuery: GraphQLQuery {
  public static let operationName: String = "SettingsAdapterDefinitions"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"query SettingsAdapterDefinitions { adapterDefinitions { __typename semanticDigest definitionId adapterId displayName definitionRevision sourceReference origin authenticationMode scopes clientSetupUrl oauthRedirectUri accountIdentityOperationId operations { __typename operationId method path readOnly idempotent destructive openWorld argumentNames responseTransform { __typename language sourceDigest source acceptedContentTypes outputSchemaJson } } manifestJson acceptsOauthClientJson connectionCount connections { __typename connectionId status accountKind connectionRevision credentialRevision grantRevision policyRevision grantedScopes allowedOperations policyConfigured } reviewed superseded } }"#
    ))

  public init() {}

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.QueryRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("adapterDefinitions", [AdapterDefinition].self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      SettingsAdapterDefinitionsQuery.Data.self
    ] }

    /// List filesystem-canonical adapter definitions safe to review in Settings.
    public var adapterDefinitions: [AdapterDefinition] { __data["adapterDefinitions"] }

    /// AdapterDefinition
    ///
    /// Parent Type: `AdapterDefinition`
    nonisolated public struct AdapterDefinition: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AdapterDefinition }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("semanticDigest", String.self),
        .field("definitionId", String.self),
        .field("adapterId", String.self),
        .field("displayName", String.self),
        .field("definitionRevision", String.self),
        .field("sourceReference", String.self),
        .field("origin", String.self),
        .field("authenticationMode", String.self),
        .field("scopes", [String].self),
        .field("clientSetupUrl", String?.self),
        .field("oauthRedirectUri", String?.self),
        .field("accountIdentityOperationId", String?.self),
        .field("operations", [Operation].self),
        .field("manifestJson", String.self),
        .field("acceptsOauthClientJson", Bool.self),
        .field("connectionCount", Int.self),
        .field("connections", [Connection].self),
        .field("reviewed", Bool.self),
        .field("superseded", Bool.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SettingsAdapterDefinitionsQuery.Data.AdapterDefinition.self
      ] }

      public var semanticDigest: String { __data["semanticDigest"] }
      public var definitionId: String { __data["definitionId"] }
      public var adapterId: String { __data["adapterId"] }
      public var displayName: String { __data["displayName"] }
      public var definitionRevision: String { __data["definitionRevision"] }
      public var sourceReference: String { __data["sourceReference"] }
      public var origin: String { __data["origin"] }
      public var authenticationMode: String { __data["authenticationMode"] }
      public var scopes: [String] { __data["scopes"] }
      public var clientSetupUrl: String? { __data["clientSetupUrl"] }
      public var oauthRedirectUri: String? { __data["oauthRedirectUri"] }
      public var accountIdentityOperationId: String? { __data["accountIdentityOperationId"] }
      public var operations: [Operation] { __data["operations"] }
      public var manifestJson: String { __data["manifestJson"] }
      public var acceptsOauthClientJson: Bool { __data["acceptsOauthClientJson"] }
      public var connectionCount: Int { __data["connectionCount"] }
      public var connections: [Connection] { __data["connections"] }
      public var reviewed: Bool { __data["reviewed"] }
      public var superseded: Bool { __data["superseded"] }

      /// AdapterDefinition.Operation
      ///
      /// Parent Type: `AdapterOperation`
      nonisolated public struct Operation: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AdapterOperation }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("operationId", String.self),
          .field("method", String.self),
          .field("path", String.self),
          .field("readOnly", Bool?.self),
          .field("idempotent", Bool?.self),
          .field("destructive", Bool?.self),
          .field("openWorld", Bool?.self),
          .field("argumentNames", [String].self),
          .field("responseTransform", ResponseTransform?.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          SettingsAdapterDefinitionsQuery.Data.AdapterDefinition.Operation.self
        ] }

        public var operationId: String { __data["operationId"] }
        public var method: String { __data["method"] }
        public var path: String { __data["path"] }
        public var readOnly: Bool? { __data["readOnly"] }
        public var idempotent: Bool? { __data["idempotent"] }
        public var destructive: Bool? { __data["destructive"] }
        public var openWorld: Bool? { __data["openWorld"] }
        public var argumentNames: [String] { __data["argumentNames"] }
        public var responseTransform: ResponseTransform? { __data["responseTransform"] }

        /// AdapterDefinition.Operation.ResponseTransform
        ///
        /// Parent Type: `AdapterResponseTransform`
        nonisolated public struct ResponseTransform: NoemaAPI.SelectionSet {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AdapterResponseTransform }
          @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .field("language", String.self),
            .field("sourceDigest", String.self),
            .field("source", String.self),
            .field("acceptedContentTypes", [String].self),
            .field("outputSchemaJson", String.self),
          ] }
          @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
            SettingsAdapterDefinitionsQuery.Data.AdapterDefinition.Operation.ResponseTransform.self
          ] }

          public var language: String { __data["language"] }
          public var sourceDigest: String { __data["sourceDigest"] }
          public var source: String { __data["source"] }
          public var acceptedContentTypes: [String] { __data["acceptedContentTypes"] }
          public var outputSchemaJson: String { __data["outputSchemaJson"] }
        }
      }

      /// AdapterDefinition.Connection
      ///
      /// Parent Type: `AdapterConnection`
      nonisolated public struct Connection: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AdapterConnection }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("connectionId", String.self),
          .field("status", String.self),
          .field("accountKind", String.self),
          .field("connectionRevision", Int.self),
          .field("credentialRevision", Int.self),
          .field("grantRevision", Int.self),
          .field("policyRevision", Int.self),
          .field("grantedScopes", [String].self),
          .field("allowedOperations", [String].self),
          .field("policyConfigured", Bool.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          SettingsAdapterDefinitionsQuery.Data.AdapterDefinition.Connection.self
        ] }

        public var connectionId: String { __data["connectionId"] }
        public var status: String { __data["status"] }
        public var accountKind: String { __data["accountKind"] }
        public var connectionRevision: Int { __data["connectionRevision"] }
        public var credentialRevision: Int { __data["credentialRevision"] }
        public var grantRevision: Int { __data["grantRevision"] }
        public var policyRevision: Int { __data["policyRevision"] }
        public var grantedScopes: [String] { __data["grantedScopes"] }
        public var allowedOperations: [String] { __data["allowedOperations"] }
        public var policyConfigured: Bool { __data["policyConfigured"] }
      }
    }
  }
}
