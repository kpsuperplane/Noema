// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Continue setup after adding authentication material.
nonisolated public struct ContinueMcpServerSetupInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    mcpServerId: String,
    secretEnv: GraphQLNullable<JSON> = nil,
    secretHeaders: GraphQLNullable<JSON> = nil,
    oauthClientCredentials: GraphQLNullable<McpOAuthClientCredentialsInput> = nil
  ) {
    __data = InputDict([
      "mcpServerId": mcpServerId,
      "secretEnv": secretEnv,
      "secretHeaders": secretHeaders,
      "oauthClientCredentials": oauthClientCredentials
    ])
  }

  /// Durable MCP server id.
  public var mcpServerId: String {
    get { __data["mcpServerId"] }
    set { __data["mcpServerId"] = newValue }
  }

  /// Secret environment variables stored on disk.
  public var secretEnv: GraphQLNullable<JSON> {
    get { __data["secretEnv"] }
    set { __data["secretEnv"] = newValue }
  }

  /// Secret headers stored on disk.
  public var secretHeaders: GraphQLNullable<JSON> {
    get { __data["secretHeaders"] }
    set { __data["secretHeaders"] = newValue }
  }

  /// OAuth client-secret credentials, when supported by the server.
  public var oauthClientCredentials: GraphQLNullable<McpOAuthClientCredentialsInput> {
    get { __data["oauthClientCredentials"] }
    set { __data["oauthClientCredentials"] = newValue }
  }
}
