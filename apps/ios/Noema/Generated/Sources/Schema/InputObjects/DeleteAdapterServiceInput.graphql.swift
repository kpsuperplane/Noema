// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Delete one exact adapter service family selection.
nonisolated public struct DeleteAdapterServiceInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    definitionId: String,
    expectedSourceRevision: String
  ) {
    __data = InputDict([
      "definitionId": definitionId,
      "expectedSourceRevision": expectedSourceRevision
    ])
  }

  public var definitionId: String {
    get { __data["definitionId"] }
    set { __data["definitionId"] = newValue }
  }

  public var expectedSourceRevision: String {
    get { __data["expectedSourceRevision"] }
    set { __data["expectedSourceRevision"] = newValue }
  }
}
