// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Start browser sign-in for one exact request revision.
nonisolated public struct StartMcpAuthenticationInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    requestId: String,
    expectedRevision: Int32,
    redirectUri: String
  ) {
    __data = InputDict([
      "requestId": requestId,
      "expectedRevision": expectedRevision,
      "redirectUri": redirectUri
    ])
  }

  public var requestId: String {
    get { __data["requestId"] }
    set { __data["requestId"] = newValue }
  }

  public var expectedRevision: Int32 {
    get { __data["expectedRevision"] }
    set { __data["expectedRevision"] = newValue }
  }

  public var redirectUri: String {
    get { __data["redirectUri"] }
    set { __data["redirectUri"] = newValue }
  }
}
