// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Attach one reviewed API definition to one reusable grant.
nonisolated public struct AttachAdapterOauthConnectionInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    semanticDigest: String,
    grantId: String,
    expectedGrantRevision: Int32,
    replacementConnectionId: GraphQLNullable<String> = nil
  ) {
    __data = InputDict([
      "semanticDigest": semanticDigest,
      "grantId": grantId,
      "expectedGrantRevision": expectedGrantRevision,
      "replacementConnectionId": replacementConnectionId
    ])
  }

  public var semanticDigest: String {
    get { __data["semanticDigest"] }
    set { __data["semanticDigest"] = newValue }
  }

  public var grantId: String {
    get { __data["grantId"] }
    set { __data["grantId"] = newValue }
  }

  public var expectedGrantRevision: Int32 {
    get { __data["expectedGrantRevision"] }
    set { __data["expectedGrantRevision"] = newValue }
  }

  public var replacementConnectionId: GraphQLNullable<String> {
    get { __data["replacementConnectionId"] }
    set { __data["replacementConnectionId"] = newValue }
  }
}
