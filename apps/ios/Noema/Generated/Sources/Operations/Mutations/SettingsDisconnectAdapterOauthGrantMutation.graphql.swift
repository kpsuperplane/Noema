// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct SettingsDisconnectAdapterOauthGrantMutation: GraphQLMutation {
  public static let operationName: String = "SettingsDisconnectAdapterOauthGrant"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation SettingsDisconnectAdapterOauthGrant($input: DisconnectAdapterOauthGrantInput!) { disconnectAdapterOauthGrant(input: $input) { __typename grantId authorityRevision status connectionIds } }"#
    ))

  public var input: DisconnectAdapterOauthGrantInput

  public init(input: DisconnectAdapterOauthGrantInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("disconnectAdapterOauthGrant", DisconnectAdapterOauthGrant.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      SettingsDisconnectAdapterOauthGrantMutation.Data.self
    ] }

    /// Disconnect one reusable account grant.
    public var disconnectAdapterOauthGrant: DisconnectAdapterOauthGrant { __data["disconnectAdapterOauthGrant"] }

    /// DisconnectAdapterOauthGrant
    ///
    /// Parent Type: `AdapterAuthorizationGrant`
    nonisolated public struct DisconnectAdapterOauthGrant: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AdapterAuthorizationGrant }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("grantId", String.self),
        .field("authorityRevision", Int.self),
        .field("status", String.self),
        .field("connectionIds", [String].self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SettingsDisconnectAdapterOauthGrantMutation.Data.DisconnectAdapterOauthGrant.self
      ] }

      public var grantId: String { __data["grantId"] }
      public var authorityRevision: Int { __data["authorityRevision"] }
      public var status: String { __data["status"] }
      public var connectionIds: [String] { __data["connectionIds"] }
    }
  }
}
