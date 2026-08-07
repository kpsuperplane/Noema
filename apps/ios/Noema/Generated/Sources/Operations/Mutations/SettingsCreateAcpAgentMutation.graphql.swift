// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct SettingsCreateAcpAgentMutation: GraphQLMutation {
  public static let operationName: String = "SettingsCreateAcpAgent"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation SettingsCreateAcpAgent($input: CreateAcpAgentInput!) { createAcpAgent(input: $input) { __typename ...SettingsAcpAgentFields } }"#,
      fragments: [SettingsAcpAgentFields.self]
    ))

  public var input: CreateAcpAgentInput

  public init(input: CreateAcpAgentInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("createAcpAgent", CreateAcpAgent.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      SettingsCreateAcpAgentMutation.Data.self
    ] }

    /// Register a trusted operator-managed ACP executable.
    public var createAcpAgent: CreateAcpAgent { __data["createAcpAgent"] }

    /// CreateAcpAgent
    ///
    /// Parent Type: `AcpAgent`
    nonisolated public struct CreateAcpAgent: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AcpAgent }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .fragment(SettingsAcpAgentFields.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SettingsCreateAcpAgentMutation.Data.CreateAcpAgent.self,
        SettingsAcpAgentFields.self
      ] }

      public var agentId: String { __data["agentId"] }
      public var displayName: String { __data["displayName"] }
      public var command: String { __data["command"] }
      public var arguments: [String] { __data["arguments"] }
      public var enabled: Bool { __data["enabled"] }
      public var authStatus: GraphQLEnum<NoemaAPI.AcpAgentAuthStatus> { __data["authStatus"] }
      public var healthStatus: GraphQLEnum<NoemaAPI.AcpAgentHealthStatus> { __data["healthStatus"] }
      public var implementationName: String? { __data["implementationName"] }
      public var implementationVersion: String? { __data["implementationVersion"] }
      public var capabilities: NoemaAPI.JSON { __data["capabilities"] }
      public var connectionRevision: Int { __data["connectionRevision"] }
      public var lastError: String? { __data["lastError"] }

      public struct Fragments: FragmentContainer {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        public var settingsAcpAgentFields: SettingsAcpAgentFields { _toFragment() }
      }
    }
  }
}
