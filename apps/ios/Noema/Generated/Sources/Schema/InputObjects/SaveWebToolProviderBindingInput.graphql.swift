// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

nonisolated public struct SaveWebToolProviderBindingInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    toolName: String,
    capabilityId: String,
    providerAccountId: String
  ) {
    __data = InputDict([
      "toolName": toolName,
      "capabilityId": capabilityId,
      "providerAccountId": providerAccountId
    ])
  }

  public var toolName: String {
    get { __data["toolName"] }
    set { __data["toolName"] = newValue }
  }

  public var capabilityId: String {
    get { __data["capabilityId"] }
    set { __data["capabilityId"] = newValue }
  }

  public var providerAccountId: String {
    get { __data["providerAccountId"] }
    set { __data["providerAccountId"] = newValue }
  }
}
