// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct StartAdapterOauthSetupMutation: GraphQLMutation {
  public static let operationName: String = "StartAdapterOauthSetup"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation StartAdapterOauthSetup($input: StartAdapterOauthSetupInput!) { startAdapterOauthSetup(input: $input) { __typename attemptId authorizationUrl expiresAtEpochSeconds } }"#
    ))

  public var input: StartAdapterOauthSetupInput

  public init(input: StartAdapterOauthSetupInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("startAdapterOauthSetup", StartAdapterOauthSetup.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      StartAdapterOauthSetupMutation.Data.self
    ] }

    /// Start provider-neutral browser OAuth for one exact native-adapter
    /// connection. The serving shell owns the callback URI and mode.
    public var startAdapterOauthSetup: StartAdapterOauthSetup { __data["startAdapterOauthSetup"] }

    /// StartAdapterOauthSetup
    ///
    /// Parent Type: `AdapterOauthSetupAttempt`
    nonisolated public struct StartAdapterOauthSetup: NoemaAPI.SelectionSet {
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
        StartAdapterOauthSetupMutation.Data.StartAdapterOauthSetup.self
      ] }

      public var attemptId: String { __data["attemptId"] }
      public var authorizationUrl: String { __data["authorizationUrl"] }
      public var expiresAtEpochSeconds: Int { __data["expiresAtEpochSeconds"] }
    }
  }
}
