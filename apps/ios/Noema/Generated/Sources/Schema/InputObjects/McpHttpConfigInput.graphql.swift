// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// HTTP MCP setup config.
nonisolated public struct McpHttpConfigInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    url: String,
    headers: GraphQLNullable<JSON> = nil,
    secretHeaders: GraphQLNullable<JSON> = nil,
    oauthClientCredentials: GraphQLNullable<McpOAuthClientCredentialsInput> = nil
  ) {
    __data = InputDict([
      "url": url,
      "headers": headers,
      "secretHeaders": secretHeaders,
      "oauthClientCredentials": oauthClientCredentials
    ])
  }

  /// MCP endpoint URL.
  public var url: String {
    get { __data["url"] }
    set { __data["url"] = newValue }
  }

  /// Non-secret request headers.
  public var headers: GraphQLNullable<JSON> {
    get { __data["headers"] }
    set { __data["headers"] = newValue }
  }

  /// Secret request headers stored on disk.
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
