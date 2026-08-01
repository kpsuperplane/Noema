// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct SettingsRemoveLocalModelMutation: GraphQLMutation {
  public static let operationName: String = "SettingsRemoveLocalModel"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation SettingsRemoveLocalModel($installationId: String!) { removeLocalModel(installationId: $installationId) }"#
    ))

  public var installationId: String

  public init(installationId: String) {
    self.installationId = installationId
  }

  @_spi(Unsafe) public var __variables: Variables? { ["installationId": installationId] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("removeLocalModel", Bool.self, arguments: ["installationId": .variable("installationId")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      SettingsRemoveLocalModelMutation.Data.self
    ] }

    /// Remove one local-model installation and unreferenced model bytes.
    public var removeLocalModel: Bool { __data["removeLocalModel"] }
  }
}
