// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Delete one exact filesystem-canonical adapter connection revision.
nonisolated public struct DeleteAdapterConnectionInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    connectionId: String,
    expectedConnectionRevision: Int32
  ) {
    __data = InputDict([
      "connectionId": connectionId,
      "expectedConnectionRevision": expectedConnectionRevision
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
}
