// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

nonisolated public struct CreateAcpAgentInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    displayName: String,
    command: String,
    arguments: [String]? = nil
  ) {
    __data = InputDict([
      "displayName": displayName,
      "command": command,
      "arguments": arguments ?? GraphQLNullable.none
    ])
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
}
