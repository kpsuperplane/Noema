import { gql } from "@apollo/client";

export const MemorySettingsDocument = gql`
  query MemorySettings {
    memorySettings {
      modelPreference {
        providerKind
        providerAccountId
        modelProfile
        reasoningEffort
        isOverride
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
        icon
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
          icon
          excerpt
          hash
        }
      }
      pages {
        id
        path
        title
        icon
      }
      pendingCount
      updateStatus {
        state
        active
        operation
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
      icon
      body
      hash
      sourceReferences {
        source
        excerpt
      }
      ancestors {
        id
        path
        title
      }
      children {
        id
        path
        title
        icon
        excerpt
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
        icon
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
          icon
          excerpt
          hash
        }
      }
      pages {
        id
        path
        title
        icon
      }
      pendingCount
      updateStatus {
        state
        active
        operation
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
        operation
        lastConsolidatedSequence
        lastConsolidatedItem
        error
        updatedAt
      }
    }
  }
`;

export const RegenerateMemoryIconsDocument = gql`
  mutation RegenerateMemoryIcons {
    regenerateMemoryIcons {
      accepted
      status {
        state
        active
        operation
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
      isOverride
    }
  }
`;
