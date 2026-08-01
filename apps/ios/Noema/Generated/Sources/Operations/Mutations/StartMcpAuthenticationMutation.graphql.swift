// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct StartMcpAuthenticationMutation: GraphQLMutation {
  public static let operationName: String = "StartMcpAuthentication"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation StartMcpAuthentication($input: StartMcpAuthenticationInput!) { startMcpAuthentication(input: $input) { __typename attemptId status authorizationUrl errorMessage } }"#
    ))

  public var input: StartMcpAuthenticationInput

  public init(input: StartMcpAuthenticationInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("startMcpAuthentication", StartMcpAuthentication.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      StartMcpAuthenticationMutation.Data.self
    ] }

    /// Start browser OAuth for one exact MCP authentication interruption.
    public var startMcpAuthentication: StartMcpAuthentication { __data["startMcpAuthentication"] }

    /// StartMcpAuthentication
    ///
    /// Parent Type: `McpOAuthSetupAttempt`
    nonisolated public struct StartMcpAuthentication: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.McpOAuthSetupAttempt }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("attemptId", String.self),
        .field("status", String.self),
        .field("authorizationUrl", String?.self),
        .field("errorMessage", String?.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        StartMcpAuthenticationMutation.Data.StartMcpAuthentication.self
      ] }

      /// Short-lived attempt id.
      public var attemptId: String { __data["attemptId"] }
      /// Current attempt status.
      public var status: String { __data["status"] }
      /// Authorization URL to open in the user's browser.
      public var authorizationUrl: String? { __data["authorizationUrl"] }
      /// Non-secret UI-safe failure message.
      public var errorMessage: String? { __data["errorMessage"] }
    }
  }
}
