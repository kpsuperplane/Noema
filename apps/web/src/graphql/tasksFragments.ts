import { gql } from "@apollo/client";

export const TasksPageInfoFields = gql`
  fragment TasksPageInfoFields on PageInfo {
    endCursor
    hasNextPage
  }
`;

export const TasksProjectFields = gql`
  fragment TasksProjectFields on Project {
    projectId
    workspaceId
    name
    description
    folder
    revision
    archivedAt
    createdAt
    updatedAt
  }
`;

export const TasksStageFields = gql`
  fragment TasksStageFields on WorkflowStage {
    stageId
    workflowId
    key
    name
    displayOrder
    behavior
  }
`;

export const TasksGateFields = gql`
  fragment TasksGateFields on TaskGate {
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

export const TasksReviewSummaryFields = gql`
  fragment TasksReviewSummaryFields on TaskReviewSummary {
    reviewId
    reviewedSubmissionId
    reviewAttemptIndex
    supersedesReviewId
    verdict
    feedback
    createdAt
  }
`;

export const TasksCurrentRunFields = gql`
  fragment TasksCurrentRunFields on CurrentRunSummary {
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

export const TasksTaskCardFields = gql`
  fragment TasksTaskCardFields on TaskCard {
    taskId
    workspace {
      workspaceId
      name
      description
      isPersonal
      membershipRole
    }
    project {
      ...TasksProjectFields
    }
    title
    descriptionPreview
    stage {
      ...TasksStageFields
    }
    revision
    generation
    executorAgentId
    executorBackend
    cwdOverride
    effectiveCwd
    effectiveCwdSource
    schedule {
      scheduledFor timeZone missedRunPolicy recurrenceId recurrenceRevision recurrenceScheduledFor
    }
    createdAt
    updatedAt
    completedAt
    currentRun {
      ...TasksCurrentRunFields
    }
    activeGate {
      ...TasksGateFields
    }
    latestReview {
      ...TasksReviewSummaryFields
    }
    validActions
  }
  ${TasksProjectFields}
  ${TasksStageFields}
  ${TasksCurrentRunFields}
  ${TasksGateFields}
  ${TasksReviewSummaryFields}
`;

export const TasksTaskSummaryFields = gql`
  fragment TasksTaskSummaryFields on TaskSummary {
    taskId
    workspace {
      workspaceId
      name
      description
      isPersonal
      membershipRole
    }
    project {
      ...TasksProjectFields
    }
    title
    descriptionPreview
    stage {
      ...TasksStageFields
    }
    revision
    generation
    executorAgentId
    executorBackend
    cwdOverride
    effectiveCwd
    effectiveCwdSource
    schedule {
      scheduledFor timeZone missedRunPolicy recurrenceId recurrenceRevision recurrenceScheduledFor
    }
    createdAt
    updatedAt
    completedAt
    currentRun {
      ...TasksCurrentRunFields
    }
    activeGate {
      ...TasksGateFields
    }
    latestReview {
      ...TasksReviewSummaryFields
    }
    attention {
      kind
      title
      summary
      validActions
      gate {
        ...TasksGateFields
      }
    }
    validActions
  }
  ${TasksProjectFields}
  ${TasksStageFields}
  ${TasksCurrentRunFields}
  ${TasksGateFields}
  ${TasksReviewSummaryFields}
`;

export const TasksEventFields = gql`
  fragment TasksEventFields on TasksEvent {
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

export const TasksCommandTaskFields = gql`
  fragment TasksCommandTaskFields on TaskDetail {
    taskId
    title
    description
    stage {
      ...TasksStageFields
    }
    revision
    generation
    executorAgentId
    executorBackend
    cwdOverride
    effectiveCwd
    effectiveCwdSource
    updatedAt
    schedule {
      scheduledFor timeZone missedRunPolicy recurrenceId recurrenceRevision recurrenceScheduledFor
    }
    completedAt
    validActions
    activeGate {
      ...TasksGateFields
    }
    currentRun {
      ...TasksCurrentRunFields
    }
  }
  ${TasksStageFields}
  ${TasksGateFields}
  ${TasksCurrentRunFields}
`;

export const TasksModelFields = gql`
  fragment TasksModelFields on TaskModelSnapshot {
    providerKind
    providerAccountId
    providerInstanceKey
    selectionMode
    modelProfile
    reasoningEffort
    selectionSource
  }
`;

export const TasksPolicyFields = gql`
  fragment TasksPolicyFields on TaskExecutionPolicy {
    maxProviderContinuations
    maxToolCalls
    maxActiveMinutes
    progressAuditInterval
    maxAutomaticRetries
    maxReviewRounds
  }
`;

export const TasksContractFields = gql`
  fragment TasksContractFields on TaskExecutionContract {
    requestMarkdown
    criteria { criterionId ordinal description expectedEvidence }
    complexity
    executionPolicy { ...TasksPolicyFields }
    executorAgentId
    executorBackend
    effectiveCwd
  }
  ${TasksPolicyFields}
`;

export const TasksSubmissionFields = gql`
  fragment TasksSubmissionFields on TaskSubmission {
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

export const TasksReviewFields = gql`
  fragment TasksReviewFields on TaskReview {
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

export const TasksRunFields = gql`
  fragment TasksRunFields on TaskRun {
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
    model { ...TasksModelFields }
    executorBackend
    executorAgentId
    effectiveCwd
    acpSessionId
    actualProviderKind
    actualModelProfile
    executionPolicy { ...TasksPolicyFields }
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
  ${TasksModelFields}
  ${TasksPolicyFields}
`;
