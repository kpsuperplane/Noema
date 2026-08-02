// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct SettingsSetupAdapterConnectionMutation: GraphQLMutation {
  public static let operationName: String = "SettingsSetupAdapterConnection"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation SettingsSetupAdapterConnection($input: SetupAdapterConnectionInput!) { setupAdapterConnection(input: $input) { __typename ...SettingsAdapterDefinitionFields } }"#,
      fragments: [AdapterCredentialSetupFields.self, SettingsAdapterDefinitionFields.self]
    ))

  public var input: SetupAdapterConnectionInput

  public init(input: SetupAdapterConnectionInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("setupAdapterConnection", SetupAdapterConnection.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      SettingsSetupAdapterConnectionMutation.Data.self
    ] }

    /// Normalize transient credential input and create one adapter connection.
    public var setupAdapterConnection: SetupAdapterConnection { __data["setupAdapterConnection"] }

    /// SetupAdapterConnection
    ///
    /// Parent Type: `AdapterDefinition`
    nonisolated public struct SetupAdapterConnection: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AdapterDefinition }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .fragment(SettingsAdapterDefinitionFields.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SettingsSetupAdapterConnectionMutation.Data.SetupAdapterConnection.self,
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
      public var scopes: [String] { __data["scopes"] }
      public var credentialSetup: CredentialSetup? { __data["credentialSetup"] }
      public var accountIdentityOperationId: String? { __data["accountIdentityOperationId"] }
      public var operations: [Operation] { __data["operations"] }
      public var connectionCount: Int { __data["connectionCount"] }
      public var connections: [Connection] { __data["connections"] }
      public var reviewed: Bool { __data["reviewed"] }
      public var superseded: Bool { __data["superseded"] }

      public struct Fragments: FragmentContainer {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        public var settingsAdapterDefinitionFields: SettingsAdapterDefinitionFields { _toFragment() }
      }

      public typealias CredentialSetup = SettingsAdapterDefinitionFields.CredentialSetup

      public typealias Operation = SettingsAdapterDefinitionFields.Operation

      public typealias Connection = SettingsAdapterDefinitionFields.Connection
    }
  }
}
