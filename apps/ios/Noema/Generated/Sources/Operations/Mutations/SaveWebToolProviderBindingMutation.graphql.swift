// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct SaveWebToolProviderBindingMutation: GraphQLMutation {
  public static let operationName: String = "SaveWebToolProviderBinding"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation SaveWebToolProviderBinding($input: SaveWebToolProviderBindingInput!) { saveWebToolProviderBinding(input: $input) { __typename toolName capabilityId activeProviderAccountId } }"#
    ))

  public var input: SaveWebToolProviderBindingInput

  public init(input: SaveWebToolProviderBindingInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("saveWebToolProviderBinding", SaveWebToolProviderBinding.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      SaveWebToolProviderBindingMutation.Data.self
    ] }

    /// Save the provider binding for one web tool capability.
    public var saveWebToolProviderBinding: SaveWebToolProviderBinding { __data["saveWebToolProviderBinding"] }

    /// SaveWebToolProviderBinding
    ///
    /// Parent Type: `WebToolBindingSettings`
    nonisolated public struct SaveWebToolProviderBinding: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.WebToolBindingSettings }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("toolName", String.self),
        .field("capabilityId", String.self),
        .field("activeProviderAccountId", String.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SaveWebToolProviderBindingMutation.Data.SaveWebToolProviderBinding.self
      ] }

      public var toolName: String { __data["toolName"] }
      public var capabilityId: String { __data["capabilityId"] }
      public var activeProviderAccountId: String { __data["activeProviderAccountId"] }
    }
  }
}
