// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Import one reusable OAuth application document.
nonisolated public struct ImportAdapterOauthApplicationInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    profileDigest: String,
    projectLabel: GraphQLNullable<String> = nil,
    clientDocumentBase64: String
  ) {
    __data = InputDict([
      "profileDigest": profileDigest,
      "projectLabel": projectLabel,
      "clientDocumentBase64": clientDocumentBase64
    ])
  }

  public var profileDigest: String {
    get { __data["profileDigest"] }
    set { __data["profileDigest"] = newValue }
  }

  public var projectLabel: GraphQLNullable<String> {
    get { __data["projectLabel"] }
    set { __data["projectLabel"] = newValue }
  }

  public var clientDocumentBase64: String {
    get { __data["clientDocumentBase64"] }
    set { __data["clientDocumentBase64"] = newValue }
  }
}
