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

export const WorkTaskHistoryDocument = gql`
  query WorkTaskHistory($workspaceId: String!, $projectId: String, $kind: TerminalTaskKind, $text: String, $first: Int = 50, $after: String) {
    taskHistory(workspaceId: $workspaceId, projectId: $projectId, kind: $kind, text: $text, first: $first, after: $after) {
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
    workTasks(input: { workspaceId: $workspaceId, scope: ACTIVE }, first: 16) {
      edges {
        node {
          taskId
          title
          stage { name }
          currentRun { activityLabel }
          attention { kind }
        }
      }
    }
    needsYou(workspaceId: $workspaceId, first: 5) {
      edges {
        node {
          kind
          summary
          task {
            taskId
            title
          }
        }
      }
    }
  }
`;

export const WorkTaskEditFieldsDocument = gql`
  query WorkTaskEditFields($taskId: String!) {
    task(taskId: $taskId) {
      taskId
      title
      description
      revision
      generation
      executorAgentId
      executorBackend
      cwdOverride
      effectiveCwd
      effectiveCwdSource
      project { projectId name folder }
      schedule { scheduledFor timeZone missedRunPolicy recurrenceId recurrenceRevision recurrenceScheduledFor }
      activeGate { gateId kind }
    }
  }
`;

export const WorkTaskReferenceDocument = gql`
  query WorkTaskReference($taskId: String!) {
    task(taskId: $taskId) {
      ...WorkCommandTaskFields
      attention {
        kind
        title
        summary
      }
    }
  }
  ${WorkCommandTaskFields}
`;

export const WorkTaskDetailDocument = gql`
  query WorkTaskDetail($taskId: String!) {
    task(taskId: $taskId) {
      ...WorkCommandTaskFields
      project {
        ...WorkProjectFields
      }
      createdAt
      source {
        conversationId
      }
      currentContract {
        ...WorkContractFields
      }
      latestReview {
        ...WorkReviewSummaryFields
      }
      completedResult {
        ...WorkSubmissionFields
      }
      attention {
        kind
        title
        summary
      }
      messages {
        messageId
        bodyMarkdown
        author
        createdAt
      }
      runs {
        ...WorkRunFields
      }
      submissions {
        ...WorkSubmissionFields
      }
      reviews {
        ...WorkReviewFields
      }
    }
  }
  ${WorkCommandTaskFields}
  ${WorkProjectFields}
  ${WorkContractFields}
  ${WorkSubmissionFields}
  ${WorkReviewSummaryFields}
  ${WorkRunFields}
  ${WorkReviewFields}
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

export const WorkTaskRuntimeEventsDocument = gql`
  subscription WorkTaskRuntimeEvents($taskId: String!) {
    taskRuntimeEvents(taskId: $taskId) {
      taskId
      runId
    }
  }
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

export const WorkScheduleTaskDocument = gql`
  mutation WorkScheduleTask($input: ScheduleTaskInput!) {
    scheduleTask(input: $input) { task { ...WorkCommandTaskFields } eventCursor clientMutationId }
  }
  ${WorkCommandTaskFields}
`;

export const WorkRescheduleTaskDocument = gql`
  mutation WorkRescheduleTask($input: ScheduleTaskInput!) {
    rescheduleTask(input: $input) { task { ...WorkCommandTaskFields } eventCursor clientMutationId }
  }
  ${WorkCommandTaskFields}
`;

export const WorkUnscheduleTaskDocument = gql`
  mutation WorkUnscheduleTask($input: UnscheduleTaskInput!) {
    unscheduleTask(input: $input) { task { ...WorkCommandTaskFields } eventCursor clientMutationId }
  }
  ${WorkCommandTaskFields}
`;

export const WorkRunScheduledTaskNowDocument = gql`
  mutation WorkRunScheduledTaskNow($input: RunScheduledTaskNowInput!) {
    runScheduledTaskNow(input: $input) { task { ...WorkCommandTaskFields } eventCursor clientMutationId }
  }
  ${WorkCommandTaskFields}
`;

export const WorkTaskSchedulePreviewDocument = gql`
  query WorkTaskSchedulePreview($input: TaskSchedulePreviewInput!) {
    taskSchedulePreview(input: $input) { resolvedStart occurrences }
  }
`;

export const WorkTaskRecurrenceDocument = gql`
  query WorkTaskRecurrence($recurrenceId: String!, $first: Int = 30) {
    taskRecurrence(recurrenceId: $recurrenceId, first: $first) {
      recurrenceId title description startsAt cronExpression timeZone missedRunPolicy overlapPolicy
      lifecycle revision nextRunAt pendingCoalescedAt
      occurrences { recurrenceRevision scheduledFor localSlot trigger resolution taskId createdAt }
    }
  }
`;

export const WorkUpdateTaskRecurrenceDocument = gql`
  mutation WorkUpdateTaskRecurrence($input: UpdateTaskRecurrenceInput!) {
    updateTaskRecurrence(input: $input) { task { ...WorkCommandTaskFields } eventCursor clientMutationId }
  }
  ${WorkCommandTaskFields}
`;

export const WorkPauseTaskRecurrenceDocument = gql`
  mutation WorkPauseTaskRecurrence($input: TaskRecurrenceCommandInput!) {
    pauseTaskRecurrence(input: $input) { eventCursor clientMutationId }
  }
`;

export const WorkResumeTaskRecurrenceDocument = gql`
  mutation WorkResumeTaskRecurrence($input: TaskRecurrenceCommandInput!) {
    resumeTaskRecurrence(input: $input) { eventCursor clientMutationId }
  }
`;

export const WorkSkipTaskRecurrenceNextDocument = gql`
  mutation WorkSkipTaskRecurrenceNext($input: TaskRecurrenceCommandInput!) {
    skipTaskRecurrenceNext(input: $input) { eventCursor clientMutationId }
  }
`;

export const WorkEndTaskRecurrenceDocument = gql`
  mutation WorkEndTaskRecurrence($input: TaskRecurrenceCommandInput!) {
    endTaskRecurrence(input: $input) { eventCursor clientMutationId }
  }
`;

export const WorkRunTaskRecurrenceNowDocument = gql`
  mutation WorkRunTaskRecurrenceNow($input: TaskRecurrenceCommandInput!) {
    runTaskRecurrenceNow(input: $input) { task { ...WorkCommandTaskFields } eventCursor clientMutationId }
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
