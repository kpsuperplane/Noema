// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Input for starting a provider auth attempt.
nonisolated public struct StartProviderAuthAttemptInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    providerKind: String,
    providerAccountId: GraphQLNullable<String> = nil,
    method: GraphQLEnum<ProviderAuthMethod>
  ) {
    __data = InputDict([
      "providerKind": providerKind,
      "providerAccountId": providerAccountId,
      "method": method
    ])
  }

  /// Provider family, such as `codex`.
  public var providerKind: String {
    get { __data["providerKind"] }
    set { __data["providerKind"] = newValue }
  }

  /// Stable provider account id when reconnecting an existing account.
  public var providerAccountId: GraphQLNullable<String> {
    get { __data["providerAccountId"] }
    set { __data["providerAccountId"] = newValue }
  }

  /// Requested authentication method.
  public var method: GraphQLEnum<ProviderAuthMethod> {
    get { __data["method"] }
    set { __data["method"] = newValue }
  }
}
