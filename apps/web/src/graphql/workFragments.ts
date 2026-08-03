import { gql } from "@apollo/client";

export const WorkPageInfoFields = gql`
  fragment WorkPageInfoFields on PageInfo {
    endCursor
    hasNextPage
  }
`;

export const WorkProjectFields = gql`
  fragment WorkProjectFields on Project {
    projectId
    workspaceId
    name
    description
    revision
    archivedAt
    createdAt
    updatedAt
  }
`;

export const WorkStageFields = gql`
  fragment WorkStageFields on WorkflowStage {
    stageId
    workflowId
    key
    name
    displayOrder
    behavior
  }
`;

export const WorkGateFields = gql`
  fragment WorkGateFields on TaskGate {
    gateId
    taskGeneration
    kind
    state
    recoveryReason
    retryRunKind
    prompt
    contextMarkdown
    suggestedAnswers
    openedBy
    originatingRunId
    openedAt
    resolvedBy
    resolvedAt
    resolution
  }
`;

export const WorkReviewSummaryFields = gql`
  fragment WorkReviewSummaryFields on TaskReviewSummary {
    reviewId
    reviewedSubmissionId
    reviewAttemptIndex
    supersedesReviewId
    verdict
    feedback
    createdAt
  }
`;

export const WorkCurrentRunFields = gql`
  fragment WorkCurrentRunFields on CurrentRunSummary {
    runId
    instanceName
    kind
    status
    attemptIndex
    contractId
    queuedAt
    startedAt
    updatedAt
    activityLabel
  }
`;

export const WorkTaskCardFields = gql`
  fragment WorkTaskCardFields on TaskCard {
    taskId
    workspace {
      workspaceId
      name
      description
      isPersonal
      membershipRole
    }
    project {
      ...WorkProjectFields
    }
    title
    descriptionPreview
    stage {
      ...WorkStageFields
    }
    revision
    generation
    schedule {
      scheduledFor timeZone missedRunPolicy recurrenceId recurrenceRevision recurrenceScheduledFor
    }
    createdAt
    updatedAt
    completedAt
    currentRun {
      ...WorkCurrentRunFields
    }
    activeGate {
      ...WorkGateFields
    }
    latestReview {
      ...WorkReviewSummaryFields
    }
    validActions
  }
  ${WorkProjectFields}
  ${WorkStageFields}
  ${WorkCurrentRunFields}
  ${WorkGateFields}
  ${WorkReviewSummaryFields}
`;

export const WorkTaskSummaryFields = gql`
  fragment WorkTaskSummaryFields on TaskSummary {
    taskId
    workspace {
      workspaceId
      name
      description
      isPersonal
      membershipRole
    }
    project {
      ...WorkProjectFields
    }
    title
    descriptionPreview
    stage {
      ...WorkStageFields
    }
    revision
    generation
    schedule {
      scheduledFor timeZone missedRunPolicy recurrenceId recurrenceRevision recurrenceScheduledFor
    }
    createdAt
    updatedAt
    completedAt
    currentRun {
      ...WorkCurrentRunFields
    }
    activeGate {
      ...WorkGateFields
    }
    latestReview {
      ...WorkReviewSummaryFields
    }
    attention {
      kind
      title
      summary
      validActions
      gate {
        ...WorkGateFields
      }
    }
    validActions
  }
  ${WorkProjectFields}
  ${WorkStageFields}
  ${WorkCurrentRunFields}
  ${WorkGateFields}
  ${WorkReviewSummaryFields}
`;

export const WorkEventFields = gql`
  fragment WorkEventFields on WorkEvent {
    cursor
    eventId
    kind
    occurredAt
    workspaceId
    projectId
    taskId
    runId
    actor
    causationId
    correlationId
    payload
  }
`;

export const WorkCommandTaskFields = gql`
  fragment WorkCommandTaskFields on TaskDetail {
    taskId
    title
    description
    stage {
      ...WorkStageFields
    }
    revision
    generation
    updatedAt
    schedule {
      scheduledFor timeZone missedRunPolicy recurrenceId recurrenceRevision recurrenceScheduledFor
    }
    completedAt
    validActions
    activeGate {
      ...WorkGateFields
    }
    currentRun {
      ...WorkCurrentRunFields
    }
  }
  ${WorkStageFields}
  ${WorkGateFields}
  ${WorkCurrentRunFields}
`;

export const WorkModelFields = gql`
  fragment WorkModelFields on TaskModelSnapshot {
    providerKind
    providerAccountId
    providerInstanceKey
    selectionMode
    modelProfile
    reasoningEffort
    selectionSource
  }
`;

export const WorkPolicyFields = gql`
  fragment WorkPolicyFields on TaskExecutionPolicy {
    maxProviderContinuations
    maxToolCalls
    maxActiveMinutes
    progressAuditInterval
    maxAutomaticRetries
    maxReviewRounds
  }
`;

export const WorkContractFields = gql`
  fragment WorkContractFields on TaskExecutionContract {
    requestMarkdown
    criteria { criterionId ordinal description expectedEvidence }
    complexity
    executionPolicy { ...WorkPolicyFields }
  }
  ${WorkPolicyFields}
`;

export const WorkSubmissionFields = gql`
  fragment WorkSubmissionFields on TaskSubmission {
    submissionId
    contractId
    executorRunId
    reviewRound
    summary
    resultMarkdown
    criteria { criterionId evidenceMarkdown }
    artifacts { artifactId artifactVersionId title artifactKind storageKind mediaType downloadUrl externalUrl }
    createdAt
  }
`;

export const WorkReviewFields = gql`
  fragment WorkReviewFields on TaskReview {
    reviewId
    contractId
    reviewerRunId
    reviewedSubmissionId
    reviewAttemptIndex
    supersedesReviewId
    verdict
    feedback
    criteria { criterionId outcome evidenceMarkdown feedback }
    createdAt
  }
`;

export const WorkRunFields = gql`
  fragment WorkRunFields on TaskRun {
    runId
    instanceName
    kind
    status
    agentId
    taskGeneration
    contractId
    attemptIndex
    reviewRound
    parentRunId
    triggeringSubmissionId
    triggeringReviewId
    model { ...WorkModelFields }
    actualProviderKind
    actualModelProfile
    executionPolicy { ...WorkPolicyFields }
    errorCode
    errorMessage
    providerCallCount
    toolCallCount
    inputTokens
    cachedInputTokens
    outputTokens
    activeMilliseconds
    queuedAt
    startedAt
    endedAt
    createdAt
    updatedAt
  }
  ${WorkModelFields}
  ${WorkPolicyFields}
`;
