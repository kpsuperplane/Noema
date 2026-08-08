// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

nonisolated public struct RegisterClientNotificationsInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    deviceToken: String,
    environment: GraphQLEnum<ApnsEnvironment>
  ) {
    __data = InputDict([
      "deviceToken": deviceToken,
      "environment": environment
    ])
  }

  public var deviceToken: String {
    get { __data["deviceToken"] }
    set { __data["deviceToken"] = newValue }
  }

  public var environment: GraphQLEnum<ApnsEnvironment> {
    get { __data["environment"] }
    set { __data["environment"] = newValue }
  }
}
