// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Skip one exact API adapter call without replacing its credential.
nonisolated public struct SkipAdapterAuthenticationInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    requestId: String,
    expectedRevision: Int32
  ) {
    __data = InputDict([
      "requestId": requestId,
      "expectedRevision": expectedRevision
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
}
