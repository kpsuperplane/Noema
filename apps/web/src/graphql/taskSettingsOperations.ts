import { gql } from "@apollo/client";

export const TaskExecutionPolicyDocument = gql`
  query TaskExecutionPolicy {
    taskExecutionPolicy {
      maxProviderContinuations
      maxToolCalls
      maxActiveMinutes
      progressAuditInterval
    }
  }
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

export const TaskModelPoolsDocument = gql`
  query TaskModelPools {
    taskModelPools {
      poolEntryId
      complexity
      label
      providerKind
      providerAccountId
      modelProfile
      reasoningEffort
      enabled
      sortOrder
      createdAt
      updatedAt
    }
  }
`;

export const UpdateTaskModelPoolEntryDocument = gql`
  mutation UpdateTaskModelPoolEntry($poolEntryId: String!, $input: TaskModelPoolEntryInput!) {
    updateTaskModelPoolEntry(poolEntryId: $poolEntryId, input: $input) {
      poolEntryId
      complexity
      label
      providerKind
      providerAccountId
      modelProfile
      reasoningEffort
      enabled
      sortOrder
      createdAt
      updatedAt
    }
  }
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
