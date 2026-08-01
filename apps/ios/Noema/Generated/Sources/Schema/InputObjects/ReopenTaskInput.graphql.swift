// @generated
// This file was automatically generated and should not be edited.

@_spi(Internal) @_spi(Unsafe) import ApolloAPI

/// Reopen a completed task with new direction.
nonisolated public struct ReopenTaskInput: InputObject {
  @_spi(Unsafe) public private(set) var __data: InputDict

  @_spi(Unsafe) public init(_ data: InputDict) {
    __data = data
  }

  public init(
    taskId: String,
    expectedRevision: Int32,
    expectedGeneration: Int32,
    feedbackMarkdown: String,
    requestMarkdown: GraphQLNullable<String> = nil,
    replacementCriteria: GraphQLNullable<[TaskValidationCriterionInput]> = nil,
    complexity: GraphQLNullable<GraphQLEnum<TaskComplexity>> = nil,
    clientMutationId: String
  ) {
    __data = InputDict([
      "taskId": taskId,
      "expectedRevision": expectedRevision,
      "expectedGeneration": expectedGeneration,
      "feedbackMarkdown": feedbackMarkdown,
      "requestMarkdown": requestMarkdown,
      "replacementCriteria": replacementCriteria,
      "complexity": complexity,
      "clientMutationId": clientMutationId
    ])
  }

  /// Task target.
  public var taskId: String {
    get { __data["taskId"] }
    set { __data["taskId"] = newValue }
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

  /// Nonblank feedback.
  public var feedbackMarkdown: String {
    get { __data["feedbackMarkdown"] }
    set { __data["feedbackMarkdown"] = newValue }
  }

  /// Optional replacement request.
  public var requestMarkdown: GraphQLNullable<String> {
    get { __data["requestMarkdown"] }
    set { __data["requestMarkdown"] = newValue }
  }

  /// Complete replacement criteria set.
  public var replacementCriteria: GraphQLNullable<[TaskValidationCriterionInput]> {
    get { __data["replacementCriteria"] }
    set { __data["replacementCriteria"] = newValue }
  }

  /// Optional replacement complexity.
  public var complexity: GraphQLNullable<GraphQLEnum<TaskComplexity>> {
    get { __data["complexity"] }
    set { __data["complexity"] = newValue }
  }

  /// Caller idempotency key.
  public var clientMutationId: String {
    get { __data["clientMutationId"] }
    set { __data["clientMutationId"] = newValue }
  }
}
