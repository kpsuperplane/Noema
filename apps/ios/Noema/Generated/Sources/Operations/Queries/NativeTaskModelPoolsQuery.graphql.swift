// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct NativeTaskModelPoolsQuery: GraphQLQuery {
  public static let operationName: String = "NativeTaskModelPools"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"query NativeTaskModelPools { taskModelPools { __typename poolEntryId complexity label providerKind providerAccountId modelProfile reasoningEffort selectionMode fastMode sortOrder createdAt updatedAt } }"#
    ))

  public init() {}

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.QueryRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("taskModelPools", [TaskModelPool].self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      NativeTaskModelPoolsQuery.Data.self
    ] }

    /// Return human-controlled executor model-pool entries.
    public var taskModelPools: [TaskModelPool] { __data["taskModelPools"] }

    /// TaskModelPool
    ///
    /// Parent Type: `TaskModelPoolEntry`
    nonisolated public struct TaskModelPool: NoemaAPI.SelectionSet {
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
        .field("sortOrder", Int.self),
        .field("createdAt", String.self),
        .field("updatedAt", String.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        NativeTaskModelPoolsQuery.Data.TaskModelPool.self
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
      /// Human-controlled ordering within its tier.
      public var sortOrder: Int { __data["sortOrder"] }
      /// Creation timestamp.
      public var createdAt: String { __data["createdAt"] }
      /// Last update timestamp.
      public var updatedAt: String { __data["updatedAt"] }
    }
  }
}
