// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

nonisolated public struct RegisterClientLiveActivityUpdateInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    activityId: String,
    updateToken: String
  ) {
    __data = InputDict([
      "activityId": activityId,
      "updateToken": updateToken
    ])
  }

  public var activityId: String {
    get { __data["activityId"] }
    set { __data["activityId"] = newValue }
  }

  public var updateToken: String {
    get { __data["updateToken"] }
    set { __data["updateToken"] = newValue }
  }
}
