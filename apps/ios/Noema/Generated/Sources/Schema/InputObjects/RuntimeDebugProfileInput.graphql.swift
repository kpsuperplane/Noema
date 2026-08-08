// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

nonisolated public struct RuntimeDebugProfileInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    kind: GraphQLEnum<RuntimeDebugScopeKind>,
    scopeId: String
  ) {
    __data = InputDict([
      "kind": kind,
      "scopeId": scopeId
    ])
  }

  public var kind: GraphQLEnum<RuntimeDebugScopeKind> {
    get { __data["kind"] }
    set { __data["kind"] = newValue }
  }

  public var scopeId: String {
    get { __data["scopeId"] }
    set { __data["scopeId"] = newValue }
  }
}
