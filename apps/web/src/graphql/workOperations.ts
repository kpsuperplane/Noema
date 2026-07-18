import { gql } from "@apollo/client";
import {
  WorkCommandTaskFields,
  WorkContractFields,
  WorkEventFields,
  WorkGateFields,
  WorkPageInfoFields,
  WorkProjectFields,
  WorkReviewFields,
  WorkReviewSummaryFields,
  WorkRunFields,
  WorkStageFields,
  WorkSubmissionFields,
  WorkTaskCardFields,
  WorkTaskSummaryFields
} from "./workFragments";

export const WorkProjectsDocument = gql`
  query WorkProjects($workspaceId: String!, $includeArchived: Boolean!, $first: Int = 100, $after: String) {
    projects(workspaceId: $workspaceId, includeArchived: $includeArchived, first: $first, after: $after) {
      edges {
        cursor
        node {
          ...WorkProjectFields
        }
      }
      pageInfo {
        ...WorkPageInfoFields
      }
    }
  }
  ${WorkProjectFields}
  ${WorkPageInfoFields}
`;

export const WorkOverviewDocument = gql`
  query WorkOverview($workspaceId: String!, $projectId: String) {
    workOverview(workspaceId: $workspaceId, projectId: $projectId) {
      workspace {
        workspaceId
        name
        description
        isPersonal
        membershipRole
      }
      workflow {
        workflowId
        name
        stages {
          ...WorkStageFields
        }
      }
      activeColumns {
        stage {
          ...WorkStageFields
        }
        taskCount
      }
      recentTasks {
        edges {
          cursor
          node {
            ...WorkTaskSummaryFields
          }
        }
        pageInfo {
          ...WorkPageInfoFields
        }
      }
      needsYouCount
    }
  }
  ${WorkStageFields}
  ${WorkTaskSummaryFields}
  ${WorkPageInfoFields}
`;

export const WorkTasksDocument = gql`
  query WorkTasks($input: WorkTasksInput!, $first: Int = 50, $after: String) {
    workTasks(input: $input, first: $first, after: $after) {
      edges {
        cursor
        node {
          ...WorkTaskSummaryFields
        }
      }
      pageInfo {
        ...WorkPageInfoFields
      }
    }
  }
  ${WorkTaskSummaryFields}
  ${WorkPageInfoFields}
`;

export const WorkNeedsYouDocument = gql`
  query WorkNeedsYou($workspaceId: String!, $projectId: String, $first: Int = 50, $after: String) {
    needsYou(workspaceId: $workspaceId, projectId: $projectId, first: $first, after: $after) {
      edges {
        cursor
        node {
          kind
          title
          summary
          validActions
          gate {
            ...WorkGateFields
          }
          review {
            reviewId
            verdict
            feedback
            createdAt
          }
          task {
            ...WorkTaskCardFields
          }
        }
      }
      pageInfo {
        ...WorkPageInfoFields
      }
    }
  }
  ${WorkGateFields}
  ${WorkTaskCardFields}
  ${WorkPageInfoFields}
`;

export const WorkActivityDocument = gql`
  query WorkActivity($workspaceId: String!, $projectId: String, $taskId: String, $first: Int = 50, $after: String) {
    workActivity(workspaceId: $workspaceId, projectId: $projectId, taskId: $taskId, first: $first, after: $after) {
      edges {
        cursor
        node {
          ...WorkEventFields
        }
      }
      pageInfo {
        ...WorkPageInfoFields
      }
    }
  }
  ${WorkEventFields}
  ${WorkPageInfoFields}
`;

export const WorkCompletedTasksDocument = gql`
  query WorkCompletedTasks($workspaceId: String!, $projectId: String, $kind: TerminalTaskKind, $text: String, $first: Int = 50, $after: String) {
    completedTasks(workspaceId: $workspaceId, projectId: $projectId, kind: $kind, text: $text, first: $first, after: $after) {
      edges {
        cursor
        node {
          ...WorkTaskSummaryFields
        }
      }
      pageInfo {
        ...WorkPageInfoFields
      }
    }
  }
  ${WorkTaskSummaryFields}
  ${WorkPageInfoFields}
`;

