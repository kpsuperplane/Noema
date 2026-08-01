// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct ImportAdapterOauthClientJsonMutation: GraphQLMutation {
  public static let operationName: String = "ImportAdapterOauthClientJson"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation ImportAdapterOauthClientJson($input: ImportAdapterOauthClientJsonInput!) { importAdapterOauthClientJson(input: $input) { __typename semanticDigest } }"#
    ))

  public var input: ImportAdapterOauthClientJsonInput

  public init(input: ImportAdapterOauthClientJsonInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("importAdapterOauthClientJson", ImportAdapterOauthClientJson.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      ImportAdapterOauthClientJsonMutation.Data.self
    ] }

    /// Import one human-selected OAuth client JSON document without retaining
    /// the raw upload.
    public var importAdapterOauthClientJson: ImportAdapterOauthClientJson { __data["importAdapterOauthClientJson"] }

    /// ImportAdapterOauthClientJson
    ///
    /// Parent Type: `AdapterDefinition`
    nonisolated public struct ImportAdapterOauthClientJson: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AdapterDefinition }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("semanticDigest", String.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        ImportAdapterOauthClientJsonMutation.Data.ImportAdapterOauthClientJson.self
      ] }

      public var semanticDigest: String { __data["semanticDigest"] }
    }
  }
}
