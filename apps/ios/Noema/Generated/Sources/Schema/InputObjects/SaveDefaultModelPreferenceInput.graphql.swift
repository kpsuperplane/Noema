// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Save Noema's system model default without changing existing explicit selections.
nonisolated public struct SaveDefaultModelPreferenceInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    providerKind: String,
    providerAccountId: String,
    selectionMode: GraphQLEnum<ModelPreferenceSelectionMode>,
    modelProfile: GraphQLNullable<String> = nil,
    reasoningEffort: GraphQLNullable<GraphQLEnum<ReasoningEffort>> = nil,
    fastMode: Bool
  ) {
    __data = InputDict([
      "providerKind": providerKind,
      "providerAccountId": providerAccountId,
      "selectionMode": selectionMode,
      "modelProfile": modelProfile,
      "reasoningEffort": reasoningEffort,
      "fastMode": fastMode
    ])
  }

  /// Provider family used for new workloads.
  public var providerKind: String {
    get { __data["providerKind"] }
    set { __data["providerKind"] = newValue }
  }

  /// Provider account used for new workloads.
  public var providerAccountId: String {
    get { __data["providerAccountId"] }
    set { __data["providerAccountId"] = newValue }
  }

  /// Whether Noema or the human chooses the concrete model.
  public var selectionMode: GraphQLEnum<ModelPreferenceSelectionMode> {
    get { __data["selectionMode"] }
    set { __data["selectionMode"] = newValue }
  }

  /// Provider-specific model profile for an explicit selection.
  public var modelProfile: GraphQLNullable<String> {
    get { __data["modelProfile"] }
    set { __data["modelProfile"] = newValue }
  }

  /// Optional provider-specific reasoning effort.
  public var reasoningEffort: GraphQLNullable<GraphQLEnum<ReasoningEffort>> {
    get { __data["reasoningEffort"] }
    set { __data["reasoningEffort"] = newValue }
  }

  /// Whether this preference requests faster service.
  public var fastMode: Bool {
    get { __data["fastMode"] }
    set { __data["fastMode"] = newValue }
  }
}
