import { gql } from "@apollo/client";

export const LocalStatusDocument = gql`
  query LocalStatus {
    localStatus {
      localService
      assistantConnection
      memoryStorage
      primaryAgentDisplayName
    }
  }
`;

export const OnboardingStatusDocument = gql`
  query OnboardingStatus {
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
  }
`;

export const ProviderAccountsDocument = gql`
  query ProviderAccounts {
    providerAccounts {
      providerKind
      accountKey
      displayName
      authMethod
      status
      isActive
      isDefault
      lastCheckedAt
      lastAuthenticatedAt
      lastErrorCode
      lastErrorMessage
    }
  }
`;

export const AgentsDocument = gql`
  query Agents {
    agents {
      agentId
      displayName
      isPrimary
    }
  }
`;

export const McpSettingsDocument = gql`
  query McpSettings {
    mcpServers {
      mcpServerId
      displayName
      transportKind
      enabled
      healthStatus
      authStatus
      toolCount
    }
  }
`;

export const McpToolsDocument = gql`
  query McpTools($mcpServerId: String!) {
    mcpTools(mcpServerId: $mcpServerId) {
      mcpToolId
      mcpServerId
      name
      description
      inputSchema
      outputSchema
      annotations
      metadataFingerprint
      calibration {
        calibrationId
        mcpToolId
        status
        readClassification
        writeClassification
        exportClassification
      }
    }
  }
`;

export const TrustedIdentitySettingsDocument = gql`
  query TrustedIdentitySettings($ownerScopeId: String!) {
    trustedIdentitySelectors(ownerScopeId: $ownerScopeId) {
      selectorId
      ownerScopeId
      selectorKind
      normalizedValue
      effect
      issuerActorId
    }
  }
`;

export const McpApprovalSettingsDocument = gql`
  query McpApprovalSettings($status: String) {
    mcpApprovalRequests(status: $status) {
      approvalId
      actionSummary
      toolInvocationId
      mcpServerId
      mcpToolId
      requesterActorId
      ownerScopeId
      activeScopeId
      destinationSummary
      dataSourceSummary
      sourceOwnerIdentity
      sourceOwnerTrust
      destinationOwnerIdentity
      destinationOwnerTrust
      exportSummary
      payloadPreview
      status
    }
  }
`;

export const SaveToolCalibrationDocument = gql`
  mutation SaveToolCalibration($input: GraphqlSaveToolCalibrationInput!) {
    saveToolCalibration(input: $input) {
      calibrationId
      mcpToolId
      status
      readClassification
      writeClassification
      exportClassification
    }
  }
`;

export const CreateMcpServerDocument = gql`
  mutation CreateMcpServer($input: GraphqlCreateMcpServerInput!) {
    createMcpServer(input: $input) {
      setupStatus
      discoveryStatus
      discoveredToolCount
      setupError
      server {
        mcpServerId
        displayName
        transportKind
        enabled
        healthStatus
        authStatus
        toolCount
      }
    }
  }
`;

export const ContinueMcpServerSetupDocument = gql`
  mutation ContinueMcpServerSetup($input: GraphqlContinueMcpServerSetupInput!) {
    continueMcpServerSetup(input: $input) {
      setupStatus
      discoveryStatus
      discoveredToolCount
      setupError
      server {
        mcpServerId
        displayName
        transportKind
        enabled
        healthStatus
        authStatus
        toolCount
      }
    }
  }
`;

export const DeleteMcpServerDocument = gql`
  mutation DeleteMcpServer($mcpServerId: String!) {
    deleteMcpServer(mcpServerId: $mcpServerId)
  }
`;

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

export const MemoryGraphDocument = gql`
  query MemoryGraph($input: GraphqlMemoryGraphInput) {
    memoryGraph(input: $input) {
      nodes {
        nodeId
        entityId
        label
        entityType
        redacted
        claimCount
      }
      edges {
        claimId
        sourceNodeId
        targetNodeId
        predicateId
        predicateLabel
        fact
        factRedacted
        status
        sensitivity
        confidence
        evidenceCount
        createdAt
        updatedAt
      }
      summary {
        returnedClaimCount
        returnedNodeCount
        limit
        truncated
      }
    }
  }
`;

export const MemoryGraphClaimDetailDocument = gql`
  query MemoryGraphClaimDetail($claimId: String!) {
    memoryClaim(claimId: $claimId) {
      claimId
      fact
      predicateId
      predicateLabel
      subjectEntityId
      subjectEntityName
      subjectEntityType
      objectEntityId
      objectEntityName
      objectEntityType
      status
      sensitivity
      confidence
      evidenceCount
      createdAt
      updatedAt
      evidence {
        evidenceId
        sourceItemId
        authority
        excerpt
        observedAt
        createdAt
      }
    }
  }
`;

export const StartProviderAuthAttemptDocument = gql`
  mutation StartProviderAuthAttempt($input: GraphqlStartProviderAuthAttemptInput!) {
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

export const StartPrimaryConversationDocument = gql`
  mutation StartPrimaryConversation {
    startPrimaryConversation {
      conversationId
      provider
      replay {
        itemId
        turnId
        item {
          __typename
          ... on GraphqlUserText {
            text
          }
          ... on GraphqlAssistantText {
            text
          }
          ... on GraphqlActivity {
            id
            activityKind
            status
            title
            summary
            metadata
          }
          ... on GraphqlA2UiCard {
            id
            schema
            payload
          }
          ... on GraphqlErrorNotice {
            message
            recoverable
          }
        }
      }
    }
  }
`;

export const SendConversationTurnDocument = gql`
  mutation SendConversationTurn($input: GraphqlSendConversationTurnInput!) {
    sendConversationTurn(input: $input) {
      conversationId
      clientMessageId
    }
  }
`;

export const ConversationEventsDocument = gql`
  subscription ConversationEvents($conversationId: String!) {
    conversationEvents(conversationId: $conversationId) {
      __typename
      ... on GraphqlConversationItemEvent {
        conversationId
        clientMessageId
        itemId
        turnId
        metadata
        item {
          __typename
          ... on GraphqlUserText {
            text
          }
          ... on GraphqlAssistantText {
            text
          }
          ... on GraphqlActivity {
            id
            activityKind
            status
            title
            summary
            metadata
          }
          ... on GraphqlA2UiCard {
            id
            schema
            payload
          }
          ... on GraphqlErrorNotice {
            message
            recoverable
          }
        }
      }
      ... on GraphqlAssistantTextDeltaEvent {
        conversationId
        deltaTurnId: turnId
        streamId
        delta
      }
      ... on GraphqlAgentStatusEvent {
        conversationId
        status
      }
      ... on GraphqlTurnCompletedEvent {
        conversationId
        clientMessageId
      }
    }
  }
`;
