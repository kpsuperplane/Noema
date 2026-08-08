// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct ClientNotificationPresenceSubscription: GraphQLSubscription {
  public static let operationName: String = "ClientNotificationPresence"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"subscription ClientNotificationPresence { clientNotificationPresence { __typename clientId ready } }"#
    ))

  public init() {}

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.SubscriptionRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("clientNotificationPresence", ClientNotificationPresence.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      ClientNotificationPresenceSubscription.Data.self
    ] }

    /// Keep the current paired native client visible while its Chat is focused.
    public var clientNotificationPresence: ClientNotificationPresence { __data["clientNotificationPresence"] }

    /// ClientNotificationPresence
    ///
    /// Parent Type: `ClientNotificationPresenceEvent`
    nonisolated public struct ClientNotificationPresence: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.ClientNotificationPresenceEvent }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("clientId", String.self),
        .field("ready", Bool.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        ClientNotificationPresenceSubscription.Data.ClientNotificationPresence.self
      ] }

      public var clientId: String { __data["clientId"] }
      public var ready: Bool { __data["ready"] }
    }
  }
}
