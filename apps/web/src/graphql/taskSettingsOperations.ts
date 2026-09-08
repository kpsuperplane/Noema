import { gql } from "@apollo/client";
import {
  AgentFields,
  AgentModelPreferenceFields,
  AgentModelProviderOptionFields
} from "./operations";

export const TaskModelPoolEntryFields = gql`
  fragment TaskModelPoolEntryFields on TaskModelPoolEntry {
    poolEntryId
    complexity
    label
    providerKind
    providerAccountId
    modelProfile
    reasoningEffort
    selectionMode
    fastMode
    sortOrder
    createdAt
    updatedAt
  }
`;

export const AgentsSettingsRootDocument = gql`
  query AgentsSettingsRoot {
    agents { ...AgentFields }
    taskModelPools { ...TaskModelPoolEntryFields }
  }
  ${AgentFields}
  ${TaskModelPoolEntryFields}
`;

export const UsageSettingsRootDocument = gql`
  query UsageSettingsRoot {
    usageSettings {
      progressAudit {
        modelPreference { ...AgentModelPreferenceFields }
        modelOptions { ...AgentModelProviderOptionFields }
      }
    }
    taskExecutionPolicy {
      maxProviderContinuations
      maxToolCalls
      maxActiveMinutes
      progressAuditInterval
    }
  }
  ${AgentModelPreferenceFields}
  ${AgentModelProviderOptionFields}
`;

export const UpdateTaskExecutionPolicyDocument = gql`
  mutation UpdateTaskExecutionPolicy($input: TaskExecutionPolicyInput!) {
    updateTaskExecutionPolicy(input: $input) {
      maxProviderContinuations
      maxToolCalls
      maxActiveMinutes
      progressAuditInterval
    }
  }
`;

export const UpdateTaskModelPoolEntryDocument = gql`
  mutation UpdateTaskModelPoolEntry($poolEntryId: String!, $input: TaskModelPoolEntryInput!) {
    updateTaskModelPoolEntry(poolEntryId: $poolEntryId, input: $input) { ...TaskModelPoolEntryFields }
  }
  ${TaskModelPoolEntryFields}
`;

export const CreateConversationExternalArtifactDocument = gql`
  mutation CreateConversationExternalArtifact($input: CreateConversationExternalArtifactInput!) {
    createConversationExternalArtifact(input: $input) {
      artifactId
      ownerObjectType
      ownerObjectId
      title
      storageKind
      currentVersion {
        versionIndex
        externalUrl
        downloadUrl
      }
    }
  }
`;
