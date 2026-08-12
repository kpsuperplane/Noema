// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct SettingsReplaceAdapterOauthApplicationMutation: GraphQLMutation {
  public static let operationName: String = "SettingsReplaceAdapterOauthApplication"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation SettingsReplaceAdapterOauthApplication($input: ReplaceAdapterOauthApplicationInput!) { replaceAdapterOauthApplication(input: $input) { __typename applicationId revision status } }"#
    ))

  public var input: ReplaceAdapterOauthApplicationInput

  public init(input: ReplaceAdapterOauthApplicationInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("replaceAdapterOauthApplication", ReplaceAdapterOauthApplication.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      SettingsReplaceAdapterOauthApplicationMutation.Data.self
    ] }

    /// Replace one exact OAuth application document.
    public var replaceAdapterOauthApplication: ReplaceAdapterOauthApplication { __data["replaceAdapterOauthApplication"] }

    /// ReplaceAdapterOauthApplication
    ///
    /// Parent Type: `AdapterOauthApplication`
    nonisolated public struct ReplaceAdapterOauthApplication: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.AdapterOauthApplication }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("applicationId", String.self),
        .field("revision", Int.self),
        .field("status", String.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SettingsReplaceAdapterOauthApplicationMutation.Data.ReplaceAdapterOauthApplication.self
      ] }

      public var applicationId: String { __data["applicationId"] }
      public var revision: Int { __data["revision"] }
      public var status: String { __data["status"] }
    }
  }
}
