// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Input for cancelling a provider authentication attempt.
nonisolated public struct CancelProviderAuthAttemptInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    attemptId: String
  ) {
    __data = InputDict([
      "attemptId": attemptId
    ])
  }

  /// Short-lived attempt identifier.
  public var attemptId: String {
    get { __data["attemptId"] }
    set { __data["attemptId"] = newValue }
  }
}
