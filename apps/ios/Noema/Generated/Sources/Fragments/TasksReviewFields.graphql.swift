// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct TasksReviewFields: NoemaAPI.SelectionSet, Fragment {
  public static var fragmentDefinition: StaticString {
    #"fragment TasksReviewFields on TaskReview { __typename reviewId contractId reviewerRunId reviewedSubmissionId reviewAttemptIndex supersedesReviewId verdict feedback criteria { __typename criterionId outcome evidenceMarkdown feedback } createdAt }"#
  }

  @_spi(Unsafe) public let __data: DataDict
  @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

  @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskReview }
  @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
    .field("__typename", String.self),
    .field("reviewId", String.self),
    .field("contractId", String.self),
    .field("reviewerRunId", String.self),
    .field("reviewedSubmissionId", String.self),
    .field("reviewAttemptIndex", Int.self),
    .field("supersedesReviewId", String?.self),
    .field("verdict", GraphQLEnum<NoemaAPI.TaskReviewVerdict>.self),
    .field("feedback", String.self),
    .field("criteria", [Criterium].self),
    .field("createdAt", String.self),
  ] }
  @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
    TasksReviewFields.self
  ] }

  /// Review identity.
  public var reviewId: String { __data["reviewId"] }
  /// Contract evaluated.
  public var contractId: String { __data["contractId"] }
  /// Reviewer run.
  public var reviewerRunId: String { __data["reviewerRunId"] }
  /// Submission evaluated.
  public var reviewedSubmissionId: String { __data["reviewedSubmissionId"] }
  /// Review attempt for the submission.
  public var reviewAttemptIndex: Int { __data["reviewAttemptIndex"] }
  /// Prior needs-human review, when any.
  public var supersedesReviewId: String? { __data["supersedesReviewId"] }
  /// Approve, request_changes, or needs_human.
  public var verdict: GraphQLEnum<NoemaAPI.TaskReviewVerdict> { __data["verdict"] }
  /// Safe reviewer feedback.
  public var feedback: String { __data["feedback"] }
  /// Criterion outcomes.
  public var criteria: [Criterium] { __data["criteria"] }
  /// Creation timestamp.
  public var createdAt: String { __data["createdAt"] }

  /// Criterium
  ///
  /// Parent Type: `TaskReviewCriterion`
  nonisolated public struct Criterium: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskReviewCriterion }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("__typename", String.self),
      .field("criterionId", String.self),
      .field("outcome", GraphQLEnum<NoemaAPI.TaskCriterionOutcome>.self),
      .field("evidenceMarkdown", String?.self),
      .field("feedback", String?.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksReviewFields.Criterium.self
    ] }

    /// Criterion identity.
    public var criterionId: String { __data["criterionId"] }
    /// Pass, fail, or uncertain.
    public var outcome: GraphQLEnum<NoemaAPI.TaskCriterionOutcome> { __data["outcome"] }
    /// Evidence Markdown.
    public var evidenceMarkdown: String? { __data["evidenceMarkdown"] }
    /// Reviewer feedback.
    public var feedback: String? { __data["feedback"] }
  }
}
