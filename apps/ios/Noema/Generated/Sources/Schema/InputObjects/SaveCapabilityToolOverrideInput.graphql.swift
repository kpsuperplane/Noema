// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

nonisolated public struct SaveCapabilityToolOverrideInput: InputObject {
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
    readOnly: Bool,
    idempotent: Bool,
    destructive: Bool,
    openWorld: Bool
  ) {
    __data = InputDict([
      "kind": kind,
      "connectionId": connectionId,
      "expectedConnectionRevision": expectedConnectionRevision,
      "toolId": toolId,
      "sourceRevision": sourceRevision,
      "expectedPolicyRevision": expectedPolicyRevision,
      "readOnly": readOnly,
      "idempotent": idempotent,
      "destructive": destructive,
      "openWorld": openWorld
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

  public var readOnly: Bool {
    get { __data["readOnly"] }
    set { __data["readOnly"] = newValue }
  }

  public var idempotent: Bool {
    get { __data["idempotent"] }
    set { __data["idempotent"] = newValue }
  }

  public var destructive: Bool {
    get { __data["destructive"] }
    set { __data["destructive"] = newValue }
  }

  public var openWorld: Bool {
    get { __data["openWorld"] }
    set { __data["openWorld"] = newValue }
  }
}
