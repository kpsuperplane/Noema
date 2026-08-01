// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Exact immutable pending definition selected by the local human.
nonisolated public struct ApproveAdapterDefinitionInput: InputObject {
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
