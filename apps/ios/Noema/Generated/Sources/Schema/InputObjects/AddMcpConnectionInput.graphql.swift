// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Add a fresh authenticated connection to one exact MCP definition revision.
nonisolated public struct AddMcpConnectionInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    mcpDefinitionId: String,
    expectedDefinitionRevision: String,
    connectionLabel: GraphQLNullable<String> = nil,
    secretEnv: GraphQLNullable<JSON> = nil,
    secretHeaders: GraphQLNullable<JSON> = nil,
    oauthClientCredentials: GraphQLNullable<McpOAuthClientCredentialsInput> = nil,
    authPreference: GraphQLNullable<GraphQLEnum<McpSetupAuthPreference>> = nil
  ) {
    __data = InputDict([
      "mcpDefinitionId": mcpDefinitionId,
      "expectedDefinitionRevision": expectedDefinitionRevision,
      "connectionLabel": connectionLabel,
      "secretEnv": secretEnv,
      "secretHeaders": secretHeaders,
      "oauthClientCredentials": oauthClientCredentials,
      "authPreference": authPreference
    ])
  }

  public var mcpDefinitionId: String {
    get { __data["mcpDefinitionId"] }
    set { __data["mcpDefinitionId"] = newValue }
  }

  public var expectedDefinitionRevision: String {
    get { __data["expectedDefinitionRevision"] }
    set { __data["expectedDefinitionRevision"] = newValue }
  }

  public var connectionLabel: GraphQLNullable<String> {
    get { __data["connectionLabel"] }
    set { __data["connectionLabel"] = newValue }
  }

  public var secretEnv: GraphQLNullable<JSON> {
    get { __data["secretEnv"] }
    set { __data["secretEnv"] = newValue }
  }

  public var secretHeaders: GraphQLNullable<JSON> {
    get { __data["secretHeaders"] }
    set { __data["secretHeaders"] = newValue }
  }

  public var oauthClientCredentials: GraphQLNullable<McpOAuthClientCredentialsInput> {
    get { __data["oauthClientCredentials"] }
    set { __data["oauthClientCredentials"] = newValue }
  }

  public var authPreference: GraphQLNullable<GraphQLEnum<McpSetupAuthPreference>> {
    get { __data["authPreference"] }
    set { __data["authPreference"] = newValue }
  }
}
