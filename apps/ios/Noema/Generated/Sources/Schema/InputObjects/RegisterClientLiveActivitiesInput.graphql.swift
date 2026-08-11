// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

nonisolated public struct RegisterClientLiveActivitiesInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    pushToStartToken: String,
    environment: GraphQLEnum<ApnsEnvironment>,
    activeActivityIds: [String]
  ) {
    __data = InputDict([
      "pushToStartToken": pushToStartToken,
      "environment": environment,
      "activeActivityIds": activeActivityIds
    ])
  }

  public var pushToStartToken: String {
    get { __data["pushToStartToken"] }
    set { __data["pushToStartToken"] = newValue }
  }

  public var environment: GraphQLEnum<ApnsEnvironment> {
    get { __data["environment"] }
    set { __data["environment"] = newValue }
  }

  public var activeActivityIds: [String] {
    get { __data["activeActivityIds"] }
    set { __data["activeActivityIds"] = newValue }
  }
}
