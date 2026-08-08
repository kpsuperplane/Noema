// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct RegisterClientNotificationsMutation: GraphQLMutation {
  public static let operationName: String = "RegisterClientNotifications"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation RegisterClientNotifications($input: RegisterClientNotificationsInput!) { registerClientNotifications(input: $input) { __typename available blocker enabled environment } }"#
    ))

  public var input: RegisterClientNotificationsInput

  public init(input: RegisterClientNotificationsInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("registerClientNotifications", RegisterClientNotifications.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      RegisterClientNotificationsMutation.Data.self
    ] }

    public var registerClientNotifications: RegisterClientNotifications { __data["registerClientNotifications"] }

    /// RegisterClientNotifications
    ///
    /// Parent Type: `ClientNotificationStatus`
    nonisolated public struct RegisterClientNotifications: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.ClientNotificationStatus }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("available", Bool.self),
        .field("blocker", String?.self),
        .field("enabled", Bool.self),
        .field("environment", GraphQLEnum<NoemaAPI.ApnsEnvironment>?.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        RegisterClientNotificationsMutation.Data.RegisterClientNotifications.self
      ] }

      public var available: Bool { __data["available"] }
      public var blocker: String? { __data["blocker"] }
      public var enabled: Bool { __data["enabled"] }
      public var environment: GraphQLEnum<NoemaAPI.ApnsEnvironment>? { __data["environment"] }
    }
  }
}
