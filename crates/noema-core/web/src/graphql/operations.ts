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
        reviewedBy
        reviewedMetadataFingerprint
      }
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
        title
        taskStatus: status
        revision
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
            title
            taskStatus: status
            revision
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

export const TaskDetailDocument = gql`
  query TaskDetail($taskId: String!) {
    task(taskId: $taskId) {
      taskId
      title
      requestMarkdown
      complexity
      status
      ownerHumanId
      source {
        conversationId
        turnId
        itemId
      }
      createdByAgentId
      creationToolCallId
      poolEntryId
      executorModel {
        providerKind
        providerAccountId
        selectionMode
        modelProfile
        reasoningEffort
        selectionSource
      }
      reviewerModel {
        providerKind
        providerAccountId
        selectionMode
        modelProfile
        reasoningEffort
        selectionSource
      }
      revisionIndex
      maxReviewRounds
      finalSubmissionId
      latestRunId
      terminalReason
      errorCode
      errorMessage
      createdAt
      updatedAt
      completedAt
      resumable
      cancellable
      blockingQuestion
      criteria {
        criterionId
        ordinal
        description
        expectedEvidence
      }
      submissions {
        submissionId
        executorRunId
        revisionIndex
        summary
        resultMarkdown
        criteria {
          criterionId
          evidenceMarkdown
        }
        createdAt
      }
      reviews {
        reviewId
        reviewerRunId
        reviewedSubmissionId
        overallVerdict
        overallFeedback
        criteria {
          criterionId
          outcome
          evidenceMarkdown
          feedback
        }
        createdAt
      }
      runs {
        runId
        runKind
        agentId
        revisionIndex
        attemptIndex
        status
        model {
          providerKind
          providerAccountId
          selectionMode
          modelProfile
          reasoningEffort
          selectionSource
        }
        actualProviderKind
        actualModelProfile
        triggeringSubmissionId
        triggeringReviewId
        errorCode
        errorMessage
        executionPolicy {
          maxProviderContinuations
          maxToolCalls
          maxActiveMinutes
          progressAuditInterval
        }
        providerCallCount
        toolCallCount
        cachedInputTokens
        activeMilliseconds
        inputTokens
        outputTokens
        queuedAt
        startedAt
        endedAt
        createdAt
        updatedAt
      }
    }
  }
`;

export const ResumeTaskDocument = gql`
  mutation ResumeTask($taskId: String!, $message: String) {
    resumeTask(taskId: $taskId, message: $message) {
      taskId
      status
      latestRunId
      errorCode
      errorMessage
      updatedAt
    }
  }
`;

export const CancelTaskDocument = gql`
  mutation CancelTask($taskId: String!) {
    cancelTask(taskId: $taskId) {
      taskId
      status
      latestRunId
      errorCode
      errorMessage
      updatedAt
    }
  }
`;

export const TaskRunItemsDocument = gql`
  query TaskRunItems($runId: String!, $after: String, $first: Int = 50) {
    taskRunItems(runId: $runId, after: $after, first: $first) {
      items {
        itemId
        runId
        sequenceIndex
        roundIndex
        kind
        status
        correlationId
        parentItemId
        contentText
        payload
        createdAt
        updatedAt
      }
      pageInfo {
        endCursor
        hasNextPage
      }
    }
  }
`;

export const TaskEventsDocument = gql`
  subscription TaskEvents($taskId: String!, $after: String) {
    taskEvents(taskId: $taskId, after: $after) {
      cursor
      kind
      taskId
      runId
      status
      createdAt
      item {
        itemId
        runId
        sequenceIndex
        roundIndex
        kind
        status
        correlationId
        parentItemId
        contentText
        payload
        createdAt
        updatedAt
      }
      run {
        runId
        runKind
        agentId
        revisionIndex
        attemptIndex
        status
        model {
          providerKind
          providerAccountId
          selectionMode
          modelProfile
          reasoningEffort
          selectionSource
        }
        actualProviderKind
        actualModelProfile
        triggeringSubmissionId
        triggeringReviewId
        errorCode
        errorMessage
        executionPolicy {
          maxProviderContinuations
          maxToolCalls
          maxActiveMinutes
          progressAuditInterval
        }
        providerCallCount
        toolCallCount
        cachedInputTokens
        activeMilliseconds
        inputTokens
        outputTokens
        queuedAt
        startedAt
        endedAt
        createdAt
        updatedAt
      }
    }
  }
`;

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
