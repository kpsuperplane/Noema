// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct SettingsCancelAdapterDefinitionMutation: GraphQLMutation {
  public static let operationName: String = "SettingsCancelAdapterDefinition"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation SettingsCancelAdapterDefinition($input: CancelAdapterDefinitionInput!) { cancelAdapterDefinition(input: $input) }"#
    ))

  public var input: CancelAdapterDefinitionInput

  public init(input: CancelAdapterDefinitionInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("cancelAdapterDefinition", Bool.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      SettingsCancelAdapterDefinitionMutation.Data.self
    ] }

    /// Cancel one exact current pending adapter definition as the local human.
    public var cancelAdapterDefinition: Bool { __data["cancelAdapterDefinition"] }
  }
}
