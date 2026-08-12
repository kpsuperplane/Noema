// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct SettingsAdapterOauthAttemptQuery: GraphQLQuery {
  public static let operationName: String = "SettingsAdapterOauthAttempt"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"query SettingsAdapterOauthAttempt($attemptId: String!) { adapterOauthAttempt(attemptId: $attemptId) { __typename attemptId semanticDigest grantId grantRevision status } }"#
    ))

  public var attemptId: String

  public init(attemptId: String) {
    self.attemptId = attemptId
  }

  @_spi(Unsafe) public var __variables: Variables? { ["attemptId": attemptId] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.QueryRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("adapterOauthAttempt", AdapterOauthAttempt?.self, arguments: ["attemptId": .variable("attemptId")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      SettingsAdapterOauthAttemptQuery.Data.self
    ] }

    /// Return the latest process-local state for one OAuth attempt.
    public var adapterOauthAttempt: AdapterOauthAttempt? { __data["adapterOauthAttempt"] }

    /// AdapterOauthAttempt
    ///
    /// Parent Type: `AdapterOauthAttemptEvent`
    nonisolated public struct AdapterOauthAttempt: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AdapterOauthAttemptEvent }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("attemptId", String.self),
        .field("semanticDigest", String?.self),
        .field("grantId", String?.self),
        .field("grantRevision", Int?.self),
        .field("status", String.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SettingsAdapterOauthAttemptQuery.Data.AdapterOauthAttempt.self
      ] }

      public var attemptId: String { __data["attemptId"] }
      public var semanticDigest: String? { __data["semanticDigest"] }
      public var grantId: String? { __data["grantId"] }
      public var grantRevision: Int? { __data["grantRevision"] }
      public var status: String { __data["status"] }
    }
  }
}
