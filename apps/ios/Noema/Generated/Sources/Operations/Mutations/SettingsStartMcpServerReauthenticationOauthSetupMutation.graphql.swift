// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct SettingsStartMcpServerReauthenticationOauthSetupMutation: GraphQLMutation {
  public static let operationName: String = "SettingsStartMcpServerReauthenticationOauthSetup"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation SettingsStartMcpServerReauthenticationOauthSetup($input: StartMcpServerReauthenticationOAuthSetupInput!) { startMcpServerReauthenticationOauthSetup(input: $input) { __typename attemptId status authorizationUrl errorMessage setupResult { __typename ...SettingsMcpSetupResultFields } } }"#,
      fragments: [SettingsMcpSetupResultFields.self]
    ))

  public var input: StartMcpServerReauthenticationOAuthSetupInput

  public init(input: StartMcpServerReauthenticationOAuthSetupInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("startMcpServerReauthenticationOauthSetup", StartMcpServerReauthenticationOauthSetup.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      SettingsStartMcpServerReauthenticationOauthSetupMutation.Data.self
    ] }

    /// Start browser OAuth reauthentication for an existing hosted MCP server.
    public var startMcpServerReauthenticationOauthSetup: StartMcpServerReauthenticationOauthSetup { __data["startMcpServerReauthenticationOauthSetup"] }

    /// StartMcpServerReauthenticationOauthSetup
    ///
    /// Parent Type: `McpOAuthSetupAttempt`
    nonisolated public struct StartMcpServerReauthenticationOauthSetup: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.McpOAuthSetupAttempt }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("attemptId", String.self),
        .field("status", String.self),
        .field("authorizationUrl", String?.self),
        .field("errorMessage", String?.self),
        .field("setupResult", SetupResult?.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SettingsStartMcpServerReauthenticationOauthSetupMutation.Data.StartMcpServerReauthenticationOauthSetup.self
      ] }

      /// Short-lived attempt id.
      public var attemptId: String { __data["attemptId"] }
      /// Current attempt status.
      public var status: String { __data["status"] }
      /// Authorization URL to open in the user's browser.
      public var authorizationUrl: String? { __data["authorizationUrl"] }
      /// Non-secret UI-safe failure message.
      public var errorMessage: String? { __data["errorMessage"] }
      /// Final setup result after OAuth callback and tool discovery.
      public var setupResult: SetupResult? { __data["setupResult"] }

      /// StartMcpServerReauthenticationOauthSetup.SetupResult
      ///
      /// Parent Type: `McpServerSetupResult`
      nonisolated public struct SetupResult: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.McpServerSetupResult }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .fragment(SettingsMcpSetupResultFields.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          SettingsStartMcpServerReauthenticationOauthSetupMutation.Data.StartMcpServerReauthenticationOauthSetup.SetupResult.self,
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
}
