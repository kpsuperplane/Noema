// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct SettingsSaveAdapterOauthGrantLabelMutation: GraphQLMutation {
  public static let operationName: String = "SettingsSaveAdapterOauthGrantLabel"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation SettingsSaveAdapterOauthGrantLabel($input: SaveAdapterOauthGrantLabelInput!) { saveAdapterOauthGrantLabel(input: $input) { __typename grantId accountLabel authorityRevision } }"#
    ))

  public var input: SaveAdapterOauthGrantLabelInput

  public init(input: SaveAdapterOauthGrantLabelInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("saveAdapterOauthGrantLabel", SaveAdapterOauthGrantLabel.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      SettingsSaveAdapterOauthGrantLabelMutation.Data.self
    ] }

    /// Save a label for an account without stable provider identity.
    public var saveAdapterOauthGrantLabel: SaveAdapterOauthGrantLabel { __data["saveAdapterOauthGrantLabel"] }

    /// SaveAdapterOauthGrantLabel
    ///
    /// Parent Type: `AdapterAuthorizationGrant`
    nonisolated public struct SaveAdapterOauthGrantLabel: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AdapterAuthorizationGrant }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("grantId", String.self),
        .field("accountLabel", String?.self),
        .field("authorityRevision", Int.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SettingsSaveAdapterOauthGrantLabelMutation.Data.SaveAdapterOauthGrantLabel.self
      ] }

      public var grantId: String { __data["grantId"] }
      public var accountLabel: String? { __data["accountLabel"] }
      public var authorityRevision: Int { __data["authorityRevision"] }
    }
  }
}
