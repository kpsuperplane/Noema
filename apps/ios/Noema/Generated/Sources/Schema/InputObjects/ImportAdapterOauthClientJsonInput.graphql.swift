// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// One transient, human-selected OAuth client document for an exact definition.
nonisolated public struct ImportAdapterOauthClientJsonInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    semanticDigest: String,
    clientJsonBase64: String
  ) {
    __data = InputDict([
      "semanticDigest": semanticDigest,
      "clientJsonBase64": clientJsonBase64
    ])
  }

  public var semanticDigest: String {
    get { __data["semanticDigest"] }
    set { __data["semanticDigest"] = newValue }
  }

  public var clientJsonBase64: String {
    get { __data["clientJsonBase64"] }
    set { __data["clientJsonBase64"] = newValue }
  }
}
