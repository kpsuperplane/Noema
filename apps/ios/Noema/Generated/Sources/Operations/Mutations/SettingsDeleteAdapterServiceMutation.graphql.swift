// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct SettingsDeleteAdapterServiceMutation: GraphQLMutation {
  public static let operationName: String = "SettingsDeleteAdapterService"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation SettingsDeleteAdapterService($input: DeleteAdapterServiceInput!) { deleteAdapterService(input: $input) }"#
    ))

  public var input: DeleteAdapterServiceInput

  public init(input: DeleteAdapterServiceInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("deleteAdapterService", Bool.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      SettingsDeleteAdapterServiceMutation.Data.self
    ] }

    /// Delete one adapter service after all of its connections are removed.
    public var deleteAdapterService: Bool { __data["deleteAdapterService"] }
  }
}
