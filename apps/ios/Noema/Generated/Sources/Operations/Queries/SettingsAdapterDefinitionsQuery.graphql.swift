// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct SettingsAdapterDefinitionsQuery: GraphQLQuery {
  public static let operationName: String = "SettingsAdapterDefinitions"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"query SettingsAdapterDefinitions { adapterDefinitions { __typename ...SettingsAdapterDefinitionFields } }"#,
      fragments: [AdapterCredentialSetupFields.self, SettingsAdapterDefinitionFields.self]
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
        .fragment(SettingsAdapterDefinitionFields.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SettingsAdapterDefinitionsQuery.Data.AdapterDefinition.self,
        SettingsAdapterDefinitionFields.self
      ] }

      public var semanticDigest: String { __data["semanticDigest"] }
      public var definitionId: String { __data["definitionId"] }
      public var adapterId: String { __data["adapterId"] }
      public var displayName: String { __data["displayName"] }
      public var definitionRevision: String { __data["definitionRevision"] }
      public var sourceReference: String { __data["sourceReference"] }
      public var origin: String { __data["origin"] }
      public var authenticationMode: String { __data["authenticationMode"] }
      public var oauthProfileDigest: String? { __data["oauthProfileDigest"] }
      public var scopes: [String] { __data["scopes"] }
      public var credentialSetup: CredentialSetup? { __data["credentialSetup"] }
      public var accountIdentityOperationId: String? { __data["accountIdentityOperationId"] }
      public var manifestJson: String { __data["manifestJson"] }
      public var operations: [Operation] { __data["operations"] }
      public var connectionCount: Int { __data["connectionCount"] }
      public var connections: [Connection] { __data["connections"] }
      public var reviewed: Bool { __data["reviewed"] }
      public var superseded: Bool { __data["superseded"] }
      public var nextAction: NextAction? { __data["nextAction"] }
      public var connectionActions: [ConnectionAction] { __data["connectionActions"] }

      public struct Fragments: FragmentContainer {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        public var settingsAdapterDefinitionFields: SettingsAdapterDefinitionFields { _toFragment() }
      }

      public typealias CredentialSetup = SettingsAdapterDefinitionFields.CredentialSetup

      public typealias Operation = SettingsAdapterDefinitionFields.Operation

      public typealias Connection = SettingsAdapterDefinitionFields.Connection

      public typealias NextAction = SettingsAdapterDefinitionFields.NextAction

      public typealias ConnectionAction = SettingsAdapterDefinitionFields.ConnectionAction
    }
  }
}
