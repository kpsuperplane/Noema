// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct ClientsQuery: GraphQLQuery {
  public static let operationName: String = "Clients"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"query Clients { clients { __typename clientId displayName createdAt revokedAt isCurrent } }"#
    ))

  public init() {}

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.QueryRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("clients", [Client].self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      ClientsQuery.Data.self
    ] }

    /// List all paired clients, including retained revoked rows.
    public var clients: [Client] { __data["clients"] }

    /// Client
    ///
    /// Parent Type: `Client`
    nonisolated public struct Client: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.Client }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("clientId", String.self),
        .field("displayName", String.self),
        .field("createdAt", String.self),
        .field("revokedAt", String?.self),
        .field("isCurrent", Bool.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        ClientsQuery.Data.Client.self
      ] }

      /// Opaque identifier carried by the client bearer credential.
      public var clientId: String { __data["clientId"] }
      /// Human-visible client label.
      public var displayName: String { __data["displayName"] }
      /// Creation timestamp.
      public var createdAt: String { __data["createdAt"] }
      /// Revocation timestamp, when this credential is no longer accepted.
      public var revokedAt: String? { __data["revokedAt"] }
      /// Whether this row represents the credential used by the current request.
      public var isCurrent: Bool { __data["isCurrent"] }
    }
  }
}