export const WorkPanelDocument = gql`
  query WorkPanel($workspaceId: String!) {
    workTasks(input: { workspaceId: $workspaceId, scope: ACTIVE }, first: 8) {
      edges {
        node {
          ...WorkTaskSummaryFields
        }
      }
    }
    needsYou(workspaceId: $workspaceId, first: 5) {
      edges {
        node {
          kind
          title
          summary
          validActions
          task {
            ...WorkTaskCardFields
          }
        }
      }
    }
  }
  ${WorkTaskSummaryFields}
  ${WorkTaskCardFields}
`;

export const WorkTaskEditFieldsDocument = gql`
  query WorkTaskEditFields($taskId: String!) {
    task(taskId: $taskId) {
      taskId
      title
      description
      revision
      generation
      project { projectId name }
      activeGate { gateId kind }
    }
  }
`;

export const WorkTaskDetailDocument = gql`
  query WorkTaskDetail($taskId: String!) {
    task(taskId: $taskId) {
      ...WorkCommandTaskFields
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
      descriptionPreview
      createdAt
      source {
        sourceKind
        conversationId
        turnId
        itemId
      }
      workflow {
        workflowId
        name
        stages {
          ...WorkStageFields
        }
      }
      currentContract {
        ...WorkContractFields
      }
      latestSubmission {
        ...WorkSubmissionFields
      }
      acceptedResult {
        ...WorkSubmissionFields
      }
      latestReview {
        ...WorkReviewSummaryFields
      }
      attention {
        kind
        title
        summary
        validActions
      }
      contracts(first: 20) {
        edges { cursor node { ...WorkContractFields } }
        pageInfo { ...WorkPageInfoFields }
      }
      gates(first: 20) {
        edges { cursor node { ...WorkGateFields } }
        pageInfo { ...WorkPageInfoFields }
      }
      messages(first: 20) {
        edges {
          cursor
          node {
            messageId
            taskGeneration
            kind
            bodyMarkdown
            author
            gateId
            contractId
            approvalDecision
            consumedByRunId
            consumedAt
            createdAt
          }
        }
        pageInfo { ...WorkPageInfoFields }
      }
      runs(first: 20) {
        edges { cursor node { ...WorkRunFields } }
        pageInfo { ...WorkPageInfoFields }
      }
      submissions(first: 20) {
        edges { cursor node { ...WorkSubmissionFields } }
        pageInfo { ...WorkPageInfoFields }
      }
      reviews(first: 20) {
        edges { cursor node { ...WorkReviewFields } }
        pageInfo { ...WorkPageInfoFields }
      }
      artifacts(first: 20) {
        edges {
          cursor
          node {
            artifactId
            title
            description
            artifactKind
            storageKind
            currentVersion {
              artifactVersionId
              versionIndex
              externalUrl
              downloadUrl
              mediaType
            }
          }
        }
        pageInfo { ...WorkPageInfoFields }
      }
      activity(first: 20) {
        edges { cursor node { ...WorkEventFields } }
        pageInfo { ...WorkPageInfoFields }
      }
    }
  }
  ${WorkCommandTaskFields}
  ${WorkProjectFields}
  ${WorkStageFields}
  ${WorkContractFields}
  ${WorkSubmissionFields}
  ${WorkReviewSummaryFields}
  ${WorkGateFields}
  ${WorkRunFields}
  ${WorkReviewFields}
  ${WorkEventFields}
  ${WorkPageInfoFields}
`;

export const WorkTaskRunItemsDocument = gql`
  query WorkTaskRunItems($runId: String!, $first: Int = 50, $after: String) {
    taskRunItems(runId: $runId, first: $first, after: $after) {
      edges {
        cursor
        node {
          itemId
          runId
          cursor
          sequenceIndex
          roundIndex
          kind
          status
          correlationId
          parentItemId
          contentText
          payload
          createdAt
          updatedAt
        }
      }
      pageInfo { ...WorkPageInfoFields }
    }
  }
  ${WorkPageInfoFields}
`;

