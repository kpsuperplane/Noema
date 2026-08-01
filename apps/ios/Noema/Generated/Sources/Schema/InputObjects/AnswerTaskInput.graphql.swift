// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Resolve a human gate.
nonisolated public struct AnswerTaskInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    taskId: String,
    gateId: String,
    expectedRevision: Int32,
    expectedGeneration: Int32,
    answerMarkdown: String,
    approvalDecision: GraphQLNullable<GraphQLEnum<ApprovalDecision>> = nil,
    clientMutationId: String
  ) {
    __data = InputDict([
      "taskId": taskId,
      "gateId": gateId,
      "expectedRevision": expectedRevision,
      "expectedGeneration": expectedGeneration,
      "answerMarkdown": answerMarkdown,
      "approvalDecision": approvalDecision,
      "clientMutationId": clientMutationId
    ])
  }

  /// Task target.
  public var taskId: String {
    get { __data["taskId"] }
    set { __data["taskId"] = newValue }
  }

  /// Current open gate.
  public var gateId: String {
    get { __data["gateId"] }
    set { __data["gateId"] = newValue }
  }

  /// Expected current revision.
  public var expectedRevision: Int32 {
    get { __data["expectedRevision"] }
    set { __data["expectedRevision"] = newValue }
  }

  /// Expected execution generation.
  public var expectedGeneration: Int32 {
    get { __data["expectedGeneration"] }
    set { __data["expectedGeneration"] = newValue }
  }

  /// Human answer Markdown.
  public var answerMarkdown: String {
    get { __data["answerMarkdown"] }
    set { __data["answerMarkdown"] = newValue }
  }

  /// Required for Approval gates.
  public var approvalDecision: GraphQLNullable<GraphQLEnum<ApprovalDecision>> {
    get { __data["approvalDecision"] }
    set { __data["approvalDecision"] = newValue }
  }

  /// Caller idempotency key.
  public var clientMutationId: String {
    get { __data["clientMutationId"] }
    set { __data["clientMutationId"] = newValue }
  }
}
