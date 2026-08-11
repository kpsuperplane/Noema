// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct TasksDetailOutcomeFields: NoemaAPI.SelectionSet, Fragment {
  public static var fragmentDefinition: StaticString {
    #"fragment TasksDetailOutcomeFields on TaskDetail { __typename taskId currentContract { __typename ...TasksContractFields } latestSubmission { __typename ...TasksSubmissionFields } completedResult { __typename ...TasksSubmissionFields } latestReview { __typename ...TasksReviewSummaryFields } submissions { __typename ...TasksSubmissionFields } reviews { __typename ...TasksReviewFields } }"#
  }

  @_spi(Unsafe) public let __data: DataDict
  @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

  @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskDetail }
  @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
    .field("__typename", String.self),
    .field("taskId", String.self),
    .field("currentContract", CurrentContract?.self),
    .field("latestSubmission", LatestSubmission?.self),
    .field("completedResult", CompletedResult?.self),
    .field("latestReview", LatestReview?.self),
    .field("submissions", [Submission].self),
    .field("reviews", [Review].self),
  ] }
  @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
    TasksDetailOutcomeFields.self
  ] }

  /// Task identity.
  public var taskId: String { __data["taskId"] }
  /// Current immutable contract.
  public var currentContract: CurrentContract? { __data["currentContract"] }
  /// Latest immutable submission.
  public var latestSubmission: LatestSubmission? { __data["latestSubmission"] }
  /// Reviewer-approved immutable result that completed the task.
  public var completedResult: CompletedResult? { __data["completedResult"] }
  /// Latest immutable review.
  public var latestReview: LatestReview? { __data["latestReview"] }
  /// Bounded recent submissions.
  public var submissions: [Submission] { __data["submissions"] }
  /// Bounded recent reviews.
  public var reviews: [Review] { __data["reviews"] }

  /// CurrentContract
  ///
  /// Parent Type: `TaskExecutionContract`
  nonisolated public struct CurrentContract: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskExecutionContract }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("__typename", String.self),
      .fragment(TasksContractFields.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksDetailOutcomeFields.CurrentContract.self,
      TasksContractFields.self
    ] }

    /// Immutable request Markdown.
    public var requestMarkdown: String { __data["requestMarkdown"] }
    /// Exact criteria.
    public var criteria: [Criterium] { __data["criteria"] }
    /// Complexity tier.
    public var complexity: GraphQLEnum<NoemaAPI.TaskComplexity> { __data["complexity"] }
    /// Execution policy snapshot.
    public var executionPolicy: ExecutionPolicy { __data["executionPolicy"] }
    /// Assigned executor agent identity.
    public var executorAgentId: String { __data["executorAgentId"] }
    /// Executor backend.
    public var executorBackend: String { __data["executorBackend"] }
    /// Frozen effective working directory.
    public var effectiveCwd: String? { __data["effectiveCwd"] }

    public struct Fragments: FragmentContainer {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      public var tasksContractFields: TasksContractFields { _toFragment() }
    }

    public typealias Criterium = TasksContractFields.Criterium

    public typealias ExecutionPolicy = TasksContractFields.ExecutionPolicy
  }

  /// LatestSubmission
  ///
  /// Parent Type: `TaskSubmission`
  nonisolated public struct LatestSubmission: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskSubmission }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("__typename", String.self),
      .fragment(TasksSubmissionFields.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksDetailOutcomeFields.LatestSubmission.self,
      TasksSubmissionFields.self
    ] }

    /// Submission identity.
    public var submissionId: String { __data["submissionId"] }
    /// Contract evaluated.
    public var contractId: String { __data["contractId"] }
    /// Executor run.
    public var executorRunId: String { __data["executorRunId"] }
    /// Review round.
    public var reviewRound: Int { __data["reviewRound"] }
    /// Short summary.
    public var summary: String { __data["summary"] }
    /// Complete result Markdown.
    public var resultMarkdown: String { __data["resultMarkdown"] }
    /// Verified web sources.
    public var citations: [Citation] { __data["citations"] }
    /// Criterion evidence.
    public var criteria: [Criterium] { __data["criteria"] }
    /// Linked immutable artifact versions.
    public var artifacts: [Artifact] { __data["artifacts"] }
    /// Creation timestamp.
    public var createdAt: String { __data["createdAt"] }

    public struct Fragments: FragmentContainer {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      public var tasksSubmissionFields: TasksSubmissionFields { _toFragment() }
    }

    public typealias Citation = TasksSubmissionFields.Citation

    public typealias Criterium = TasksSubmissionFields.Criterium

    public typealias Artifact = TasksSubmissionFields.Artifact
  }

  /// CompletedResult
  ///
  /// Parent Type: `TaskSubmission`
  nonisolated public struct CompletedResult: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskSubmission }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("__typename", String.self),
      .fragment(TasksSubmissionFields.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksDetailOutcomeFields.CompletedResult.self,
      TasksSubmissionFields.self
    ] }

    /// Submission identity.
    public var submissionId: String { __data["submissionId"] }
    /// Contract evaluated.
    public var contractId: String { __data["contractId"] }
    /// Executor run.
    public var executorRunId: String { __data["executorRunId"] }
    /// Review round.
    public var reviewRound: Int { __data["reviewRound"] }
    /// Short summary.
    public var summary: String { __data["summary"] }
    /// Complete result Markdown.
    public var resultMarkdown: String { __data["resultMarkdown"] }
    /// Verified web sources.
    public var citations: [Citation] { __data["citations"] }
    /// Criterion evidence.
    public var criteria: [Criterium] { __data["criteria"] }
    /// Linked immutable artifact versions.
    public var artifacts: [Artifact] { __data["artifacts"] }
    /// Creation timestamp.
    public var createdAt: String { __data["createdAt"] }

    public struct Fragments: FragmentContainer {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      public var tasksSubmissionFields: TasksSubmissionFields { _toFragment() }
    }

    public typealias Citation = TasksSubmissionFields.Citation

    public typealias Criterium = TasksSubmissionFields.Criterium

    public typealias Artifact = TasksSubmissionFields.Artifact
  }

  /// LatestReview
  ///
  /// Parent Type: `TaskReviewSummary`
  nonisolated public struct LatestReview: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskReviewSummary }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("__typename", String.self),
      .fragment(TasksReviewSummaryFields.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksDetailOutcomeFields.LatestReview.self,
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

    public struct Fragments: FragmentContainer {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      public var tasksReviewSummaryFields: TasksReviewSummaryFields { _toFragment() }
    }
  }

  /// Submission
  ///
  /// Parent Type: `TaskSubmission`
  nonisolated public struct Submission: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskSubmission }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("__typename", String.self),
      .fragment(TasksSubmissionFields.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksDetailOutcomeFields.Submission.self,
      TasksSubmissionFields.self
    ] }

    /// Submission identity.
    public var submissionId: String { __data["submissionId"] }
    /// Contract evaluated.
    public var contractId: String { __data["contractId"] }
    /// Executor run.
    public var executorRunId: String { __data["executorRunId"] }
    /// Review round.
    public var reviewRound: Int { __data["reviewRound"] }
    /// Short summary.
    public var summary: String { __data["summary"] }
    /// Complete result Markdown.
    public var resultMarkdown: String { __data["resultMarkdown"] }
    /// Verified web sources.
    public var citations: [Citation] { __data["citations"] }
    /// Criterion evidence.
    public var criteria: [Criterium] { __data["criteria"] }
    /// Linked immutable artifact versions.
    public var artifacts: [Artifact] { __data["artifacts"] }
    /// Creation timestamp.
    public var createdAt: String { __data["createdAt"] }

    public struct Fragments: FragmentContainer {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      public var tasksSubmissionFields: TasksSubmissionFields { _toFragment() }
    }

    public typealias Citation = TasksSubmissionFields.Citation

    public typealias Criterium = TasksSubmissionFields.Criterium

    public typealias Artifact = TasksSubmissionFields.Artifact
  }

  /// Review
  ///
  /// Parent Type: `TaskReview`
  nonisolated public struct Review: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskReview }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("__typename", String.self),
      .fragment(TasksReviewFields.self),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksDetailOutcomeFields.Review.self,
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

    public struct Fragments: FragmentContainer {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      public var tasksReviewFields: TasksReviewFields { _toFragment() }
    }

    public typealias Criterium = TasksReviewFields.Criterium
  }
}
