// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Label one grant when the provider exposes no stable account identity.
nonisolated public struct SaveAdapterOauthGrantLabelInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    grantId: String,
    expectedAuthorityRevision: Int32,
    accountLabel: GraphQLNullable<String> = nil
  ) {
    __data = InputDict([
      "grantId": grantId,
      "expectedAuthorityRevision": expectedAuthorityRevision,
      "accountLabel": accountLabel
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

  public var accountLabel: GraphQLNullable<String> {
    get { __data["accountLabel"] }
    set { __data["accountLabel"] = newValue }
  }
}
