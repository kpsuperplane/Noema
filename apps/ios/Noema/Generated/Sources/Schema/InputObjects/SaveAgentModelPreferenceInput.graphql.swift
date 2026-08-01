// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Input for saving an agent model preference.
nonisolated public struct SaveAgentModelPreferenceInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    agentId: String,
    providerAccountId: String,
    selectionMode: GraphQLEnum<ModelPreferenceSelectionMode>,
    modelProfile: GraphQLNullable<String> = nil,
    reasoningEffort: GraphQLNullable<GraphQLEnum<ReasoningEffort>> = nil
  ) {
    __data = InputDict([
      "agentId": agentId,
      "providerAccountId": providerAccountId,
      "selectionMode": selectionMode,
      "modelProfile": modelProfile,
      "reasoningEffort": reasoningEffort
    ])
  }

  /// Agent to update.
  public var agentId: String {
    get { __data["agentId"] }
    set { __data["agentId"] = newValue }
  }

  /// Provider account id to use.
  public var providerAccountId: String {
    get { __data["providerAccountId"] }
    set { __data["providerAccountId"] = newValue }
  }

  /// Provider-specific model id or profile id.
  public var selectionMode: GraphQLEnum<ModelPreferenceSelectionMode> {
    get { __data["selectionMode"] }
    set { __data["selectionMode"] = newValue }
  }

  /// Provider-specific model id or profile id for explicit selections.
  public var modelProfile: GraphQLNullable<String> {
    get { __data["modelProfile"] }
    set { __data["modelProfile"] = newValue }
  }

  /// Optional explicit reasoning effort for reasoning-capable model profiles.
  public var reasoningEffort: GraphQLNullable<GraphQLEnum<ReasoningEffort>> {
    get { __data["reasoningEffort"] }
    set { __data["reasoningEffort"] = newValue }
  }
}
