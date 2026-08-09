import { gql } from "@apollo/client";
import {
  TasksCommandTaskFields,
  TasksContractFields,
  TasksEventFields,
  TasksGateFields,
  TasksPageInfoFields,
  TasksProjectFields,
  TasksReviewFields,
  TasksReviewSummaryFields,
  TasksRunFields,
  TasksSubmissionFields,
  TasksTaskCardFields,
  TasksTaskSummaryFields
} from "./tasksFragments";

export const TasksProjectsDocument = gql`
  query TasksProjects($workspaceId: String!, $includeArchived: Boolean!, $first: Int = 100, $after: String) {
    projects(workspaceId: $workspaceId, includeArchived: $includeArchived, first: $first, after: $after) {
      edges {
        cursor
        node {
          ...TasksProjectFields
        }
      }
      pageInfo {
        ...TasksPageInfoFields
      }
    }
  }
  ${TasksProjectFields}
  ${TasksPageInfoFields}
`;

export const TasksListDocument = gql`
  query TasksList($input: TaskListInput!, $first: Int = 50, $after: String) {
    tasks(input: $input, first: $first, after: $after) {
      edges {
        cursor
        node {
          ...TasksTaskSummaryFields
        }
      }
      pageInfo {
        ...TasksPageInfoFields
      }
    }
  }
  ${TasksTaskSummaryFields}
  ${TasksPageInfoFields}
`;

export const TasksNeedsYouDocument = gql`
  query TasksNeedsYou($workspaceId: String!, $projectId: String, $first: Int = 50, $after: String) {
    needsYou(workspaceId: $workspaceId, projectId: $projectId, first: $first, after: $after) {
      edges {
        cursor
        node {
          kind
          title
          summary
          validActions
          gate {
            ...TasksGateFields
          }
          review {
            reviewId
            verdict
            feedback
            createdAt
          }
          task {
            ...TasksTaskCardFields
          }
        }
      }
      pageInfo {
        ...TasksPageInfoFields
      }
    }
  }
  ${TasksGateFields}
  ${TasksTaskCardFields}
  ${TasksPageInfoFields}
`;

export const TasksHistoryDocument = gql`
  query TasksHistory($workspaceId: String!, $projectId: String, $kind: TerminalTaskKind, $text: String, $first: Int = 50, $after: String) {
    taskHistory(workspaceId: $workspaceId, projectId: $projectId, kind: $kind, text: $text, first: $first, after: $after) {
      edges {
        cursor
        node {
          ...TasksTaskSummaryFields
        }
      }
      pageInfo {
        ...TasksPageInfoFields
      }
    }
  }
  ${TasksTaskSummaryFields}
  ${TasksPageInfoFields}
`;

