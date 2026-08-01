// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Input for creating a provider account.
nonisolated public struct CreateProviderAccountInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    providerKind: String,
    displayName: GraphQLNullable<String> = nil,
    secret: String,
    authMethod: GraphQLEnum<ProviderAuthMethod>
  ) {
    __data = InputDict([
      "providerKind": providerKind,
      "displayName": displayName,
      "secret": secret,
      "authMethod": authMethod
    ])
  }

  public var providerKind: String {
    get { __data["providerKind"] }
    set { __data["providerKind"] = newValue }
  }

  public var displayName: GraphQLNullable<String> {
    get { __data["displayName"] }
    set { __data["displayName"] = newValue }
  }

  public var secret: String {
    get { __data["secret"] }
    set { __data["secret"] = newValue }
  }

  /// Authentication method represented by the supplied secret.
  public var authMethod: GraphQLEnum<ProviderAuthMethod> {
    get { __data["authMethod"] }
    set { __data["authMethod"] = newValue }
  }
}
