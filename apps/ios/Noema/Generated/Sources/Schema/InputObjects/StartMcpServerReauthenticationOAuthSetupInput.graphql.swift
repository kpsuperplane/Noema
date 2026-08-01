// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Start a browser OAuth reauthentication attempt for an existing MCP server.
nonisolated public struct StartMcpServerReauthenticationOAuthSetupInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    mcpServerId: String,
    redirectUri: String
  ) {
    __data = InputDict([
      "mcpServerId": mcpServerId,
      "redirectUri": redirectUri
    ])
  }

  /// Durable MCP server id.
  public var mcpServerId: String {
    get { __data["mcpServerId"] }
    set { __data["mcpServerId"] = newValue }
  }

  /// Absolute local callback URI owned by Noema web.
  public var redirectUri: String {
    get { __data["redirectUri"] }
    set { __data["redirectUri"] = newValue }
  }
}
