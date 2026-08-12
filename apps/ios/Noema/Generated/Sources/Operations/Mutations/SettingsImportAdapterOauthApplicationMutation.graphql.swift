// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct SettingsImportAdapterOauthApplicationMutation: GraphQLMutation {
  public static let operationName: String = "SettingsImportAdapterOauthApplication"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation SettingsImportAdapterOauthApplication($input: ImportAdapterOauthApplicationInput!) { importAdapterOauthApplication(input: $input) { __typename applicationId profileDigest revision status } }"#
    ))

  public var input: ImportAdapterOauthApplicationInput

  public init(input: ImportAdapterOauthApplicationInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("importAdapterOauthApplication", ImportAdapterOauthApplication.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      SettingsImportAdapterOauthApplicationMutation.Data.self
    ] }

    /// Import one reusable OAuth client document.
    public var importAdapterOauthApplication: ImportAdapterOauthApplication { __data["importAdapterOauthApplication"] }

    /// ImportAdapterOauthApplication
    ///
    /// Parent Type: `AdapterOauthApplication`
    nonisolated public struct ImportAdapterOauthApplication: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AdapterOauthApplication }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("applicationId", String.self),
        .field("profileDigest", String.self),
        .field("revision", Int.self),
        .field("status", String.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SettingsImportAdapterOauthApplicationMutation.Data.ImportAdapterOauthApplication.self
      ] }

      public var applicationId: String { __data["applicationId"] }
      public var profileDigest: String { __data["profileDigest"] }
      public var revision: Int { __data["revision"] }
      public var status: String { __data["status"] }
    }
  }
}
