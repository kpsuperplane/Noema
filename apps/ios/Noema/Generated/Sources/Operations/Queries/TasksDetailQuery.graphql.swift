// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
@_spi(Execution) @_spi(Unsafe) import ApolloAPI

nonisolated public struct TasksDetailQuery: GraphQLQuery {
  public static let operationName: String = "TasksDetail"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"query TasksDetail($taskId: String!) { task(taskId: $taskId) { __typename ...TasksCommandTaskFields project { __typename ...TasksProjectFields } createdAt source { __typename conversationId } currentContract { __typename ...TasksContractFields } latestSubmission { __typename ...TasksSubmissionFields } completedResult { __typename ...TasksSubmissionFields } latestReview { __typename ...TasksReviewSummaryFields } attention { __typename kind title summary validActions gate { __typename ...TasksGateFields } } messages { __typename messageId bodyMarkdown author createdAt } runs { __typename ...TasksRunFields } submissions { __typename ...TasksSubmissionFields } reviews { __typename ...TasksReviewFields } artifacts { __typename artifactId title artifactKind storageKind currentVersion { __typename artifactVersionId mediaType downloadUrl externalUrl } } } }"#,
      fragments: [TasksCommandTaskFields.self, TasksContractFields.self, TasksCurrentRunFields.self, TasksGateFields.self, TasksPolicyFields.self, TasksProjectFields.self, TasksReviewFields.self, TasksReviewSummaryFields.self, TasksRunFields.self, TasksStageFields.self, TasksSubmissionFields.self]
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
      TasksDetailQuery.Data.self
    ] }

    /// Return one owner-authorized Tasks task detail.
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
        .field("project", Project?.self),
        .field("createdAt", String.self),
        .field("source", Source.self),
        .field("currentContract", CurrentContract?.self),
        .field("latestSubmission", LatestSubmission?.self),
        .field("completedResult", CompletedResult?.self),
        .field("latestReview", LatestReview?.self),
        .field("attention", Attention?.self),
        .field("messages", [Message].self),
        .field("runs", [Run].self),
        .field("submissions", [Submission].self),
        .field("reviews", [Review].self),
        .field("artifacts", [Artifact].self),
        .fragment(TasksCommandTaskFields.self),
      ] }
      @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
        TasksDetailQuery.Data.Task.self,
        TasksCommandTaskFields.self
      ] }

      /// Nullable project placement.
      public var project: Project? { __data["project"] }
      /// Creation timestamp.
      public var createdAt: String { __data["createdAt"] }
      /// Safe provenance.
      public var source: Source { __data["source"] }
      /// Current immutable contract.
      public var currentContract: CurrentContract? { __data["currentContract"] }
      /// Latest immutable submission.
      public var latestSubmission: LatestSubmission? { __data["latestSubmission"] }
      /// Reviewer-approved immutable result that completed the task.
      public var completedResult: CompletedResult? { __data["completedResult"] }
      /// Latest immutable review.
      public var latestReview: LatestReview? { __data["latestReview"] }
      /// Derived attention.
      public var attention: Attention? { __data["attention"] }
      /// Bounded recent human messages.
      public var messages: [Message] { __data["messages"] }
      /// Bounded recent task runs.
      public var runs: [Run] { __data["runs"] }
      /// Bounded recent submissions.
      public var submissions: [Submission] { __data["submissions"] }
      /// Bounded recent reviews.
      public var reviews: [Review] { __data["reviews"] }
      /// Bounded current artifacts.
      public var artifacts: [Artifact] { __data["artifacts"] }
      /// Task identity.
      public var taskId: String { __data["taskId"] }
      /// Full title.
      public var title: String { __data["title"] }
      /// Full description Markdown.
      public var description: String { __data["description"] }
      /// Assigned executor agent identity.
      public var executorAgentId: String { __data["executorAgentId"] }
      /// Assigned executor backend.
      public var executorBackend: String { __data["executorBackend"] }
      /// Explicit task working-directory override.
      public var cwdOverride: String? { __data["cwdOverride"] }
      /// Derived or frozen effective working directory.
      public var effectiveCwd: String? { __data["effectiveCwd"] }
      /// Effective working-directory source: task, project, or default.
      public var effectiveCwdSource: String { __data["effectiveCwdSource"] }
      /// The only task-level state.
      public var stage: Stage { __data["stage"] }
      /// Optimistic revision.
      public var revision: Int { __data["revision"] }
      /// Execution generation fence.
      public var generation: Int { __data["generation"] }
      /// Last update timestamp.
      public var updatedAt: String { __data["updatedAt"] }
      /// Optional future execution and recurrence provenance.
      public var schedule: Schedule? { __data["schedule"] }
      /// Completion timestamp, when any.
      public var completedAt: String? { __data["completedAt"] }
      /// Server-authorized actions.
      public var validActions: [GraphQLEnum<NoemaAPI.ValidTaskAction>] { __data["validActions"] }
      /// Open gate projection.
      public var activeGate: ActiveGate? { __data["activeGate"] }
      /// Current run projection.
      public var currentRun: CurrentRun? { __data["currentRun"] }

      public struct Fragments: FragmentContainer {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        public var tasksCommandTaskFields: TasksCommandTaskFields { _toFragment() }
      }

      /// Task.Project
      ///
      /// Parent Type: `Project`
      nonisolated public struct Project: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.Project }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .fragment(TasksProjectFields.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          TasksDetailQuery.Data.Task.Project.self,
          TasksProjectFields.self
        ] }

        /// Opaque project identity.
        public var projectId: String { __data["projectId"] }
        /// Owning workspace.
        public var workspaceId: String { __data["workspaceId"] }
        /// Project name.
        public var name: String { __data["name"] }
        /// Project description.
        public var description: String { __data["description"] }
        /// Optional absolute project working folder.
        public var folder: String? { __data["folder"] }
        /// Optimistic project revision.
        public var revision: Int { __data["revision"] }
        /// Archive timestamp, if archived.
        public var archivedAt: String? { __data["archivedAt"] }
        /// Creation timestamp.
        public var createdAt: String { __data["createdAt"] }
        /// Last update timestamp.
        public var updatedAt: String { __data["updatedAt"] }

        public struct Fragments: FragmentContainer {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          public var tasksProjectFields: TasksProjectFields { _toFragment() }
        }
      }

      /// Task.Source
      ///
      /// Parent Type: `TaskSource`
      nonisolated public struct Source: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskSource }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("conversationId", String?.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          TasksDetailQuery.Data.Task.Source.self
        ] }

        /// Source conversation.
        public var conversationId: String? { __data["conversationId"] }
      }

      /// Task.CurrentContract
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
          TasksDetailQuery.Data.Task.CurrentContract.self,
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

      /// Task.LatestSubmission
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
          TasksDetailQuery.Data.Task.LatestSubmission.self,
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

        public typealias Criterium = TasksSubmissionFields.Criterium

        public typealias Artifact = TasksSubmissionFields.Artifact
      }

      /// Task.CompletedResult
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
          TasksDetailQuery.Data.Task.CompletedResult.self,
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

        public typealias Criterium = TasksSubmissionFields.Criterium

        public typealias Artifact = TasksSubmissionFields.Artifact
      }

      /// Task.LatestReview
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
          TasksDetailQuery.Data.Task.LatestReview.self,
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

      /// Task.Attention
      ///
      /// Parent Type: `TaskAttention`
      nonisolated public struct Attention: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskAttention }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("kind", GraphQLEnum<NoemaAPI.TaskAttentionKind>.self),
          .field("title", String.self),
          .field("summary", String.self),
          .field("validActions", [GraphQLEnum<NoemaAPI.ValidTaskAction>].self),
          .field("gate", Gate?.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          TasksDetailQuery.Data.Task.Attention.self
        ] }

        /// Clarification, approval, recovery, or review readiness.
        public var kind: GraphQLEnum<NoemaAPI.TaskAttentionKind> { __data["kind"] }
        /// Stable UI title.
        public var title: String { __data["title"] }
        /// Safe summary.
        public var summary: String { __data["summary"] }
        /// Server-authorized actions.
        public var validActions: [GraphQLEnum<NoemaAPI.ValidTaskAction>] { __data["validActions"] }
        /// Complete related gate evidence, when any.
        public var gate: Gate? { __data["gate"] }

        /// Task.Attention.Gate
        ///
        /// Parent Type: `TaskGate`
        nonisolated public struct Gate: NoemaAPI.SelectionSet {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskGate }
          @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .fragment(TasksGateFields.self),
          ] }
          @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
            TasksDetailQuery.Data.Task.Attention.Gate.self,
            TasksGateFields.self
          ] }

          /// Gate identity.
          public var gateId: String { __data["gateId"] }
          /// Task generation.
          public var taskGeneration: Int { __data["taskGeneration"] }
          /// Gate kind.
          public var kind: GraphQLEnum<NoemaAPI.TaskGateKind> { __data["kind"] }
          /// Gate state.
          public var state: GraphQLEnum<NoemaAPI.TaskGateState> { __data["state"] }
          /// Recovery reason, when any.
          public var recoveryReason: GraphQLEnum<NoemaAPI.TaskRecoveryReason>? { __data["recoveryReason"] }
          /// Explicit recovery continuation role, when any.
          public var retryRunKind: GraphQLEnum<NoemaAPI.TaskRunKind>? { __data["retryRunKind"] }
          /// Human prompt.
          public var prompt: String { __data["prompt"] }
          /// Bounded context.
          public var contextMarkdown: String { __data["contextMarkdown"] }
          /// Optional direct answers.
          public var suggestedAnswers: [String] { __data["suggestedAnswers"] }
          /// Opener actor.
          public var openedBy: String { __data["openedBy"] }
          /// Opening run, when any.
          public var originatingRunId: String? { __data["originatingRunId"] }
          /// Open timestamp.
          public var openedAt: String { __data["openedAt"] }
          /// Resolver actor, when resolved.
          public var resolvedBy: String? { __data["resolvedBy"] }
          /// Resolution timestamp, when resolved.
          public var resolvedAt: String? { __data["resolvedAt"] }
          /// Resolution message identity, when resolved.
          public var resolution: String? { __data["resolution"] }

          public struct Fragments: FragmentContainer {
            @_spi(Unsafe) public let __data: DataDict
            @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

            public var tasksGateFields: TasksGateFields { _toFragment() }
          }
        }
      }

      /// Task.Message
      ///
      /// Parent Type: `TaskMessage`
      nonisolated public struct Message: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskMessage }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("messageId", String.self),
          .field("bodyMarkdown", String.self),
          .field("author", String.self),
          .field("createdAt", String.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          TasksDetailQuery.Data.Task.Message.self
        ] }

        /// Message identity.
        public var messageId: String { __data["messageId"] }
        /// Safe Markdown body.
        public var bodyMarkdown: String { __data["bodyMarkdown"] }
        /// Author actor.
        public var author: String { __data["author"] }
        /// Creation timestamp.
        public var createdAt: String { __data["createdAt"] }
      }

      /// Task.Run
      ///
      /// Parent Type: `TaskRun`
      nonisolated public struct Run: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.TaskRun }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .fragment(TasksRunFields.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          TasksDetailQuery.Data.Task.Run.self,
          TasksRunFields.self
        ] }

        /// Run identity.
        public var runId: String { __data["runId"] }
        /// Human-friendly instance identity.
        public var instanceName: String { __data["instanceName"] }
        /// Run role.
        public var kind: GraphQLEnum<NoemaAPI.TaskRunKind> { __data["kind"] }
        /// Run-local status.
        public var status: GraphQLEnum<NoemaAPI.TaskRunStatus> { __data["status"] }
        /// Agent identity.
        public var agentId: String { __data["agentId"] }
        /// Task generation.
        public var taskGeneration: Int { __data["taskGeneration"] }
        /// Contract identity, absent for Planner.
        public var contractId: String? { __data["contractId"] }
        /// Attempt index.
        public var attemptIndex: Int { __data["attemptIndex"] }
        /// Review round.
        public var reviewRound: Int { __data["reviewRound"] }
        /// Parent run, when any.
        public var parentRunId: String? { __data["parentRunId"] }
        /// Submission trigger, when any.
        public var triggeringSubmissionId: String? { __data["triggeringSubmissionId"] }
        /// Review trigger, when any.
        public var triggeringReviewId: String? { __data["triggeringReviewId"] }
        /// Requested model snapshot.
        public var model: Model { __data["model"] }
        /// Safe actual provider family.
        public var actualProviderKind: String? { __data["actualProviderKind"] }
        /// Safe actual model profile.
        public var actualModelProfile: String? { __data["actualModelProfile"] }
        /// Immutable policy snapshot.
        public var executionPolicy: ExecutionPolicy { __data["executionPolicy"] }
        /// Safe terminal error code.
        public var errorCode: String? { __data["errorCode"] }
        /// Safe terminal error message.
        public var errorMessage: String? { __data["errorMessage"] }
        /// Completed provider calls.
        public var providerCallCount: Int { __data["providerCallCount"] }
        /// Dispatched tool calls.
        public var toolCallCount: Int { __data["toolCallCount"] }
        /// Cumulative input tokens.
        public var inputTokens: Int { __data["inputTokens"] }
        /// Cumulative cached-input tokens.
        public var cachedInputTokens: Int { __data["cachedInputTokens"] }
        /// Cumulative output tokens.
        public var outputTokens: Int { __data["outputTokens"] }
        /// Active execution duration in milliseconds.
        public var activeMilliseconds: Int { __data["activeMilliseconds"] }
        /// Queue timestamp.
        public var queuedAt: String { __data["queuedAt"] }
        /// Start timestamp.
        public var startedAt: String? { __data["startedAt"] }
        /// End timestamp.
        public var endedAt: String? { __data["endedAt"] }
        /// Creation timestamp.
        public var createdAt: String { __data["createdAt"] }
        /// Last update timestamp.
        public var updatedAt: String { __data["updatedAt"] }

        public struct Fragments: FragmentContainer {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          public var tasksRunFields: TasksRunFields { _toFragment() }
        }

        public typealias Model = TasksRunFields.Model

        public typealias ExecutionPolicy = TasksRunFields.ExecutionPolicy
      }

      /// Task.Submission
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
          TasksDetailQuery.Data.Task.Submission.self,
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

        public typealias Criterium = TasksSubmissionFields.Criterium

        public typealias Artifact = TasksSubmissionFields.Artifact
      }

      /// Task.Review
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
          TasksDetailQuery.Data.Task.Review.self,
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

      /// Task.Artifact
      ///
      /// Parent Type: `Artifact`
      nonisolated public struct Artifact: NoemaAPI.SelectionSet {
        @_spi(Unsafe) public let __data: DataDict
        @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

        @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.Artifact }
        @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("artifactId", String.self),
          .field("title", String.self),
          .field("artifactKind", String.self),
          .field("storageKind", GraphQLEnum<NoemaAPI.ArtifactStorageKind>.self),
          .field("currentVersion", CurrentVersion.self),
        ] }
        @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
          TasksDetailQuery.Data.Task.Artifact.self
        ] }

        /// Stable artifact id.
        public var artifactId: String { __data["artifactId"] }
        /// Human-readable artifact title.
        public var title: String { __data["title"] }
        /// Product-defined artifact kind label.
        public var artifactKind: String { __data["artifactKind"] }
        /// Durable storage family shared by every version.
        public var storageKind: GraphQLEnum<NoemaAPI.ArtifactStorageKind> { __data["storageKind"] }
        /// Current immutable version.
        public var currentVersion: CurrentVersion { __data["currentVersion"] }

        /// Task.Artifact.CurrentVersion
        ///
        /// Parent Type: `ArtifactVersion`
        nonisolated public struct CurrentVersion: NoemaAPI.SelectionSet {
          @_spi(Unsafe) public let __data: DataDict
          @_spi(Unsafe) public init(_dataDict: DataDict) { __data = _dataDict }

          @_spi(Execution) public static var __parentType: any ApolloAPI.ParentType { NoemaAPI.Objects.ArtifactVersion }
          @_spi(Execution) public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .field("artifactVersionId", String.self),
            .field("mediaType", String?.self),
            .field("downloadUrl", String?.self),
            .field("externalUrl", String?.self),
          ] }
          @_spi(Execution) public static var __fulfilledFragments: [any ApolloAPI.SelectionSet.Type] { [
            TasksDetailQuery.Data.Task.Artifact.CurrentVersion.self
          ] }

          /// Stable artifact version id.
          public var artifactVersionId: String { __data["artifactVersionId"] }
          /// Optional media type for the version payload.
          public var mediaType: String? { __data["mediaType"] }
          /// Local download route when the version is stored in Noema.
          public var downloadUrl: String? { __data["downloadUrl"] }
          /// Durable external URL when the version is externally hosted.
          public var externalUrl: String? { __data["externalUrl"] }
        }
      }

      public typealias Stage = TasksCommandTaskFields.Stage

      public typealias Schedule = TasksCommandTaskFields.Schedule

      public typealias ActiveGate = TasksCommandTaskFields.ActiveGate

      public typealias CurrentRun = TasksCommandTaskFields.CurrentRun
    }
  }
}
