// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct StartAdapterAuthenticationMutation: GraphQLMutation {
  public static let operationName: String = "StartAdapterAuthentication"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation StartAdapterAuthentication($input: StartAdapterAuthenticationInput!) { startAdapterAuthentication(input: $input) { __typename attemptId authorizationUrl expiresAtEpochSeconds } }"#
    ))

  public var input: StartAdapterAuthenticationInput

  public init(input: StartAdapterAuthenticationInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("startAdapterAuthentication", StartAdapterAuthentication.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      StartAdapterAuthenticationMutation.Data.self
    ] }

    /// Start browser OAuth for one exact API adapter authentication interruption.
    public var startAdapterAuthentication: StartAdapterAuthentication { __data["startAdapterAuthentication"] }

    /// StartAdapterAuthentication
    ///
    /// Parent Type: `AdapterOauthSetupAttempt`
    nonisolated public struct StartAdapterAuthentication: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AdapterOauthSetupAttempt }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("attemptId", String.self),
        .field("authorizationUrl", String.self),
        .field("expiresAtEpochSeconds", Int.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        StartAdapterAuthenticationMutation.Data.StartAdapterAuthentication.self
      ] }

      public var attemptId: String { __data["attemptId"] }
      public var authorizationUrl: String { __data["authorizationUrl"] }
      public var expiresAtEpochSeconds: Int { __data["expiresAtEpochSeconds"] }
    }
  }
}
