// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// OAuth client-secret credentials for MCP setup.
nonisolated public struct McpOAuthClientCredentialsInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    clientId: String,
    clientSecret: String,
    scopes: [String]
  ) {
    __data = InputDict([
      "clientId": clientId,
      "clientSecret": clientSecret,
      "scopes": scopes
    ])
  }

  /// OAuth client id.
  public var clientId: String {
    get { __data["clientId"] }
    set { __data["clientId"] = newValue }
  }

  /// OAuth client secret stored on disk.
  public var clientSecret: String {
    get { __data["clientSecret"] }
    set { __data["clientSecret"] = newValue }
  }

  /// OAuth scopes to request.
  public var scopes: [String] {
    get { __data["scopes"] }
    set { __data["scopes"] = newValue }
  }
}
