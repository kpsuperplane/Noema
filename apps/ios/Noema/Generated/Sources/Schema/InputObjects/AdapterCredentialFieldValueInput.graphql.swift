// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// One write-only credential field value.
nonisolated public struct AdapterCredentialFieldValueInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    fieldId: String,
    value: String
  ) {
    __data = InputDict([
      "fieldId": fieldId,
      "value": value
    ])
  }

  public var fieldId: String {
    get { __data["fieldId"] }
    set { __data["fieldId"] = newValue }
  }

  public var value: String {
    get { __data["value"] }
    set { __data["value"] = newValue }
  }
}
