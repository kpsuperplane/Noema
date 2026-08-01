// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Input for saving a write-only provider secret.
nonisolated public struct ProviderSecretInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    providerAccountId: String,
    secret: String
  ) {
    __data = InputDict([
      "providerAccountId": providerAccountId,
      "secret": secret
    ])
  }

  public var providerAccountId: String {
    get { __data["providerAccountId"] }
    set { __data["providerAccountId"] = newValue }
  }

  public var secret: String {
    get { __data["secret"] }
    set { __data["secret"] = newValue }
  }
}
