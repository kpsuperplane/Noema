// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct SaveWebFetchSummarizerPreferenceMutation: GraphQLMutation {
  public static let operationName: String = "SaveWebFetchSummarizerPreference"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation SaveWebFetchSummarizerPreference($input: SaveWebFetchSummarizerPreferenceInput!) { saveWebFetchSummarizerPreference(input: $input) { __typename providerKind providerAccountId modelProfile reasoningEffort selectionMode } }"#
    ))

  public var input: SaveWebFetchSummarizerPreferenceInput

  public init(input: SaveWebFetchSummarizerPreferenceInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("saveWebFetchSummarizerPreference", SaveWebFetchSummarizerPreference.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      SaveWebFetchSummarizerPreferenceMutation.Data.self
    ] }

    /// Save the web fetch summarizer model/provider preference.
    public var saveWebFetchSummarizerPreference: SaveWebFetchSummarizerPreference { __data["saveWebFetchSummarizerPreference"] }

    /// SaveWebFetchSummarizerPreference
    ///
    /// Parent Type: `AgentModelPreference`
    nonisolated public struct SaveWebFetchSummarizerPreference: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AgentModelPreference }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("providerKind", String.self),
        .field("providerAccountId", String.self),
        .field("modelProfile", String?.self),
        .field("reasoningEffort", GraphQLEnum<NoemaAPI.ReasoningEffort>?.self),
        .field("selectionMode", GraphQLEnum<NoemaAPI.ModelPreferenceSelectionMode>.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SaveWebFetchSummarizerPreferenceMutation.Data.SaveWebFetchSummarizerPreference.self
      ] }

      /// Provider kind selected for this agent.
      public var providerKind: String { __data["providerKind"] }
      /// Provider account id selected for this agent.
      public var providerAccountId: String { __data["providerAccountId"] }
      /// Provider-specific model id or profile id.
      public var modelProfile: String? { __data["modelProfile"] }
      /// Optional explicit reasoning effort for reasoning-capable model profiles.
      public var reasoningEffort: GraphQLEnum<NoemaAPI.ReasoningEffort>? { __data["reasoningEffort"] }
      /// Whether Noema or the human chooses the concrete model.
      public var selectionMode: GraphQLEnum<NoemaAPI.ModelPreferenceSelectionMode> { __data["selectionMode"] }
    }
  }
}
