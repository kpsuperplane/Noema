// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct SettingsAttachAdapterOauthConnectionMutation: GraphQLMutation {
  public static let operationName: String = "SettingsAttachAdapterOauthConnection"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation SettingsAttachAdapterOauthConnection($input: AttachAdapterOauthConnectionInput!) { attachAdapterOauthConnection(input: $input) { __typename semanticDigest connectionCount connections { __typename connectionId grantId policyConfigured } } }"#
    ))

  public var input: AttachAdapterOauthConnectionInput

  public init(input: AttachAdapterOauthConnectionInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("attachAdapterOauthConnection", AttachAdapterOauthConnection.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      SettingsAttachAdapterOauthConnectionMutation.Data.self
    ] }

    /// Attach one API definition to one account grant.
    public var attachAdapterOauthConnection: AttachAdapterOauthConnection { __data["attachAdapterOauthConnection"] }

    /// AttachAdapterOauthConnection
    ///
    /// Parent Type: `AdapterDefinition`
    nonisolated public struct AttachAdapterOauthConnection: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AdapterDefinition }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("semanticDigest", String.self),
        .field("connectionCount", Int.self),
        .field("connections", [Connection].self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SettingsAttachAdapterOauthConnectionMutation.Data.AttachAdapterOauthConnection.self
      ] }

      public var semanticDigest: String { __data["semanticDigest"] }
      public var connectionCount: Int { __data["connectionCount"] }
      public var connections: [Connection] { __data["connections"] }

      /// AttachAdapterOauthConnection.Connection
      ///
      /// Parent Type: `AdapterConnection`
      nonisolated public struct Connection: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AdapterConnection }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("connectionId", String.self),
          .field("grantId", String?.self),
          .field("policyConfigured", Bool.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          SettingsAttachAdapterOauthConnectionMutation.Data.AttachAdapterOauthConnection.Connection.self
        ] }

        public var connectionId: String { __data["connectionId"] }
        public var grantId: String? { __data["grantId"] }
        public var policyConfigured: Bool { __data["policyConfigured"] }
      }
    }
  }
}
