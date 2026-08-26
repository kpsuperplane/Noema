// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct SettingsContinueMcpServerSetupMutation: GraphQLMutation {
  public static let operationName: String = "SettingsContinueMcpServerSetup"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation SettingsContinueMcpServerSetup($input: ContinueMcpServerSetupInput!) { continueMcpServerSetup(input: $input) { __typename ...SettingsMcpSetupResultFields } }"#,
      fragments: [SettingsMcpSetupResultFields.self]
    ))

  public var input: ContinueMcpServerSetupInput

  public init(input: ContinueMcpServerSetupInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("continueMcpServerSetup", ContinueMcpServerSetup.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      SettingsContinueMcpServerSetupMutation.Data.self
    ] }

    /// Continue MCP server setup after adding authentication material.
    public var continueMcpServerSetup: ContinueMcpServerSetup { __data["continueMcpServerSetup"] }

    /// ContinueMcpServerSetup
    ///
    /// Parent Type: `McpServerSetupResult`
    nonisolated public struct ContinueMcpServerSetup: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.McpServerSetupResult }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .fragment(SettingsMcpSetupResultFields.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SettingsContinueMcpServerSetupMutation.Data.ContinueMcpServerSetup.self,
        SettingsMcpSetupResultFields.self
      ] }

      /// Server metadata safe to show in Settings.
      public var server: Server? { __data["server"] }
      /// Setup status.
      public var setupStatus: String { __data["setupStatus"] }
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
