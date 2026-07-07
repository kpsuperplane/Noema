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
    providerAccountCatalog {
      providerKind
      displayName
      authMethod
      capabilities {
        capabilityId
        status
        reliabilityContract
        dataFlowClass
      }
    }
    providerAccounts {
      providerAccountId
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

export const CreateProviderAccountDocument = gql`
  mutation CreateProviderAccount($input: CreateProviderAccountInput!) {
    createProviderAccount(input: $input) {
      providerAccountId
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

export const SaveProviderSecretInputDocument = gql`
  mutation SaveProviderSecretInput($input: ProviderSecretInput!) {
    saveProviderSecretInput(input: $input) {
      providerAccountId
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

export const ClearProviderSecretDocument = gql`
  mutation ClearProviderSecret($input: ClearProviderSecretInput!) {
    clearProviderSecret(input: $input) {
      providerAccountId
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

export const DeleteProviderAccountDocument = gql`
  mutation DeleteProviderAccount($input: DeleteProviderAccountInput!) {
    deleteProviderAccount(input: $input)
  }
`;

export const AgentsDocument = gql`
  query Agents {
    agents {
      agentId
      displayName
      isPrimary
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

export const SaveAgentModelPreferenceDocument = gql`
  mutation SaveAgentModelPreference($input: SaveAgentModelPreferenceInput!) {
    saveAgentModelPreference(input: $input) {
      providerKind
      providerAccountId
      modelProfile
      reasoningEffort
    }
  }
`;

export const WebFetchSettingsDocument = gql`
  query WebFetchSettings {
    webFetchSettings {
      summarizer {
        defaultModelProfile
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
  }
`;

export const SaveWebFetchSummarizerPreferenceDocument = gql`
  mutation SaveWebFetchSummarizerPreference($input: SaveWebFetchSummarizerPreferenceInput!) {
    saveWebFetchSummarizerPreference(input: $input) {
      providerKind
      providerAccountId
      modelProfile
      reasoningEffort
    }
  }
`;

export const WebToolSettingsDocument = gql`
  query WebToolSettings {
    webToolSettings {
      search {
        toolName
        capabilityId
        activeProviderAccountId
        providerOptions {
          providerAccountId
          providerKind
          accountKey
          displayName
          capabilityId
          reliabilityContract
          dataFlowClass
          citations
          directUrlFetch
        }
      }
      fetch {
        toolName
        capabilityId
        activeProviderAccountId
        providerOptions {
          providerAccountId
          providerKind
          accountKey
          displayName
          capabilityId
          reliabilityContract
          dataFlowClass
          citations
          directUrlFetch
        }
      }
    }
  }
`;

export const SaveWebToolProviderBindingDocument = gql`
  mutation SaveWebToolProviderBinding($input: SaveWebToolProviderBindingInput!) {
    saveWebToolProviderBinding(input: $input) {
      toolName
      capabilityId
      activeProviderAccountId
      providerOptions {
        providerAccountId
        providerKind
        accountKey
        displayName
        capabilityId
        reliabilityContract
        dataFlowClass
        citations
        directUrlFetch
      }
    }
  }
`;

export const UsageSettingsDocument = gql`
  query UsageSettings {
    usageSettings {
      progressAudit {
        defaultModelProfile
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
  }
`;

export const SaveToolProgressAuditPreferenceDocument = gql`
  mutation SaveToolProgressAuditPreference($input: SaveToolProgressAuditPreferenceInput!) {
    saveToolProgressAuditPreference(input: $input) {
      providerKind
      providerAccountId
      modelProfile
      reasoningEffort
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
      browserOauthReauthenticationSupported
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
        ownerExtractors {
          source
          selectorKind
          path
        }
        reviewedBy
        reviewedMetadataFingerprint
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

export const SaveToolCalibrationsDocument = gql`
  mutation SaveToolCalibrations($inputs: [SaveToolCalibrationInput!]!) {
    saveToolCalibrations(inputs: $inputs) {
      calibrationId
      mcpToolId
      status
      readClassification
      writeClassification
      exportClassification
    }
  }
`;

export const AutofillToolCalibrationsDocument = gql`
  mutation AutofillToolCalibrations($mcpServerId: String!) {
    autofillToolCalibrations(mcpServerId: $mcpServerId) {
      suggestions {
        mcpToolId
        readClassification
        writeClassification
        exportClassification
        disabled
        ownerExtractors {
          source
          selectorKind
          path
        }
      }
    }
  }
`;

const McpServerSetupResultFields = gql`
  fragment McpServerSetupResultFields on McpServerSetupResult {
    setupStatus
    discoveryStatus
    discoveredToolCount
    setupError
    auth {
      oauthClientCredentialsSupported
      oauthAuthorizationSupported
      scopes
    }
    server {
      mcpServerId
      displayName
      transportKind
      enabled
      healthStatus
      authStatus
      toolCount
      browserOauthReauthenticationSupported
    }
  }
`;

const McpOAuthSetupAttemptFields = gql`
  fragment McpOAuthSetupAttemptFields on McpOAuthSetupAttempt {
    attemptId
    status
    authorizationUrl
    errorMessage
    setupResult {
      ...McpServerSetupResultFields
    }
  }
  ${McpServerSetupResultFields}
`;

export const CreateMcpServerDocument = gql`
  mutation CreateMcpServer($input: CreateMcpServerInput!) {
    createMcpServer(input: $input) {
      ...McpServerSetupResultFields
    }
  }
  ${McpServerSetupResultFields}
`;

export const ContinueMcpServerSetupDocument = gql`
  mutation ContinueMcpServerSetup($input: ContinueMcpServerSetupInput!) {
    continueMcpServerSetup(input: $input) {
      ...McpServerSetupResultFields
    }
  }
  ${McpServerSetupResultFields}
`;

export const StartMcpServerOauthSetupDocument = gql`
  mutation StartMcpServerOauthSetup($input: StartMcpServerOAuthSetupInput!) {
    startMcpServerOauthSetup(input: $input) {
      ...McpOAuthSetupAttemptFields
    }
  }
  ${McpOAuthSetupAttemptFields}
`;

export const StartMcpServerReauthenticationOauthSetupDocument = gql`
  mutation StartMcpServerReauthenticationOauthSetup($input: StartMcpServerReauthenticationOAuthSetupInput!) {
    startMcpServerReauthenticationOauthSetup(input: $input) {
      ...McpOAuthSetupAttemptFields
    }
  }
  ${McpOAuthSetupAttemptFields}
`;

export const McpOauthSetupAttemptDocument = gql`
  query McpOauthSetupAttempt($attemptId: String!) {
    mcpOauthSetupAttempt(attemptId: $attemptId) {
      ...McpOAuthSetupAttemptFields
    }
  }
  ${McpOAuthSetupAttemptFields}
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
  query MemoryGraph($input: MemoryGraphInput) {
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
      ... on ErrorNotice {
        message
        recoverable
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

export const ConversationEventsDocument = gql`
  subscription ConversationEvents($conversationId: String!) {
    conversationEvents(conversationId: $conversationId) {
      __typename
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
          ... on ErrorNotice {
            message
            recoverable
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
