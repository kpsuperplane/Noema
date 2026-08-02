// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct SetupAdapterConnectionMutation: GraphQLMutation {
  public static let operationName: String = "SetupAdapterConnection"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation SetupAdapterConnection($input: SetupAdapterConnectionInput!) { setupAdapterConnection(input: $input) { __typename semanticDigest } }"#
    ))

  public var input: SetupAdapterConnectionInput

  public init(input: SetupAdapterConnectionInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("setupAdapterConnection", SetupAdapterConnection.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      SetupAdapterConnectionMutation.Data.self
    ] }

    /// Normalize transient credential input and create one adapter connection.
    public var setupAdapterConnection: SetupAdapterConnection { __data["setupAdapterConnection"] }

    /// SetupAdapterConnection
    ///
    /// Parent Type: `AdapterDefinition`
    nonisolated public struct SetupAdapterConnection: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AdapterDefinition }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("semanticDigest", String.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SetupAdapterConnectionMutation.Data.SetupAdapterConnection.self
      ] }

      public var semanticDigest: String { __data["semanticDigest"] }
    }
  }
}
