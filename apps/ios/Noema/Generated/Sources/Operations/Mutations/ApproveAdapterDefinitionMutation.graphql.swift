// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct ApproveAdapterDefinitionMutation: GraphQLMutation {
  public static let operationName: String = "ApproveAdapterDefinition"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation ApproveAdapterDefinition($input: ApproveAdapterDefinitionInput!) { approveAdapterDefinition(input: $input) { __typename semanticDigest } }"#
    ))

  public var input: ApproveAdapterDefinitionInput

  public init(input: ApproveAdapterDefinitionInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("approveAdapterDefinition", ApproveAdapterDefinition.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      ApproveAdapterDefinitionMutation.Data.self
    ] }

    /// Approve one exact pending adapter definition as the local human.
    public var approveAdapterDefinition: ApproveAdapterDefinition { __data["approveAdapterDefinition"] }

    /// ApproveAdapterDefinition
    ///
    /// Parent Type: `AdapterDefinition`
    nonisolated public struct ApproveAdapterDefinition: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AdapterDefinition }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("semanticDigest", String.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        ApproveAdapterDefinitionMutation.Data.ApproveAdapterDefinition.self
      ] }

      public var semanticDigest: String { __data["semanticDigest"] }
    }
  }
}
