// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct SettingsSaveDefaultModelPreferenceMutation: GraphQLMutation {
  public static let operationName: String = "SettingsSaveDefaultModelPreference"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation SettingsSaveDefaultModelPreference($input: SaveDefaultModelPreferenceInput!) { saveDefaultModelPreference(input: $input) { __typename providerKind providerAccountId selectionMode modelProfile reasoningEffort } }"#
    ))

  public var input: SaveDefaultModelPreferenceInput

  public init(input: SaveDefaultModelPreferenceInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("saveDefaultModelPreference", SaveDefaultModelPreference.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      SettingsSaveDefaultModelPreferenceMutation.Data.self
    ] }

    /// Save Noema's system model default.
    public var saveDefaultModelPreference: SaveDefaultModelPreference { __data["saveDefaultModelPreference"] }

    /// SaveDefaultModelPreference
    ///
    /// Parent Type: `DefaultModelPreference`
    nonisolated public struct SaveDefaultModelPreference: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.DefaultModelPreference }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("providerKind", String.self),
        .field("providerAccountId", String.self),
        .field("selectionMode", GraphQLEnum<NoemaAPI.ModelPreferenceSelectionMode>.self),
        .field("modelProfile", String?.self),
        .field("reasoningEffort", String?.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SettingsSaveDefaultModelPreferenceMutation.Data.SaveDefaultModelPreference.self
      ] }

      /// Provider family used for new workloads.
      public var providerKind: String { __data["providerKind"] }
      /// Provider account used for new workloads.
      public var providerAccountId: String { __data["providerAccountId"] }
      /// Provider-specific model profile.
      public var selectionMode: GraphQLEnum<NoemaAPI.ModelPreferenceSelectionMode> { __data["selectionMode"] }
      public var modelProfile: String? { __data["modelProfile"] }
      /// Optional provider-specific reasoning effort.
      public var reasoningEffort: String? { __data["reasoningEffort"] }
    }
  }
}
