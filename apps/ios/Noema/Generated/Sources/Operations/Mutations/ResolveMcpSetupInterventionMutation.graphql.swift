// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct ResolveMcpSetupInterventionMutation: GraphQLMutation {
  public static let operationName: String = "ResolveMcpSetupIntervention"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation ResolveMcpSetupIntervention($input: ResolveMcpSetupInterventionInput!) { resolveMcpSetupIntervention(input: $input) }"#
    ))

  public var input: ResolveMcpSetupInterventionInput

  public init(input: ResolveMcpSetupInterventionInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("resolveMcpSetupIntervention", Bool.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      ResolveMcpSetupInterventionMutation.Data.self
    ] }

    /// Resolve one chat-driven MCP setup after its exact connection is configured.
    public var resolveMcpSetupIntervention: Bool { __data["resolveMcpSetupIntervention"] }
  }
}
