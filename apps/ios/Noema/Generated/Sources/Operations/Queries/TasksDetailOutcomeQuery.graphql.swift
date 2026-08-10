// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct TasksDetailOutcomeQuery: GraphQLQuery {
  public static let operationName: String = "TasksDetailOutcome"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"query TasksDetailOutcome($taskId: String!) { task(taskId: $taskId) { __typename ...TasksDetailOutcomeFields } }"#,
      fragments: [TasksContractFields.self, TasksDetailOutcomeFields.self, TasksPolicyFields.self, TasksReviewFields.self, TasksReviewSummaryFields.self, TasksSubmissionFields.self]
    ))

  public var taskId: String

  public init(taskId: String) {
    self.taskId = taskId
  }

  @_spi(Unsafe) public var __variables: Variables? { ["taskId": taskId] }

  nonisolated public struct Data: NoemaAPI.SelectionSet {
    @_spi(Unsafe) public let __data: DataDict
    @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

    @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.QueryRoot }
    @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
      .field("task", Task.self, arguments: ["taskId": .variable("taskId")]),
    ] }
    @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
      TasksDetailOutcomeQuery.Data.self
    ] }

    /// Return one owner-authorized task detail.
    public var task: Task { __data["task"] }

    /// Task
    ///
    /// Parent Type: `TaskDetail`
    nonisolated public struct Task: NoemaAPI.SelectionSet {
      @_spi(Unsafe) public let __data: DataDict
      @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

      @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskDetail }
      @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .fragment(TasksDetailOutcomeFields.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        TasksDetailOutcomeQuery.Data.Task.self,
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

      public struct Fragments: FragmentContainer {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        public var tasksDetailOutcomeFields: TasksDetailOutcomeFields { _toFragment() }
      }

      public typealias CurrentContract = TasksDetailOutcomeFields.CurrentContract

      public typealias LatestSubmission = TasksDetailOutcomeFields.LatestSubmission

      public typealias CompletedResult = TasksDetailOutcomeFields.CompletedResult

      public typealias LatestReview = TasksDetailOutcomeFields.LatestReview

      public typealias Submission = TasksDetailOutcomeFields.Submission

      public typealias Review = TasksDetailOutcomeFields.Review
    }
  }
}
