// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

nonisolated public struct SetCapabilityToolEnabledInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    kind: GraphQLEnum<CapabilityIntegrationKind>,
    connectionId: String,
    expectedConnectionRevision: String,
    toolId: String,
    sourceRevision: String,
    expectedPolicyRevision: Int32,
    enabled: Bool
  ) {
    __data = InputDict([
      "kind": kind,
      "connectionId": connectionId,
      "expectedConnectionRevision": expectedConnectionRevision,
      "toolId": toolId,
      "sourceRevision": sourceRevision,
      "expectedPolicyRevision": expectedPolicyRevision,
      "enabled": enabled
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

  public var toolId: String {
    get { __data["toolId"] }
    set { __data["toolId"] = newValue }
  }

  public var sourceRevision: String {
    get { __data["sourceRevision"] }
    set { __data["sourceRevision"] = newValue }
  }

  public var expectedPolicyRevision: Int32 {
    get { __data["expectedPolicyRevision"] }
    set { __data["expectedPolicyRevision"] = newValue }
  }

  public var enabled: Bool {
    get { __data["enabled"] }
    set { __data["enabled"] = newValue }
  }
}
