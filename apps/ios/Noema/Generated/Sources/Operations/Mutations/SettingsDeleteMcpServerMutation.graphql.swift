// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct SettingsDeleteMcpServerMutation: GraphQLMutation {
  public static let operationName: String = "SettingsDeleteMcpServer"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation SettingsDeleteMcpServer($mcpServerId: String!) { deleteMcpServer(mcpServerId: $mcpServerId) }"#
    ))

  public var mcpServerId: String

  public init(mcpServerId: String) {
    self.mcpServerId = mcpServerId
  }

  @_spi(Unsafe) public var __variables: Variables? { ["mcpServerId": mcpServerId] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("deleteMcpServer", Bool.self, arguments: ["mcpServerId": .variable("mcpServerId")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      SettingsDeleteMcpServerMutation.Data.self
    ] }

    /// Delete an MCP server and its setup secrets.
    public var deleteMcpServer: Bool { __data["deleteMcpServer"] }
  }
}
