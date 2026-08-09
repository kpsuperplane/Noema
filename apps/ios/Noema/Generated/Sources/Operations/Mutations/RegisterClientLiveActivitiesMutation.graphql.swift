// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct RegisterClientLiveActivitiesMutation: GraphQLMutation {
  public static let operationName: String = "RegisterClientLiveActivities"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation RegisterClientLiveActivities($input: RegisterClientLiveActivitiesInput!) { registerClientLiveActivities(input: $input) { __typename available blocker enabled registered environment } }"#
    ))

  public var input: RegisterClientLiveActivitiesInput

  public init(input: RegisterClientLiveActivitiesInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("registerClientLiveActivities", RegisterClientLiveActivities.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      RegisterClientLiveActivitiesMutation.Data.self
    ] }

    public var registerClientLiveActivities: RegisterClientLiveActivities { __data["registerClientLiveActivities"] }

    /// RegisterClientLiveActivities
    ///
    /// Parent Type: `ClientLiveActivityStatus`
    nonisolated public struct RegisterClientLiveActivities: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.ClientLiveActivityStatus }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("available", Bool.self),
        .field("blocker", String?.self),
        .field("enabled", Bool.self),
        .field("registered", Bool.self),
        .field("environment", GraphQLEnum<NoemaAPI.ApnsEnvironment>?.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        RegisterClientLiveActivitiesMutation.Data.RegisterClientLiveActivities.self
      ] }

      public var available: Bool { __data["available"] }
      public var blocker: String? { __data["blocker"] }
      public var enabled: Bool { __data["enabled"] }
      public var registered: Bool { __data["registered"] }
      public var environment: GraphQLEnum<NoemaAPI.ApnsEnvironment>? { __data["environment"] }
    }
  }
}
