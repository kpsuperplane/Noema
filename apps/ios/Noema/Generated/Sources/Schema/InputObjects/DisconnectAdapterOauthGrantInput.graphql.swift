// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Disconnect one exact reusable grant revision.
nonisolated public struct DisconnectAdapterOauthGrantInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    grantId: String,
    expectedAuthorityRevision: Int32
  ) {
    __data = InputDict([
      "grantId": grantId,
      "expectedAuthorityRevision": expectedAuthorityRevision
    ])
  }

  public var grantId: String {
    get { __data["grantId"] }
    set { __data["grantId"] = newValue }
  }

  public var expectedAuthorityRevision: Int32 {
    get { __data["expectedAuthorityRevision"] }
    set { __data["expectedAuthorityRevision"] = newValue }
  }
}
