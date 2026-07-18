import { gql } from "@apollo/client";
import {
  WorkContractFields,
  WorkEventFields,
  WorkGateFields,
  WorkPageInfoFields,
  WorkReviewFields,
  WorkRunFields,
  WorkSubmissionFields
} from "./workFragments";

export const WorkTaskContractsPageDocument = gql`
  query WorkTaskContractsPage($taskId: String!, $after: String, $first: Int = 20) {
    task(taskId: $taskId) {
      taskId
      contracts(after: $after, first: $first) { edges { cursor node { ...WorkContractFields } } pageInfo { ...WorkPageInfoFields } }
    }
  }
  ${WorkContractFields}
  ${WorkPageInfoFields}
`;

export const WorkTaskGatesPageDocument = gql`
  query WorkTaskGatesPage($taskId: String!, $after: String, $first: Int = 20) {
    task(taskId: $taskId) {
      taskId
      gates(after: $after, first: $first) { edges { cursor node { ...WorkGateFields } } pageInfo { ...WorkPageInfoFields } }
    }
  }
  ${WorkGateFields}
  ${WorkPageInfoFields}
`;

export const WorkTaskMessagesPageDocument = gql`
  query WorkTaskMessagesPage($taskId: String!, $after: String, $first: Int = 20) {
    task(taskId: $taskId) {
      taskId
      messages(after: $after, first: $first) {
        edges { cursor node { messageId taskGeneration kind bodyMarkdown author gateId contractId approvalDecision consumedByRunId consumedAt createdAt } }
        pageInfo { ...WorkPageInfoFields }
      }
    }
  }
  ${WorkPageInfoFields}
`;

export const WorkTaskRunsPageDocument = gql`
  query WorkTaskRunsPage($taskId: String!, $after: String, $first: Int = 20) {
    task(taskId: $taskId) {
      taskId
      runs(after: $after, first: $first) { edges { cursor node { ...WorkRunFields } } pageInfo { ...WorkPageInfoFields } }
    }
  }
  ${WorkRunFields}
  ${WorkPageInfoFields}
`;

export const WorkTaskSubmissionsPageDocument = gql`
  query WorkTaskSubmissionsPage($taskId: String!, $after: String, $first: Int = 20) {
    task(taskId: $taskId) {
      taskId
      submissions(after: $after, first: $first) { edges { cursor node { ...WorkSubmissionFields } } pageInfo { ...WorkPageInfoFields } }
    }
  }
  ${WorkSubmissionFields}
  ${WorkPageInfoFields}
`;

export const WorkTaskReviewsPageDocument = gql`
  query WorkTaskReviewsPage($taskId: String!, $after: String, $first: Int = 20) {
    task(taskId: $taskId) {
      taskId
      reviews(after: $after, first: $first) { edges { cursor node { ...WorkReviewFields } } pageInfo { ...WorkPageInfoFields } }
    }
  }
  ${WorkReviewFields}
  ${WorkPageInfoFields}
`;

export const WorkTaskArtifactsPageDocument = gql`
  query WorkTaskArtifactsPage($taskId: String!, $after: String, $first: Int = 20) {
    task(taskId: $taskId) {
      taskId
      artifacts(after: $after, first: $first) {
        edges { cursor node { artifactId title description artifactKind storageKind currentVersion { artifactVersionId versionIndex externalUrl downloadUrl mediaType } } }
        pageInfo { ...WorkPageInfoFields }
      }
    }
  }
  ${WorkPageInfoFields}
`;

export const WorkTaskActivityPageDocument = gql`
  query WorkTaskActivityPage($taskId: String!, $after: String, $first: Int = 20) {
    task(taskId: $taskId) {
      taskId
      activity(after: $after, first: $first) { edges { cursor node { ...WorkEventFields } } pageInfo { ...WorkPageInfoFields } }
    }
  }
  ${WorkEventFields}
  ${WorkPageInfoFields}
`;
