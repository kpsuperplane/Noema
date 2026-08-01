// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Start a browser OAuth setup attempt for a hosted MCP server.
nonisolated public struct StartMcpServerOAuthSetupInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    server: CreateMcpServerInput,
    redirectUri: String
  ) {
    __data = InputDict([
      "server": server,
      "redirectUri": redirectUri
    ])
  }

  /// Pending MCP server setup input.
  public var server: CreateMcpServerInput {
    get { __data["server"] }
    set { __data["server"] = newValue }
  }

  /// Absolute local callback URI owned by Noema web.
  public var redirectUri: String {
    get { __data["redirectUri"] }
    set { __data["redirectUri"] = newValue }
  }
}
