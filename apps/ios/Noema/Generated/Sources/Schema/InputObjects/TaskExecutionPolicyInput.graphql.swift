// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Input for replacing the four user-controlled Task Executor runtime limits.
nonisolated public struct TaskExecutionPolicyInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    maxProviderContinuations: Int32,
    maxToolCalls: Int32,
    maxActiveMinutes: Int32,
    progressAuditInterval: Int32
  ) {
    __data = InputDict([
      "maxProviderContinuations": maxProviderContinuations,
      "maxToolCalls": maxToolCalls,
      "maxActiveMinutes": maxActiveMinutes,
      "progressAuditInterval": progressAuditInterval
    ])
  }

  /// Maximum provider continuations before terminal-only finalization.
  public var maxProviderContinuations: Int32 {
    get { __data["maxProviderContinuations"] }
    set { __data["maxProviderContinuations"] = newValue }
  }

  /// Maximum tool calls before terminal-only finalization.
  public var maxToolCalls: Int32 {
    get { __data["maxToolCalls"] }
    set { __data["maxToolCalls"] = newValue }
  }

  /// Maximum active execution time in minutes, excluding queue time.
  public var maxActiveMinutes: Int32 {
    get { __data["maxActiveMinutes"] }
    set { __data["maxActiveMinutes"] = newValue }
  }

  /// Continuation interval between progress audits.
  public var progressAuditInterval: Int32 {
    get { __data["progressAuditInterval"] }
    set { __data["progressAuditInterval"] = newValue }
  }
}
