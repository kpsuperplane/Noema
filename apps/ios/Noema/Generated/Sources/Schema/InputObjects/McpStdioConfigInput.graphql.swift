// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Stdio MCP setup config.
nonisolated public struct McpStdioConfigInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    command: String,
    args: [String],
    cwd: GraphQLNullable<String> = nil,
    env: GraphQLNullable<JSON> = nil,
    secretEnv: GraphQLNullable<JSON> = nil
  ) {
    __data = InputDict([
      "command": command,
      "args": args,
      "cwd": cwd,
      "env": env,
      "secretEnv": secretEnv
    ])
  }

  /// Command to launch.
  public var command: String {
    get { __data["command"] }
    set { __data["command"] = newValue }
  }

  /// Command arguments.
  public var args: [String] {
    get { __data["args"] }
    set { __data["args"] = newValue }
  }

  /// Optional working directory.
  public var cwd: GraphQLNullable<String> {
    get { __data["cwd"] }
    set { __data["cwd"] = newValue }
  }

  /// Non-secret environment variables.
  public var env: GraphQLNullable<JSON> {
    get { __data["env"] }
    set { __data["env"] = newValue }
  }

  /// Secret environment variables stored on disk.
  public var secretEnv: GraphQLNullable<JSON> {
    get { __data["secretEnv"] }
    set { __data["secretEnv"] = newValue }
  }
}
