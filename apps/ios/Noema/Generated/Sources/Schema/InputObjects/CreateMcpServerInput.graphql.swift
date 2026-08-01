// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Add and verify an MCP server.
nonisolated public struct CreateMcpServerInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    displayName: String,
    transportKind: String,
    stdio: GraphQLNullable<McpStdioConfigInput> = nil,
    http: GraphQLNullable<McpHttpConfigInput> = nil,
    authPreference: GraphQLNullable<GraphQLEnum<McpSetupAuthPreference>> = nil
  ) {
    __data = InputDict([
      "displayName": displayName,
      "transportKind": transportKind,
      "stdio": stdio,
      "http": http,
      "authPreference": authPreference
    ])
  }

  /// Human-visible server name.
  public var displayName: String {
    get { __data["displayName"] }
    set { __data["displayName"] = newValue }
  }

  /// MCP transport kind: `stdio` or `streamable_http`.
  public var transportKind: String {
    get { __data["transportKind"] }
    set { __data["transportKind"] = newValue }
  }

  /// Stdio transport config, when `transport_kind` is `stdio`.
  public var stdio: GraphQLNullable<McpStdioConfigInput> {
    get { __data["stdio"] }
    set { __data["stdio"] = newValue }
  }

  /// HTTP transport config, when `transport_kind` is `streamable_http`.
  public var http: GraphQLNullable<McpHttpConfigInput> {
    get { __data["http"] }
    set { __data["http"] = newValue }
  }

  /// How to handle optional browser authentication advertised after discovery.
  public var authPreference: GraphQLNullable<GraphQLEnum<McpSetupAuthPreference>> {
    get { __data["authPreference"] }
    set { __data["authPreference"] = newValue }
  }
}
