// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Start browser OAuth against one exact filesystem connection revision.
nonisolated public struct StartAdapterOauthSetupInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    connectionId: String,
    expectedConnectionRevision: Int32,
    expectedCredentialRevision: Int32,
    expectedGrantRevision: Int32,
    expectedPolicyRevision: Int32
  ) {
    __data = InputDict([
      "connectionId": connectionId,
      "expectedConnectionRevision": expectedConnectionRevision,
      "expectedCredentialRevision": expectedCredentialRevision,
      "expectedGrantRevision": expectedGrantRevision,
      "expectedPolicyRevision": expectedPolicyRevision
    ])
  }

  public var connectionId: String {
    get { __data["connectionId"] }
    set { __data["connectionId"] = newValue }
  }

  public var expectedConnectionRevision: Int32 {
    get { __data["expectedConnectionRevision"] }
    set { __data["expectedConnectionRevision"] = newValue }
  }

  public var expectedCredentialRevision: Int32 {
    get { __data["expectedCredentialRevision"] }
    set { __data["expectedCredentialRevision"] = newValue }
  }

  public var expectedGrantRevision: Int32 {
    get { __data["expectedGrantRevision"] }
    set { __data["expectedGrantRevision"] = newValue }
  }

  public var expectedPolicyRevision: Int32 {
    get { __data["expectedPolicyRevision"] }
    set { __data["expectedPolicyRevision"] = newValue }
  }
}
