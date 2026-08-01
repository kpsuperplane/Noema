// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct NativeProviderAuthAttemptFields: NoemaAPI.SelectionSet, Fragment {
  public static var fragmentDefinition: StaticString {
    #"fragment NativeProviderAuthAttemptFields on ProviderAuthAttempt { __typename attemptId providerKind providerAccountId method status verificationUrl userCode instructions errorCode errorMessage }"#
  }

  @_spi(Unsafe) public let __data: DataDict
  @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

  @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.ProviderAuthAttempt }
  @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
    .field("__typename", String.self),
    .field("attemptId", String.self),
    .field("providerKind", String.self),
    .field("providerAccountId", String.self),
    .field("method", GraphQLEnum<NoemaAPI.ProviderAuthMethod>.self),
    .field("status", GraphQLEnum<NoemaAPI.ProviderAuthAttemptStatus>.self),
    .field("verificationUrl", String?.self),
    .field("userCode", String?.self),
    .field("instructions", String?.self),
    .field("errorCode", String?.self),
    .field("errorMessage", String?.self),
  ] }
  @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
    NativeProviderAuthAttemptFields.self
  ] }

  /// Short-lived auth attempt id.
  public var attemptId: String { __data["attemptId"] }
  /// Provider family.
  public var providerKind: String { __data["providerKind"] }
  /// Provider account id.
  public var providerAccountId: String { __data["providerAccountId"] }
  /// Provider account auth method.
  public var method: GraphQLEnum<NoemaAPI.ProviderAuthMethod> { __data["method"] }
  /// Attempt status.
  public var status: GraphQLEnum<NoemaAPI.ProviderAuthAttemptStatus> { __data["status"] }
  /// Verification URL when available.
  public var verificationUrl: String? { __data["verificationUrl"] }
  /// User code when available.
  public var userCode: String? { __data["userCode"] }
  /// Static UI-safe instruction text.
  public var instructions: String? { __data["instructions"] }
  /// Stable non-secret error code.
  public var errorCode: String? { __data["errorCode"] }
  /// Failure message when available.
  public var errorMessage: String? { __data["errorMessage"] }
}
