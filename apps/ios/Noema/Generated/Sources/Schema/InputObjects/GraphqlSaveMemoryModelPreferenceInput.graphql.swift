// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

nonisolated public struct GraphqlSaveMemoryModelPreferenceInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    providerAccountId: String,
    selectionMode: GraphQLEnum<ModelPreferenceSelectionMode>,
    modelProfile: GraphQLNullable<String> = nil,
    reasoningEffort: GraphQLNullable<GraphQLEnum<ReasoningEffort>> = nil
  ) {
    __data = InputDict([
      "providerAccountId": providerAccountId,
      "selectionMode": selectionMode,
      "modelProfile": modelProfile,
      "reasoningEffort": reasoningEffort
    ])
  }

  public var providerAccountId: String {
    get { __data["providerAccountId"] }
    set { __data["providerAccountId"] = newValue }
  }

  public var selectionMode: GraphQLEnum<ModelPreferenceSelectionMode> {
    get { __data["selectionMode"] }
    set { __data["selectionMode"] = newValue }
  }

  public var modelProfile: GraphQLNullable<String> {
    get { __data["modelProfile"] }
    set { __data["modelProfile"] = newValue }
  }

  public var reasoningEffort: GraphQLNullable<GraphQLEnum<ReasoningEffort>> {
    get { __data["reasoningEffort"] }
    set { __data["reasoningEffort"] = newValue }
  }
}
