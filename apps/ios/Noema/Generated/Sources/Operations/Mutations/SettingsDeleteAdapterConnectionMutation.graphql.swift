// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct SettingsDeleteAdapterConnectionMutation: GraphQLMutation {
  public static let operationName: String = "SettingsDeleteAdapterConnection"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation SettingsDeleteAdapterConnection($input: DeleteAdapterConnectionInput!) { deleteAdapterConnection(input: $input) }"#
    ))

  public var input: DeleteAdapterConnectionInput

  public init(input: DeleteAdapterConnectionInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("deleteAdapterConnection", Bool.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      SettingsDeleteAdapterConnectionMutation.Data.self
    ] }

    /// Delete one exact native-adapter connection revision.
    public var deleteAdapterConnection: Bool { __data["deleteAdapterConnection"] }
  }
}
