// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Delete one unreferenced OAuth application revision.
nonisolated public struct DeleteAdapterOauthApplicationInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    applicationId: String,
    expectedRevision: Int32
  ) {
    __data = InputDict([
      "applicationId": applicationId,
      "expectedRevision": expectedRevision
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
}