export const TasksPanelDocument = gql`
  query TasksPanel($workspaceId: String!) {
    tasks(input: { workspaceId: $workspaceId, scope: ACTIVE }, first: 16) {
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

export const TasksTaskEditFieldsDocument = gql`
  query TasksTaskEditFields($taskId: String!) {
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

export const TasksTaskReferenceDocument = gql`
  query TasksTaskReference($taskId: String!) {
    task(taskId: $taskId) {
      ...TasksCommandTaskFields
      attention {
        kind
        title
        summary
      }
    }
  }
  ${TasksCommandTaskFields}
`;

export const TasksTaskDetailDocument = gql`
  query TasksTaskDetail($taskId: String!) {
    task(taskId: $taskId) {
      ...TasksCommandTaskFields
      project {
        ...TasksProjectFields
      }
      createdAt
      source {
        conversationId
      }
      currentContract {
        ...TasksContractFields
      }
      latestReview {
        ...TasksReviewSummaryFields
      }
      completedResult {
        ...TasksSubmissionFields
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
        ...TasksRunFields
      }
      submissions {
        ...TasksSubmissionFields
      }
      reviews {
        ...TasksReviewFields
      }
    }
  }
  ${TasksCommandTaskFields}
  ${TasksProjectFields}
  ${TasksContractFields}
  ${TasksSubmissionFields}
  ${TasksReviewSummaryFields}
  ${TasksRunFields}
  ${TasksReviewFields}
`;

export const TasksTaskRunItemsDocument = gql`
  query TasksTaskRunItems($runId: String!, $first: Int = 50, $after: String) {
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
      pageInfo { ...TasksPageInfoFields }
    }
  }
  ${TasksPageInfoFields}
`;

export const TasksEventsDocument = gql`
  subscription TasksEvents($workspaceId: String!, $after: String) {
    tasksEvents(workspaceId: $workspaceId, after: $after) { ...TasksEventFields }
  }
  ${TasksEventFields}
`;

export const TasksTaskEventsDocument = gql`
  subscription TasksTaskEvents($taskId: String!, $after: String) {
    taskEvents(taskId: $taskId, after: $after) { ...TasksEventFields }
  }
  ${TasksEventFields}
`;

export const TasksTaskRuntimeEventsDocument = gql`
  subscription TasksTaskRuntimeEvents($taskId: String!) {
    taskRuntimeEvents(taskId: $taskId) {
      taskId
      runId
    }
  }
`;

export const TasksCreateProjectDocument = gql`
  mutation TasksCreateProject($input: CreateProjectInput!) {
    createProject(input: $input) { project { ...TasksProjectFields } eventCursor clientMutationId }
  }
  ${TasksProjectFields}
`;

export const TasksUpdateProjectDocument = gql`
  mutation TasksUpdateProject($input: UpdateProjectInput!) {
    updateProject(input: $input) { project { ...TasksProjectFields } eventCursor clientMutationId }
  }
  ${TasksProjectFields}
`;

export const TasksArchiveProjectDocument = gql`
  mutation TasksArchiveProject($input: ArchiveProjectInput!) {
    archiveProject(input: $input) { project { ...TasksProjectFields } eventCursor clientMutationId }
  }
  ${TasksProjectFields}
`;

export const TasksReopenProjectDocument = gql`
  mutation TasksReopenProject($input: ReopenProjectInput!) {
    reopenProject(input: $input) { project { ...TasksProjectFields } eventCursor clientMutationId }
  }
  ${TasksProjectFields}
`;

export const TasksCaptureTaskDocument = gql`
  mutation TasksCaptureTask($input: CaptureTaskInput!) {
    captureTask(input: $input) { task { ...TasksCommandTaskFields } eventCursor clientMutationId }
  }
  ${TasksCommandTaskFields}
`;

export const TasksUpdateInboxTaskDocument = gql`
  mutation TasksUpdateInboxTask($input: UpdateInboxTaskInput!) {
    updateInboxTask(input: $input) { task { ...TasksCommandTaskFields } eventCursor clientMutationId }
  }
  ${TasksCommandTaskFields}
`;

export const TasksQueueTaskDocument = gql`
  mutation TasksQueueTask($input: QueueTaskInput!) {
    queueTask(input: $input) { task { ...TasksCommandTaskFields } eventCursor clientMutationId }
  }
  ${TasksCommandTaskFields}
`;

export const TasksScheduleTaskDocument = gql`
  mutation TasksScheduleTask($input: ScheduleTaskInput!) {
    scheduleTask(input: $input) { task { ...TasksCommandTaskFields } eventCursor clientMutationId }
  }
  ${TasksCommandTaskFields}
`;

export const TasksRescheduleTaskDocument = gql`
  mutation TasksRescheduleTask($input: ScheduleTaskInput!) {
    rescheduleTask(input: $input) { task { ...TasksCommandTaskFields } eventCursor clientMutationId }
  }
  ${TasksCommandTaskFields}
`;

export const TasksUnscheduleTaskDocument = gql`
  mutation TasksUnscheduleTask($input: UnscheduleTaskInput!) {
    unscheduleTask(input: $input) { task { ...TasksCommandTaskFields } eventCursor clientMutationId }
  }
  ${TasksCommandTaskFields}
`;

export const TasksRunScheduledTaskNowDocument = gql`
  mutation TasksRunScheduledTaskNow($input: RunScheduledTaskNowInput!) {
    runScheduledTaskNow(input: $input) { task { ...TasksCommandTaskFields } eventCursor clientMutationId }
  }
  ${TasksCommandTaskFields}
`;

export const TasksTaskSchedulePreviewDocument = gql`
  query TasksTaskSchedulePreview($input: TaskSchedulePreviewInput!) {
    taskSchedulePreview(input: $input) { resolvedStart occurrences }
  }
`;

export const TasksTaskRecurrenceDocument = gql`
  query TasksTaskRecurrence($recurrenceId: String!, $first: Int = 30) {
    taskRecurrence(recurrenceId: $recurrenceId, first: $first) {
      recurrenceId title description startsAt cronExpression timeZone missedRunPolicy overlapPolicy
      lifecycle revision nextRunAt pendingCoalescedAt
      occurrences { recurrenceRevision scheduledFor localSlot trigger resolution taskId createdAt }
    }
  }
`;

export const TasksUpdateTaskRecurrenceDocument = gql`
  mutation TasksUpdateTaskRecurrence($input: UpdateTaskRecurrenceInput!) {
    updateTaskRecurrence(input: $input) { task { ...TasksCommandTaskFields } eventCursor clientMutationId }
  }
  ${TasksCommandTaskFields}
`;

export const TasksPauseTaskRecurrenceDocument = gql`
  mutation TasksPauseTaskRecurrence($input: TaskRecurrenceCommandInput!) {
    pauseTaskRecurrence(input: $input) { eventCursor clientMutationId }
  }
`;

export const TasksResumeTaskRecurrenceDocument = gql`
  mutation TasksResumeTaskRecurrence($input: TaskRecurrenceCommandInput!) {
    resumeTaskRecurrence(input: $input) { eventCursor clientMutationId }
  }
`;

export const TasksSkipTaskRecurrenceNextDocument = gql`
  mutation TasksSkipTaskRecurrenceNext($input: TaskRecurrenceCommandInput!) {
    skipTaskRecurrenceNext(input: $input) { eventCursor clientMutationId }
  }
`;

export const TasksEndTaskRecurrenceDocument = gql`
  mutation TasksEndTaskRecurrence($input: TaskRecurrenceCommandInput!) {
    endTaskRecurrence(input: $input) { eventCursor clientMutationId }
  }
`;

export const TasksRunTaskRecurrenceNowDocument = gql`
  mutation TasksRunTaskRecurrenceNow($input: TaskRecurrenceCommandInput!) {
    runTaskRecurrenceNow(input: $input) { task { ...TasksCommandTaskFields } eventCursor clientMutationId }
  }
  ${TasksCommandTaskFields}
`;

export const TasksAnswerTaskDocument = gql`
  mutation TasksAnswerTask($input: AnswerTaskInput!) {
    answerTask(input: $input) { task { ...TasksCommandTaskFields } eventCursor clientMutationId }
  }
  ${TasksCommandTaskFields}
`;

export const TasksRetryTaskDocument = gql`
  mutation TasksRetryTask($input: RetryTaskInput!) {
    retryTask(input: $input) { task { ...TasksCommandTaskFields } eventCursor clientMutationId }
  }
  ${TasksCommandTaskFields}
`;

export const TasksCancelTaskDocument = gql`
  mutation TasksCancelTask($input: CancelTaskInput!) {
    cancelTask(input: $input) { task { ...TasksCommandTaskFields } eventCursor clientMutationId }
  }
  ${TasksCommandTaskFields}
`;

export const TasksReopenTaskDocument = gql`
  mutation TasksReopenTask($input: ReopenTaskInput!) {
    reopenTask(input: $input) { task { ...TasksCommandTaskFields } eventCursor clientMutationId }
  }
  ${TasksCommandTaskFields}
`;
