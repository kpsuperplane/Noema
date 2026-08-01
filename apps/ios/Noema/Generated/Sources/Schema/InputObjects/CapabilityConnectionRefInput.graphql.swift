// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

nonisolated public struct CapabilityConnectionRefInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    kind: GraphQLEnum<CapabilityIntegrationKind>,
    connectionId: String
  ) {
    __data = InputDict([
      "kind": kind,
      "connectionId": connectionId
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
}
