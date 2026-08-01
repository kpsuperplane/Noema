// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct SettingsAddMcpConnectionMutation: GraphQLMutation {
  public static let operationName: String = "SettingsAddMcpConnection"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation SettingsAddMcpConnection($input: AddMcpConnectionInput!) { addMcpConnection(input: $input) { __typename ...SettingsMcpSetupResultFields } }"#,
      fragments: [SettingsMcpSetupResultFields.self]
    ))

  public var input: AddMcpConnectionInput

  public init(input: AddMcpConnectionInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("addMcpConnection", AddMcpConnection.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      SettingsAddMcpConnectionMutation.Data.self
    ] }

    /// Add a fresh connection to one explicitly selected MCP definition revision.
    public var addMcpConnection: AddMcpConnection { __data["addMcpConnection"] }

    /// AddMcpConnection
    ///
    /// Parent Type: `McpServerSetupResult`
    nonisolated public struct AddMcpConnection: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.McpServerSetupResult }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .fragment(SettingsMcpSetupResultFields.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SettingsAddMcpConnectionMutation.Data.AddMcpConnection.self,
        SettingsMcpSetupResultFields.self
      ] }

      /// Server metadata safe to show in Settings.
      public var server: Server? { __data["server"] }
      /// Setup status.
      public var setupStatus: String { __data["setupStatus"] }
      /// Metadata discovery status.
      public var discoveryStatus: String? { __data["discoveryStatus"] }
      /// Number of discovered tools.
      public var discoveredToolCount: Int { __data["discoveredToolCount"] }
      /// Non-secret setup error, when present.
      public var setupError: String? { __data["setupError"] }
      /// Authentication options safe to show to the user.
      public var auth: Auth? { __data["auth"] }

      public struct Fragments: FragmentContainer {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        public var settingsMcpSetupResultFields: SettingsMcpSetupResultFields { _toFragment() }
      }

      public typealias Server = SettingsMcpSetupResultFields.Server

      public typealias Auth = SettingsMcpSetupResultFields.Auth
    }
  }
}
