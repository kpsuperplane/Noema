// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct SettingsSetAdapterConnectionActiveMutation: GraphQLMutation {
  public static let operationName: String = "SettingsSetAdapterConnectionActive"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation SettingsSetAdapterConnectionActive($input: SetAdapterConnectionActiveInput!) { setAdapterConnectionActive(input: $input) { __typename semanticDigest connectionCount } }"#
    ))

  public var input: SetAdapterConnectionActiveInput

  public init(input: SetAdapterConnectionActiveInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("setAdapterConnectionActive", SetAdapterConnectionActive.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      SettingsSetAdapterConnectionActiveMutation.Data.self
    ] }

    /// Suspend or resume one API connection.
    public var setAdapterConnectionActive: SetAdapterConnectionActive { __data["setAdapterConnectionActive"] }

    /// SetAdapterConnectionActive
    ///
    /// Parent Type: `AdapterDefinition`
    nonisolated public struct SetAdapterConnectionActive: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AdapterDefinition }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("semanticDigest", String.self),
        .field("connectionCount", Int.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SettingsSetAdapterConnectionActiveMutation.Data.SetAdapterConnectionActive.self
      ] }

      public var semanticDigest: String { __data["semanticDigest"] }
      public var connectionCount: Int { __data["connectionCount"] }
    }
  }
}
