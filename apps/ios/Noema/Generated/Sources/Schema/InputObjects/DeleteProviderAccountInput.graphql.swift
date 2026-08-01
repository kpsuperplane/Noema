// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Input for hard-deleting a user-managed provider account.
nonisolated public struct DeleteProviderAccountInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    providerAccountId: String
  ) {
    __data = InputDict([
      "providerAccountId": providerAccountId
    ])
  }

  public var providerAccountId: String {
    get { __data["providerAccountId"] }
    set { __data["providerAccountId"] = newValue }
  }
}
