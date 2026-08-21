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

export const TasksCurrentRunFields = gql`
  fragment TasksCurrentRunFields on CurrentRunSummary {
    runId
    instanceName
    kind
    status
    attemptIndex
    queuedAt
    startedAt
    updatedAt
    activityLabel
  }
`;

export const TasksTaskReferenceSummaryFields = gql`
  fragment TasksTaskReferenceSummaryFields on TaskSummary {
    taskId
    title
    stage {
      stageId
      name
      behavior
    }
    completedAt
    currentRun {
      runId
      kind
      activityLabel
    }
    attention {
      title
    }
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
    taskDocumentPreview
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
    validActions
  }
  ${TasksProjectFields}
  ${TasksStageFields}
  ${TasksCurrentRunFields}
  ${TasksGateFields}
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
    taskDocumentPreview
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
    taskDocument
    taskDocumentDigest
    project { ...TasksProjectFields }
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
  ${TasksProjectFields}
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

export const TasksRunFields = gql`
  fragment TasksRunFields on TaskRun {
    runId
    instanceName
    kind
    status
    agentId
    taskGeneration
    attemptIndex
    reviewRound
    parentRunId
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