export const WorkEventsDocument = gql`
  subscription WorkEvents($workspaceId: String!, $after: String) {
    workEvents(workspaceId: $workspaceId, after: $after) { ...WorkEventFields }
  }
  ${WorkEventFields}
`;

export const WorkTaskEventsDocument = gql`
  subscription WorkTaskEvents($taskId: String!, $after: String) {
    taskEvents(taskId: $taskId, after: $after) { ...WorkEventFields }
  }
  ${WorkEventFields}
`;

export const WorkCreateProjectDocument = gql`
  mutation WorkCreateProject($input: CreateProjectInput!) {
    createProject(input: $input) { project { ...WorkProjectFields } eventCursor clientMutationId }
  }
  ${WorkProjectFields}
`;

export const WorkUpdateProjectDocument = gql`
  mutation WorkUpdateProject($input: UpdateProjectInput!) {
    updateProject(input: $input) { project { ...WorkProjectFields } eventCursor clientMutationId }
  }
  ${WorkProjectFields}
`;

export const WorkArchiveProjectDocument = gql`
  mutation WorkArchiveProject($input: ArchiveProjectInput!) {
    archiveProject(input: $input) { project { ...WorkProjectFields } eventCursor clientMutationId }
  }
  ${WorkProjectFields}
`;

export const WorkReopenProjectDocument = gql`
  mutation WorkReopenProject($input: ReopenProjectInput!) {
    reopenProject(input: $input) { project { ...WorkProjectFields } eventCursor clientMutationId }
  }
  ${WorkProjectFields}
`;

export const WorkCaptureTaskDocument = gql`
  mutation WorkCaptureTask($input: CaptureTaskInput!) {
    captureTask(input: $input) { task { ...WorkCommandTaskFields } eventCursor clientMutationId }
  }
  ${WorkCommandTaskFields}
`;

export const WorkUpdateInboxTaskDocument = gql`
  mutation WorkUpdateInboxTask($input: UpdateInboxTaskInput!) {
    updateInboxTask(input: $input) { task { ...WorkCommandTaskFields } eventCursor clientMutationId }
  }
  ${WorkCommandTaskFields}
`;

export const WorkQueueTaskDocument = gql`
  mutation WorkQueueTask($input: QueueTaskInput!) {
    queueTask(input: $input) { task { ...WorkCommandTaskFields } eventCursor clientMutationId }
  }
  ${WorkCommandTaskFields}
`;

export const WorkAnswerTaskDocument = gql`
  mutation WorkAnswerTask($input: AnswerTaskInput!) {
    answerTask(input: $input) { task { ...WorkCommandTaskFields } eventCursor clientMutationId }
  }
  ${WorkCommandTaskFields}
`;

export const WorkRetryTaskDocument = gql`
  mutation WorkRetryTask($input: RetryTaskInput!) {
    retryTask(input: $input) { task { ...WorkCommandTaskFields } eventCursor clientMutationId }
  }
  ${WorkCommandTaskFields}
`;

export const WorkAcceptTaskDocument = gql`
  mutation WorkAcceptTask($input: AcceptTaskInput!) {
    acceptTask(input: $input) { task { ...WorkCommandTaskFields } eventCursor clientMutationId }
  }
  ${WorkCommandTaskFields}
`;

export const WorkRequestTaskChangesDocument = gql`
  mutation WorkRequestTaskChanges($input: RequestTaskChangesInput!) {
    requestTaskChanges(input: $input) { task { ...WorkCommandTaskFields } eventCursor clientMutationId }
  }
  ${WorkCommandTaskFields}
`;

export const WorkCancelTaskDocument = gql`
  mutation WorkCancelTask($input: CancelTaskInput!) {
    cancelTask(input: $input) { task { ...WorkCommandTaskFields } eventCursor clientMutationId }
  }
  ${WorkCommandTaskFields}
`;

export const WorkReopenTaskDocument = gql`
  mutation WorkReopenTask($input: ReopenTaskInput!) {
    reopenTask(input: $input) { task { ...WorkCommandTaskFields } eventCursor clientMutationId }
  }
  ${WorkCommandTaskFields}
`;
