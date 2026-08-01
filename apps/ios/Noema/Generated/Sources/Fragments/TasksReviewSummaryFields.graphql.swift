// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct TasksReviewSummaryFields: NoemaAPI.SelectionSet, Fragment {
  public static var fragmentDefinition: StaticString {
    #"fragment TasksReviewSummaryFields on TaskReviewSummary { __typename reviewId reviewedSubmissionId reviewAttemptIndex supersedesReviewId verdict feedback createdAt }"#
  }

  @_spi(Unsafe) public let __data: DataDict
  @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

  @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskReviewSummary }
  @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
    .field("__typename", String.self),
    .field("reviewId", String.self),
    .field("reviewedSubmissionId", String.self),
    .field("reviewAttemptIndex", Int.self),
    .field("supersedesReviewId", String?.self),
    .field("verdict", GraphQLEnum<NoemaAPI.TaskReviewVerdict>.self),
    .field("feedback", String.self),
    .field("createdAt", String.self),
  ] }
  @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
    TasksReviewSummaryFields.self
  ] }

  /// Review identity.
  public var reviewId: String { __data["reviewId"] }
  /// Submission evaluated.
  public var reviewedSubmissionId: String { __data["reviewedSubmissionId"] }
  /// Review attempt.
  public var reviewAttemptIndex: Int { __data["reviewAttemptIndex"] }
  /// Prior review, when any.
  public var supersedesReviewId: String? { __data["supersedesReviewId"] }
  /// Verdict.
  public var verdict: GraphQLEnum<NoemaAPI.TaskReviewVerdict> { __data["verdict"] }
  /// Safe feedback.
  public var feedback: String { __data["feedback"] }
  /// Creation timestamp.
  public var createdAt: String { __data["createdAt"] }
}
