import { gql } from "@apollo/client";

export const MemorySettingsDocument = gql`
  query MemorySettings {
    memorySettings {
      modelPreference {
        providerKind
        providerAccountId
        modelProfile
        reasoningEffort
      }
      modelOptions {
        providerKind
        providerAccountId
        providerDisplayName
        status
        disabledReason
        defaultModelProfile
        profiles {
          id
          label
          disabledReason
          reasoningEfforts
          defaultReasoningEffort
        }
      }
    }
  }
`;

export const MemoryTreeDocument = gql`
  query MemoryTree {
    memoryTree {
      root {
        id
        path
        title
        body
        hash
        sourceReferences {
          source
          excerpt
        }
        parent
        children {
          id
          path
          title
          hash
        }
      }
      pendingCount
      updateStatus {
        state
        active
        lastConsolidatedSequence
        lastConsolidatedItem
        error
        updatedAt
      }
    }
  }
`;

export const MemoryPageDocument = gql`
  query MemoryPage($pageId: String!) {
    memoryPage(pageId: $pageId) {
      id
      path
      title
      body
      hash
      sourceReferences {
        source
        excerpt
      }
      parent
      children {
        id
        path
        title
        hash
      }
    }
  }
`;

export const MemoryEventsDocument = gql`
  subscription MemoryEvents {
    memoryEvents {
      root {
        id
        path
        title
        body
        hash
        sourceReferences {
          source
          excerpt
        }
        parent
        children {
          id
          path
          title
          hash
        }
      }
      pendingCount
      updateStatus {
        state
        active
        lastConsolidatedSequence
        lastConsolidatedItem
        error
        updatedAt
      }
    }
  }
`;

export const UpdateMemoryDocument = gql`
  mutation UpdateMemory {
    updateMemory {
      accepted
      status {
        state
        active
        lastConsolidatedSequence
        lastConsolidatedItem
        error
        updatedAt
      }
    }
  }
`;

export const SaveMemoryModelPreferenceDocument = gql`
  mutation SaveMemoryModelPreference($input: GraphqlSaveMemoryModelPreferenceInput!) {
    saveMemoryModelPreference(input: $input) {
      providerKind
      providerAccountId
      modelProfile
      reasoningEffort
    }
  }
`;
