// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Exact current pending definition abandoned by the local human.
nonisolated public struct CancelAdapterDefinitionInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    semanticDigest: String
  ) {
    __data = InputDict([
      "semanticDigest": semanticDigest
    ])
  }

  public var semanticDigest: String {
    get { __data["semanticDigest"] }
    set { __data["semanticDigest"] = newValue }
  }
}
