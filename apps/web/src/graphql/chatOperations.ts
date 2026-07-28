import { gql } from "@apollo/client";

export const ProviderAuthAttemptDocument = gql`
  query ProviderAuthAttempt($attemptId: String!) {
    providerAuthAttempt(attemptId: $attemptId) {
      attemptId
      providerKind
      providerAccountId
      method
      status
      verificationUrl
      userCode
      instructions
      errorCode
      errorMessage
    }
  }
`;

export const StartProviderAuthAttemptDocument = gql`
  mutation StartProviderAuthAttempt($input: StartProviderAuthAttemptInput!) {
    startProviderAuthAttempt(input: $input) {
      attemptId
      providerKind
      providerAccountId
      method
      status
      verificationUrl
      userCode
      instructions
      errorCode
      errorMessage
    }
  }
`;

export const ConversationItemFields = gql`
  fragment ConversationItemFields on ConversationItem {
    itemId
    cursor
    turnId
    metadata
    item {
      __typename
      ... on UserText {
        text
      }
      ... on AssistantText {
        text
      }
      ... on Activity {
        id
        activityKind
        status
        title
        summary
        metadata
      }
      ... on A2UiCard {
        id
        schema
        payload
      }
      ... on MultipleChoicePrompt {
        prompt
        selectionMode
        options {
          id
          label
        }
      }
      ... on MultipleChoiceSelection {
        promptItemId
        selectionMode
        selectedOptions {
          id
          label
        }
      }
      ... on ErrorNotice {
        message
        recoverable
      }
      ... on ArtifactReference {
        artifactId
        artifactVersionId
        title
        artifactKind
        storageKind
        externalUrl
        downloadUrl
        mediaType
      }
      ... on TaskReference {
        taskId
      }
    }
  }
`;

export const ConversationTranscriptPageFields = gql`
  fragment ConversationTranscriptPageFields on ConversationTranscriptPage {
    items {
      ...ConversationItemFields
    }
    pageInfo {
      beforeCursor
      hasMoreBefore
      limit
    }
  }
  ${ConversationItemFields}
`;

export const ChatBootDocument = gql`
  query ChatBoot($transcriptLimit: Int = 80) {
    localStatus {
      localService
      assistantConnection
      memoryStorage
      primaryAgentDisplayName
    }
    onboardingStatus {
      isUserOnboarded
      steps {
        id
        status
        providerKind
        providerAccountId
        accountKey
        displayName
        providerAccountStatus
        authMethod
      }
    }
    primaryConversation {
      conversationId
      provider
      latestTranscriptPage(limit: $transcriptLimit) {
        ...ConversationTranscriptPageFields
      }
    }
  }
  ${ConversationTranscriptPageFields}
`;

export const PrimaryConversationDocument = gql`
  query PrimaryConversation {
    primaryConversation {
      conversationId
      provider
    }
  }
`;

export const EnsurePrimaryConversationDocument = gql`
  mutation EnsurePrimaryConversation {
    ensurePrimaryConversation {
      conversationId
      provider
    }
  }
`;

export const ConversationTranscriptPageDocument = gql`
  query ConversationTranscriptPage($input: ConversationTranscriptPageInput!) {
    conversationTranscriptPage(input: $input) {
      ...ConversationTranscriptPageFields
    }
  }
  ${ConversationTranscriptPageFields}
`;

export const SendConversationTurnDocument = gql`
  mutation SendConversationTurn($input: SendConversationTurnInput!) {
    sendConversationTurn(input: $input) {
      conversationId
      clientMessageId
    }
  }
`;

export const SendMultipleChoiceSelectionDocument = gql`
  mutation SendMultipleChoiceSelection($input: SendMultipleChoiceSelectionInput!) {
    sendMultipleChoiceSelection(input: $input) {
      conversationId
      clientMessageId
    }
  }
`;

export const ConversationEventsDocument = gql`
  subscription ConversationEvents($conversationId: String!) {
    conversationEvents(conversationId: $conversationId) {
      __typename
      ... on HumanInterventionsChangedEvent {
        conversationId
      }
      ... on ConversationItemEvent {
        conversationId
        clientMessageId
        itemId
        cursor
        turnId
        metadata
        item {
          __typename
          ... on UserText {
            text
          }
          ... on AssistantText {
            text
          }
          ... on Activity {
            id
            activityKind
            status
            title
            summary
            metadata
          }
          ... on A2UiCard {
            id
            schema
            payload
          }
          ... on MultipleChoicePrompt {
            prompt
            selectionMode
            options {
              id
              label
            }
          }
          ... on MultipleChoiceSelection {
            promptItemId
            selectionMode
            selectedOptions {
              id
              label
            }
          }
          ... on ErrorNotice {
            message
            recoverable
          }
          ... on ArtifactReference {
            artifactId
            artifactVersionId
            title
            artifactKind
            storageKind
            externalUrl
            downloadUrl
            mediaType
          }
          ... on TaskReference {
            taskId
          }
        }
      }
      ... on AssistantTextDeltaEvent {
        conversationId
        deltaTurnId: turnId
        streamId
        responseIndex
        delta
      }
      ... on AgentStatusEvent {
        conversationId
        status
      }
      ... on TurnCompletedEvent {
        conversationId
        clientMessageId
      }
    }
  }
`;

export const ArtifactsDocument = gql`
  query Artifacts($ownerObjectType: String!, $ownerObjectId: String!, $limit: Int) {
    artifacts(ownerObjectType: $ownerObjectType, ownerObjectId: $ownerObjectId, limit: $limit) {
      artifactId
      ownerObjectType
      ownerObjectId
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
`;

export const ArtifactVersionDetailDocument = gql`
  query ArtifactVersionDetail($artifactVersionId: String!) {
    artifactVersionDetail(artifactVersionId: $artifactVersionId) {
      artifactVersionId
      artifactId
      versionIndex
      title
      artifactKind
      storageKind
      mediaType
      previewKind
      markdown
      plainText
      downloadUrl
      externalUrl
      versions {
        artifactVersionId
        versionIndex
        downloadUrl
        externalUrl
        mediaType
      }
    }
  }
`;
