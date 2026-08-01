// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct SettingsDeleteProviderAccountMutation: GraphQLMutation {
  public static let operationName: String = "SettingsDeleteProviderAccount"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation SettingsDeleteProviderAccount($input: DeleteProviderAccountInput!) { deleteProviderAccount(input: $input) }"#
    ))

  public var input: DeleteProviderAccountInput

  public init(input: DeleteProviderAccountInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("deleteProviderAccount", Bool.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      SettingsDeleteProviderAccountMutation.Data.self
    ] }

    /// Hard-delete a user-managed provider account.
    public var deleteProviderAccount: Bool { __data["deleteProviderAccount"] }
  }
}
