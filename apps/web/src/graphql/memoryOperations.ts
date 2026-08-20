import { gql } from "@apollo/client";
import {
  AgentModelPreferenceFields,
  AgentModelProviderOptionFields
} from "./operations";

export const MemorySettingsDocument = gql`
  query MemorySettings {
    memorySettings {
      modelPreference { ...AgentModelPreferenceFields }
      modelOptions { ...AgentModelProviderOptionFields }
    }
  }
  ${AgentModelPreferenceFields}
  ${AgentModelProviderOptionFields}
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
        citations {
          sources {
            source
            kind
            excerpt
            createdAt
          }
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
      citations {
        sources {
          source
          kind
          excerpt
          createdAt
        }
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
        citations {
          sources {
            source
            kind
            excerpt
            createdAt
          }
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
    saveMemoryModelPreference(input: $input) { ...AgentModelPreferenceFields }
  }
  ${AgentModelPreferenceFields}
`;
