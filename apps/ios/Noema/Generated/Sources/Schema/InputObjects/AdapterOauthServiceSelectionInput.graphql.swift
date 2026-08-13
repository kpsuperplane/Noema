// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// One additional reviewed API included in the same OAuth authorization.
nonisolated public struct AdapterOauthServiceSelectionInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    semanticDigest: String,
    operationIds: [String]
  ) {
    __data = InputDict([
      "semanticDigest": semanticDigest,
      "operationIds": operationIds
    ])
  }

  public var semanticDigest: String {
    get { __data["semanticDigest"] }
    set { __data["semanticDigest"] = newValue }
  }

  public var operationIds: [String] {
    get { __data["operationIds"] }
    set { __data["operationIds"] = newValue }
  }
}
