// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct SettingsUpdateAcpAgentMutation: GraphQLMutation {
  public static let operationName: String = "SettingsUpdateAcpAgent"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation SettingsUpdateAcpAgent($input: UpdateAcpAgentInput!) { updateAcpAgent(input: $input) { __typename ...SettingsAcpAgentFields } }"#,
      fragments: [SettingsAcpAgentFields.self]
    ))

  public var input: UpdateAcpAgentInput

  public init(input: UpdateAcpAgentInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("updateAcpAgent", UpdateAcpAgent.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      SettingsUpdateAcpAgentMutation.Data.self
    ] }

    /// Replace one ACP launch configuration or disable it.
    public var updateAcpAgent: UpdateAcpAgent { __data["updateAcpAgent"] }

    /// UpdateAcpAgent
    ///
    /// Parent Type: `AcpAgent`
    nonisolated public struct UpdateAcpAgent: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AcpAgent }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .fragment(SettingsAcpAgentFields.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SettingsUpdateAcpAgentMutation.Data.UpdateAcpAgent.self,
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
