// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Suspend or resume one exact API connection.
nonisolated public struct SetAdapterConnectionActiveInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    connectionId: String,
    expectedConnectionRevision: Int32,
    active: Bool
  ) {
    __data = InputDict([
      "connectionId": connectionId,
      "expectedConnectionRevision": expectedConnectionRevision,
      "active": active
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

  public var active: Bool {
    get { __data["active"] }
    set { __data["active"] = newValue }
  }
}
