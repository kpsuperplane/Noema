// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

nonisolated public struct SaveCapabilityConnectionPolicyInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    kind: GraphQLEnum<CapabilityIntegrationKind>,
    connectionId: String,
    expectedConnectionRevision: String,
    expectedPolicyRevision: Int32,
    dataSharingPolicy: String,
    unsafeActionPolicy: String
  ) {
    __data = InputDict([
      "kind": kind,
      "connectionId": connectionId,
      "expectedConnectionRevision": expectedConnectionRevision,
      "expectedPolicyRevision": expectedPolicyRevision,
      "dataSharingPolicy": dataSharingPolicy,
      "unsafeActionPolicy": unsafeActionPolicy
    ])
  }

  public var kind: GraphQLEnum<CapabilityIntegrationKind> {
    get { __data["kind"] }
    set { __data["kind"] = newValue }
  }

  public var connectionId: String {
    get { __data["connectionId"] }
    set { __data["connectionId"] = newValue }
  }

  public var expectedConnectionRevision: String {
    get { __data["expectedConnectionRevision"] }
    set { __data["expectedConnectionRevision"] = newValue }
  }

  public var expectedPolicyRevision: Int32 {
    get { __data["expectedPolicyRevision"] }
    set { __data["expectedPolicyRevision"] = newValue }
  }

  public var dataSharingPolicy: String {
    get { __data["dataSharingPolicy"] }
    set { __data["dataSharingPolicy"] = newValue }
  }

  public var unsafeActionPolicy: String {
    get { __data["unsafeActionPolicy"] }
    set { __data["unsafeActionPolicy"] = newValue }
  }
}
