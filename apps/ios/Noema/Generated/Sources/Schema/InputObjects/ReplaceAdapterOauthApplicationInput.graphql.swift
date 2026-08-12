// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Replace one exact OAuth application document.
nonisolated public struct ReplaceAdapterOauthApplicationInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    applicationId: String,
    expectedRevision: Int32,
    clientDocumentBase64: String
  ) {
    __data = InputDict([
      "applicationId": applicationId,
      "expectedRevision": expectedRevision,
      "clientDocumentBase64": clientDocumentBase64
    ])
  }

  public var applicationId: String {
    get { __data["applicationId"] }
    set { __data["applicationId"] = newValue }
  }

  public var expectedRevision: Int32 {
    get { __data["expectedRevision"] }
    set { __data["expectedRevision"] = newValue }
  }

  public var clientDocumentBase64: String {
    get { __data["clientDocumentBase64"] }
    set { __data["clientDocumentBase64"] = newValue }
  }
}
