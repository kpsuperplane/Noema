// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct SettingsCreateProviderAccountMutation: GraphQLMutation {
  public static let operationName: String = "SettingsCreateProviderAccount"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"mutation SettingsCreateProviderAccount($input: CreateProviderAccountInput!) { createProviderAccount(input: $input) { __typename providerAccountId providerKind accountKey displayName authMethod status isActive isDefault lastCheckedAt lastAuthenticatedAt lastErrorCode lastErrorMessage } }"#
    ))

  public var input: CreateProviderAccountInput

  public init(input: CreateProviderAccountInput) {
    self.input = input
  }

  @_spi(Unsafe) public var __variables: Variables? { ["input": input] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.MutationRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("createProviderAccount", CreateProviderAccount.self, arguments: ["input": .variable("input")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      SettingsCreateProviderAccountMutation.Data.self
    ] }

    /// Create a user-managed provider account.
    public var createProviderAccount: CreateProviderAccount { __data["createProviderAccount"] }

    /// CreateProviderAccount
    ///
    /// Parent Type: `ProviderAccount`
    nonisolated public struct CreateProviderAccount: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.ProviderAccount }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("providerAccountId", String.self),
        .field("providerKind", String.self),
        .field("accountKey", String.self),
        .field("displayName", String.self),
        .field("authMethod", String.self),
        .field("status", GraphQLEnum<NoemaAPI.ProviderAccountStatus>.self),
        .field("isActive", Bool.self),
        .field("isDefault", Bool.self),
        .field("lastCheckedAt", String?.self),
        .field("lastAuthenticatedAt", String?.self),
        .field("lastErrorCode", String?.self),
        .field("lastErrorMessage", String?.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        SettingsCreateProviderAccountMutation.Data.CreateProviderAccount.self
      ] }

      /// Stable provider account id.
      public var providerAccountId: String { __data["providerAccountId"] }
      /// Provider family, such as `codex`.
      public var providerKind: String { __data["providerKind"] }
      /// Provider-local account key.
      public var accountKey: String { __data["accountKey"] }
      /// Human-readable account name.
      public var displayName: String { __data["displayName"] }
      /// Authentication method used for this account.
      public var authMethod: String { __data["authMethod"] }
      /// Last known account readiness status.
      public var status: GraphQLEnum<NoemaAPI.ProviderAccountStatus> { __data["status"] }
      /// Whether the account may be selected.
      public var isActive: Bool { __data["isActive"] }
      /// Whether the account is the default account for its provider.
      public var isDefault: Bool { __data["isDefault"] }
      /// Last time Noema checked the account status.
      public var lastCheckedAt: String? { __data["lastCheckedAt"] }
      /// Last time Noema observed successful authentication.
      public var lastAuthenticatedAt: String? { __data["lastAuthenticatedAt"] }
      /// Last non-secret provider error code.
      public var lastErrorCode: String? { __data["lastErrorCode"] }
      /// Last non-secret provider error message.
      public var lastErrorMessage: String? { __data["lastErrorMessage"] }
    }
  }
}
