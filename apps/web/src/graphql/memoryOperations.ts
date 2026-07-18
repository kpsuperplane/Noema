import { gql } from "@apollo/client";

export const MemorySettingsDocument = gql`
  query MemorySettings {
    memorySettings {
      mode
      baseUrl
      port
      status {
        status
        checkedAt
        lastErrorCode
        lastErrorMessage
      }
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

export const MemoryGraphDocument = gql`
  query MemoryGraph($page: Int, $limit: Int) {
    memoryGraph(input: { page: $page, limit: $limit }) {
      status {
        status
        lastErrorCode
        lastErrorMessage
      }
      article {
        title
        subtitle
        markdown
        isGenerated
        generatedAt
      }
      documents {
        id
        title
        summary
        type
        createdAt
        updatedAt
        memoryEntries {
          id
          citationKey
          documentId
          content
          summary
          title
          type
          source {
            kind
            conversationId
            turnId
            itemId
            messageText
          }
          metadata
          createdAt
          updatedAt
          spaceContainerTag
          relation
          parentMemoryId
          rootMemoryId
          memoryRelations
          isLatest
          spaceId
        }
      }
      pageInfo {
        page
        limit
        hasMore
        total
      }
    }
  }
`;

export const RegenerateMemoryArticleDocument = gql`
  mutation RegenerateMemoryArticle {
    regenerateMemoryArticle {
      title
      subtitle
      markdown
      isGenerated
      generatedAt
    }
  }
`;

export const SaveMemoryServiceSettingsDocument = gql`
  mutation SaveMemoryServiceSettings($input: SaveMemoryServiceSettingsInput!) {
    saveMemoryServiceSettings(input: $input) {
      mode
      baseUrl
      port
      status {
        status
        checkedAt
        lastErrorCode
        lastErrorMessage
      }
    }
  }
`;

export const CheckMemoryServiceDocument = gql`
  mutation CheckMemoryService {
    checkMemoryService {
      status
      checkedAt
      lastErrorCode
      lastErrorMessage
    }
  }
`;
