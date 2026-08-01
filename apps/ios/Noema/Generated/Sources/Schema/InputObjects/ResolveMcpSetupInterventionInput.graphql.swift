// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Resolve one exact MCP setup intervention after its policy is configured.
nonisolated public struct ResolveMcpSetupInterventionInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    itemId: String,
    mcpServerId: String
  ) {
    __data = InputDict([
      "itemId": itemId,
      "mcpServerId": mcpServerId
    ])
  }

  public var itemId: String {
    get { __data["itemId"] }
    set { __data["itemId"] = newValue }
  }

  public var mcpServerId: String {
    get { __data["mcpServerId"] }
    set { __data["mcpServerId"] = newValue }
  }
}
