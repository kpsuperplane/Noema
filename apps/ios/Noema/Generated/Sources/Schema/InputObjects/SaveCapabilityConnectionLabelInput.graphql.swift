// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

nonisolated public struct SaveCapabilityConnectionLabelInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    kind: GraphQLEnum<CapabilityIntegrationKind>,
    connectionId: String,
    expectedConnectionRevision: String,
    expectedConnectionLabel: GraphQLNullable<String> = nil,
    connectionLabel: GraphQLNullable<String> = nil
  ) {
    __data = InputDict([
      "kind": kind,
      "connectionId": connectionId,
      "expectedConnectionRevision": expectedConnectionRevision,
      "expectedConnectionLabel": expectedConnectionLabel,
      "connectionLabel": connectionLabel
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

  public var expectedConnectionLabel: GraphQLNullable<String> {
    get { __data["expectedConnectionLabel"] }
    set { __data["expectedConnectionLabel"] = newValue }
  }

  public var connectionLabel: GraphQLNullable<String> {
    get { __data["connectionLabel"] }
    set { __data["connectionLabel"] = newValue }
  }
}
