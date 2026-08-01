// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Input for resolving one exact action revision.
nonisolated public struct ResolveGovernedActionInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    actionId: String,
    expectedRevision: Int32,
    decision: GraphQLEnum<GovernedActionDecision>
  ) {
    __data = InputDict([
      "actionId": actionId,
      "expectedRevision": expectedRevision,
      "decision": decision
    ])
  }

  public var actionId: String {
    get { __data["actionId"] }
    set { __data["actionId"] = newValue }
  }

  public var expectedRevision: Int32 {
    get { __data["expectedRevision"] }
    set { __data["expectedRevision"] = newValue }
  }

  public var decision: GraphQLEnum<GovernedActionDecision> {
    get { __data["decision"] }
    set { __data["decision"] = newValue }
  }
}
