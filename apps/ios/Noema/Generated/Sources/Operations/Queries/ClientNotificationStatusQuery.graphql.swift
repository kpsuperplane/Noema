// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct ClientNotificationStatusQuery: GraphQLQuery {
  public static let operationName: String = "ClientNotificationStatus"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"query ClientNotificationStatus { clientNotificationStatus { __typename available blocker enabled environment } }"#
    ))

  public init() {}

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.QueryRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("clientNotificationStatus", ClientNotificationStatus.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      ClientNotificationStatusQuery.Data.self
    ] }

    /// Return the paired client's native notification registration state.
    public var clientNotificationStatus: ClientNotificationStatus { __data["clientNotificationStatus"] }

    /// ClientNotificationStatus
    ///
    /// Parent Type: `ClientNotificationStatus`
    nonisolated public struct ClientNotificationStatus: NoemaAPI.SelectionSet {
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
        ClientNotificationStatusQuery.Data.ClientNotificationStatus.self
      ] }

      public var available: Bool { __data["available"] }
      public var blocker: String? { __data["blocker"] }
      public var enabled: Bool { __data["enabled"] }
      public var environment: GraphQLEnum<NoemaAPI.ApnsEnvironment>? { __data["environment"] }
    }
  }
}
