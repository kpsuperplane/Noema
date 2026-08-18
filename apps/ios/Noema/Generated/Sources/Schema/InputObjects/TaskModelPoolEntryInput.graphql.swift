// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Input for replacing one human-controlled executor pool entry.
nonisolated public struct TaskModelPoolEntryInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    complexity: GraphQLEnum<TaskComplexity>,
    label: GraphQLNullable<String> = nil,
    providerKind: String,
    providerAccountId: String,
    selectionMode: GraphQLEnum<ModelPreferenceSelectionMode>,
    modelProfile: GraphQLNullable<String> = nil,
    reasoningEffort: GraphQLNullable<GraphQLEnum<ReasoningEffort>> = nil,
    fastMode: Bool,
    enabled: Bool,
    sortOrder: Int32
  ) {
    __data = InputDict([
      "complexity": complexity,
      "label": label,
      "providerKind": providerKind,
      "providerAccountId": providerAccountId,
      "selectionMode": selectionMode,
      "modelProfile": modelProfile,
      "reasoningEffort": reasoningEffort,
      "fastMode": fastMode,
      "enabled": enabled,
      "sortOrder": sortOrder
    ])
  }

  /// Complexity tier exposed to the primary agent.
  public var complexity: GraphQLEnum<TaskComplexity> {
    get { __data["complexity"] }
    set { __data["complexity"] = newValue }
  }

  /// Optional human-facing label.
  public var label: GraphQLNullable<String> {
    get { __data["label"] }
    set { __data["label"] = newValue }
  }

  /// Provider family for this entry.
  public var providerKind: String {
    get { __data["providerKind"] }
    set { __data["providerKind"] = newValue }
  }

  /// Provider account owning the model profile.
  public var providerAccountId: String {
    get { __data["providerAccountId"] }
    set { __data["providerAccountId"] = newValue }
  }

  /// Whether Noema or the human chooses the concrete model.
  public var selectionMode: GraphQLEnum<ModelPreferenceSelectionMode> {
    get { __data["selectionMode"] }
    set { __data["selectionMode"] = newValue }
  }

  /// Exact provider model/profile for an explicit selection.
  public var modelProfile: GraphQLNullable<String> {
    get { __data["modelProfile"] }
    set { __data["modelProfile"] = newValue }
  }

  /// Optional reasoning effort.
  public var reasoningEffort: GraphQLNullable<GraphQLEnum<ReasoningEffort>> {
    get { __data["reasoningEffort"] }
    set { __data["reasoningEffort"] = newValue }
  }

  /// Whether this preference requests faster service.
  public var fastMode: Bool {
    get { __data["fastMode"] }
    set { __data["fastMode"] = newValue }
  }

  /// Whether this entry can be selected for new tasks.
  public var enabled: Bool {
    get { __data["enabled"] }
    set { __data["enabled"] = newValue }
  }

  /// Human-controlled ordering within its tier.
  public var sortOrder: Int32 {
    get { __data["sortOrder"] }
    set { __data["sortOrder"] = newValue }
  }
}
