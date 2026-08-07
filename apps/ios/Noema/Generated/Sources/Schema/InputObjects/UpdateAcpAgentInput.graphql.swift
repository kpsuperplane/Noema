// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

nonisolated public struct UpdateAcpAgentInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    agentId: String,
    expectedRevision: Int32,
    displayName: String,
    command: String,
    arguments: [String]? = nil,
    enabled: Bool
  ) {
    __data = InputDict([
      "agentId": agentId,
      "expectedRevision": expectedRevision,
      "displayName": displayName,
      "command": command,
      "arguments": arguments ?? GraphQLNullable.none,
      "enabled": enabled
    ])
  }

  public var agentId: String {
    get { __data["agentId"] }
    set { __data["agentId"] = newValue }
  }

  public var expectedRevision: Int32 {
    get { __data["expectedRevision"] }
    set { __data["expectedRevision"] = newValue }
  }

  public var displayName: String {
    get { __data["displayName"] }
    set { __data["displayName"] = newValue }
  }

  public var command: String {
    get { __data["command"] }
    set { __data["command"] = newValue }
  }

  public var arguments: [String]? {
    get { __data["arguments"] }
    set { __data["arguments"] = newValue }
  }

  public var enabled: Bool {
    get { __data["enabled"] }
    set { __data["enabled"] = newValue }
  }
}
