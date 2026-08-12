// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct SettingsDeleteAdapterOauthApplicationMutation: GraphQLMutation {
  public static let operationName: String = "SettingsDeleteAdapterOauthApplication"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation SettingsDeleteAdapterOauthApplication($input: DeleteAdapterOauthApplicationInput!) { deleteAdapterOauthApplication(input: $input) }"#
    ))

  public var input: DeleteAdapterOauthApplicationInput

  public init(input: DeleteAdapterOauthApplicationInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("deleteAdapterOauthApplication", Bool.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      SettingsDeleteAdapterOauthApplicationMutation.Data.self
    ] }

    /// Delete one OAuth application without dependent grants.
    public var deleteAdapterOauthApplication: Bool { __data["deleteAdapterOauthApplication"] }
  }
}
