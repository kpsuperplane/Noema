// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

nonisolated public struct AuthenticateAcpAgentInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    agentId: String,
    expectedRevision: Int32,
    methodId: String
  ) {
    __data = InputDict([
      "agentId": agentId,
      "expectedRevision": expectedRevision,
      "methodId": methodId
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

  public var methodId: String {
    get { __data["methodId"] }
    set { __data["methodId"] = newValue }
  }
}
