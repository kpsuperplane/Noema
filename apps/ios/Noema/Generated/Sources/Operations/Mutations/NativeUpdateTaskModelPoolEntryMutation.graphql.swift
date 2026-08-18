// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct NativeUpdateTaskModelPoolEntryMutation: GraphQLMutation {
  public static let operationName: String = "NativeUpdateTaskModelPoolEntry"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation NativeUpdateTaskModelPoolEntry($poolEntryId: String!, $input: TaskModelPoolEntryInput!) { updateTaskModelPoolEntry(poolEntryId: $poolEntryId, input: $input) { __typename poolEntryId complexity label providerKind providerAccountId modelProfile reasoningEffort selectionMode fastMode enabled sortOrder createdAt updatedAt } }"#
    ))

  public var poolEntryId: String
  public var input: TaskModelPoolEntryInput

  public init(
    poolEntryId: String,
    input: TaskModelPoolEntryInput
  ) {
    self.poolEntryId = poolEntryId
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { [
    "poolEntryId": poolEntryId,
    "input": input
  ] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("updateTaskModelPoolEntry", UpdateTaskModelPoolEntry.self, arguments: [
        "poolEntryId": .variable("poolEntryId"),
        "input": .variable("input")
      ]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      NativeUpdateTaskModelPoolEntryMutation.Data.self
    ] }

    /// Replace one human-controlled executor model-pool entry.
    public var updateTaskModelPoolEntry: UpdateTaskModelPoolEntry { __data["updateTaskModelPoolEntry"] }

    /// UpdateTaskModelPoolEntry
    ///
    /// Parent Type: `TaskModelPoolEntry`
    nonisolated public struct UpdateTaskModelPoolEntry: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskModelPoolEntry }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("poolEntryId", String.self),
        .field("complexity", GraphQLEnum<NoemaAPI.TaskComplexity>.self),
        .field("label", String?.self),
        .field("providerKind", String.self),
        .field("providerAccountId", String.self),
        .field("modelProfile", String?.self),
        .field("reasoningEffort", GraphQLEnum<NoemaAPI.ReasoningEffort>?.self),
        .field("selectionMode", GraphQLEnum<NoemaAPI.ModelPreferenceSelectionMode>.self),
        .field("fastMode", Bool.self),
        .field("enabled", Bool.self),
        .field("sortOrder", Int.self),
        .field("createdAt", String.self),
        .field("updatedAt", String.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        NativeUpdateTaskModelPoolEntryMutation.Data.UpdateTaskModelPoolEntry.self
      ] }

      /// Stable pool entry id.
      public var poolEntryId: String { __data["poolEntryId"] }
      /// Complexity tier exposed to the primary agent.
      public var complexity: GraphQLEnum<NoemaAPI.TaskComplexity> { __data["complexity"] }
      /// Optional human-facing label.
      public var label: String? { __data["label"] }
      /// Provider family for this entry.
      public var providerKind: String { __data["providerKind"] }
      /// Provider account owning the model profile.
      public var providerAccountId: String { __data["providerAccountId"] }
      /// Exact provider model/profile for an explicit preference.
      public var modelProfile: String? { __data["modelProfile"] }
      /// Optional explicit reasoning effort.
      public var reasoningEffort: GraphQLEnum<NoemaAPI.ReasoningEffort>? { __data["reasoningEffort"] }
      /// Whether Noema or the human chooses the concrete model.
      public var selectionMode: GraphQLEnum<NoemaAPI.ModelPreferenceSelectionMode> { __data["selectionMode"] }
      /// Whether this preference requests faster service.
      public var fastMode: Bool { __data["fastMode"] }
      /// Whether this entry can be selected for new tasks.
      public var enabled: Bool { __data["enabled"] }
      /// Human-controlled ordering within its tier.
      public var sortOrder: Int { __data["sortOrder"] }
      /// Creation timestamp.
      public var createdAt: String { __data["createdAt"] }
      /// Last update timestamp.
      public var updatedAt: String { __data["updatedAt"] }
    }
  }
}
