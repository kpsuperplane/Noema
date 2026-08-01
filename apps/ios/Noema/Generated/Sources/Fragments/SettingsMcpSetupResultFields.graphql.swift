// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct SettingsMcpSetupResultFields: NoemaAPI.SelectionSet, Fragment {
  public static var fragmentDefinition: StaticString {
    #"fragment SettingsMcpSetupResultFields on McpServerSetupResult { __typename server { __typename mcpServerId connectionRevision policyRevision displayName transportKind healthStatus authStatus toolCount pendingToolCount browserOauthReauthenticationSupported } setupStatus discoveryStatus discoveredToolCount setupError auth { __typename oauthClientCredentialsSupported oauthAuthorizationSupported scopes } }"#
  }

  @_spi(Unsafe) public let __data: DataDict
  @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

  @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.McpServerSetupResult }
  @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
    .field("__typename", String.self),
    .field("server", Server?.self),
    .field("setupStatus", String.self),
    .field("discoveryStatus", String?.self),
    .field("discoveredToolCount", Int.self),
    .field("setupError", String?.self),
    .field("auth", Auth?.self),
  ] }
  @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
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

  /// Server
  ///
  /// Parent Type: `McpServer`
  nonisolated public struct Server: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.McpServer }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("__typename", String.self),
      .field("mcpServerId", String.self),
      .field("connectionRevision", String.self),
      .field("policyRevision", Int.self),
      .field("displayName", String.self),
      .field("transportKind", String.self),
      .field("healthStatus", String.self),
      .field("authStatus", String.self),
      .field("toolCount", Int.self),
      .field("pendingToolCount", Int.self),
      .field("browserOauthReauthenticationSupported", Bool.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      SettingsMcpSetupResultFields.Server.self
    ] }

    /// Durable MCP server id.
    public var mcpServerId: String { __data["mcpServerId"] }
    /// Exact connection authority generation used for policy fencing.
    public var connectionRevision: String { __data["connectionRevision"] }
    /// Monotonic provider-policy revision used for policy fencing.
    public var policyRevision: Int { __data["policyRevision"] }
    /// Human-visible server name.
    public var displayName: String { __data["displayName"] }
    /// Transport used to connect to the server.
    public var transportKind: String { __data["transportKind"] }
    /// Last known server health.
    public var healthStatus: String { __data["healthStatus"] }
    /// Last known server authentication state.
    public var authStatus: String { __data["authStatus"] }
    /// Number of discovered tools for this server.
    public var toolCount: Int { __data["toolCount"] }
    /// Number of tools waiting for background classification.
    public var pendingToolCount: Int { __data["pendingToolCount"] }
    /// Whether this persisted server can restart browser OAuth authorization.
    public var browserOauthReauthenticationSupported: Bool { __data["browserOauthReauthenticationSupported"] }
  }

  /// Auth
  ///
  /// Parent Type: `McpSetupAuthDetails`
  nonisolated public struct Auth: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.McpSetupAuthDetails }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("__typename", String.self),
      .field("oauthClientCredentialsSupported", Bool.self),
      .field("oauthAuthorizationSupported", Bool.self),
      .field("scopes", [String].self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      SettingsMcpSetupResultFields.Auth.self
    ] }

    /// Whether OAuth client-secret credentials can be attempted.
    public var oauthClientCredentialsSupported: Bool { __data["oauthClientCredentialsSupported"] }
    /// Whether browser OAuth authorization can be attempted from the server URL.
    public var oauthAuthorizationSupported: Bool { __data["oauthAuthorizationSupported"] }
    /// Suggested OAuth scopes, when known.
    public var scopes: [String] { __data["scopes"] }
  }
}
