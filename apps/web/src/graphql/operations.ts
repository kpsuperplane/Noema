import { gql } from "@apollo/client";

export const AgentModelPreferenceFields = gql`
  fragment AgentModelPreferenceFields on AgentModelPreference {
    providerKind
    providerAccountId
    modelProfile
    reasoningEffort
    selectionMode
    fastMode
  }
`;

export const AgentModelProviderOptionFields = gql`
  fragment AgentModelProviderOptionFields on AgentModelProviderOption {
    providerKind
    providerAccountId
    providerDisplayName
    status
    disabledReason
    profiles { id label disabledReason reasoningEfforts defaultReasoningEffort }
    recommendations { useCase modelProfile reasoningEffort disabledReason }
  }
`;

export const AgentFields = gql`
  fragment AgentFields on Agent {
    agentId
    displayName
    isPrimary
    modelPreference { ...AgentModelPreferenceFields }
    modelOptions { ...AgentModelProviderOptionFields }
  }
  ${AgentModelPreferenceFields}
  ${AgentModelProviderOptionFields}
`;

export const WebToolProviderOptionFields = gql`
  fragment WebToolProviderOptionFields on WebToolProviderOption {
    providerAccountId
    providerKind
    accountKey
    displayName
    capabilityId
    reliabilityContract
    dataFlowClass
    citations
    directUrlFetch
    jsRendering
    authenticatedContext
  }
`;

export const WebToolBindingSettingsFields = gql`
  fragment WebToolBindingSettingsFields on WebToolBindingSettings {
    toolName
    capabilityId
    activeProviderAccountId
    providerRouteAccountIds
    providerOptions { ...WebToolProviderOptionFields }
  }
  ${WebToolProviderOptionFields}
`;
export const LocalStatusDocument = gql`
  query LocalStatus {
    localStatus {
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

export const OnboardingModelSetupDocument = gql`
  query OnboardingModelSetup($providerAccountId: String!) {
    onboardingModelSetup(providerAccountId: $providerAccountId) {
      providerKind
      providerAccountId
      providerDisplayName
      profiles {
        id
        label
        disabledReason
        reasoningEfforts
        defaultReasoningEffort
      }
      recommendations { useCase modelProfile reasoningEffort disabledReason }
      proposedSelections {
        noema { selectionMode modelProfile reasoningEffort fastMode }
        simpleTasks { selectionMode modelProfile reasoningEffort fastMode }
        mediumTasks { selectionMode modelProfile reasoningEffort fastMode }
        difficultTasks { selectionMode modelProfile reasoningEffort fastMode }
        taskReviewer { selectionMode modelProfile reasoningEffort fastMode }
        webFetchSummarizer { selectionMode modelProfile reasoningEffort fastMode }
        toolProgressAudit { selectionMode modelProfile reasoningEffort fastMode }
        actionReviewer { selectionMode modelProfile reasoningEffort fastMode }
        memoryConsolidation { selectionMode modelProfile reasoningEffort fastMode }
      }
    }
  }
`;

export const ConfirmOnboardingModelSelectionsDocument = gql`
  mutation ConfirmOnboardingModelSelections($input: ConfirmOnboardingModelSelectionsInput!) {
    confirmOnboardingModelSelections(input: $input) {
      isUserOnboarded
      steps {
        id
        status
        providerKind
        providerAccountId
      }
    }
  }
`;

export const ProviderAccountCatalogFields = gql`
  fragment ProviderAccountCatalogFields on ProviderAccountCatalogEntry {
      providerKind
      displayName
      preferredAuthMethod
      supportedAuthMethods
      capabilities {
        capabilityId
        status
        reliabilityContract
        dataFlowClass
      }
  }
`;

export const ProviderAccountFields = gql`
  fragment ProviderAccountFields on ProviderAccount {
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
      capabilities {
        capabilityId
        status
        reliabilityContract
        dataFlowClass
      }
  }
`;

export const ProvidersSettingsRootDocument = gql`
  query ProvidersSettingsRoot {
    providerAccountCatalog { ...ProviderAccountCatalogFields }
    providerAccounts { ...ProviderAccountFields }
    webToolSettings {
      search { ...WebToolBindingSettingsFields }
      fetch { ...WebToolBindingSettingsFields }
      browse { ...WebToolBindingSettingsFields }
    }
  }
  ${ProviderAccountCatalogFields}
  ${ProviderAccountFields}
  ${WebToolBindingSettingsFields}
`;

export const CreateProviderAccountDocument = gql`
  mutation CreateProviderAccount($input: CreateProviderAccountInput!) {
    createProviderAccount(input: $input) { ...ProviderAccountFields }
  }
  ${ProviderAccountFields}
`;

export const SaveProviderSecretInputDocument = gql`
  mutation SaveProviderSecretInput($input: ProviderSecretInput!) {
    saveProviderSecretInput(input: $input) { ...ProviderAccountFields }
  }
  ${ProviderAccountFields}
`;

export const ClearProviderSecretDocument = gql`
  mutation ClearProviderSecret($input: ClearProviderSecretInput!) {
    clearProviderSecret(input: $input) { ...ProviderAccountFields }
  }
  ${ProviderAccountFields}
`;

export const DeleteProviderAccountDocument = gql`
  mutation DeleteProviderAccount($input: DeleteProviderAccountInput!) {
    deleteProviderAccount(input: $input)
  }
`;

export const AcpAgentFields = gql`
  fragment AcpAgentFields on AcpAgent {
    agentId
    displayName
    command
    arguments
    enabled
    authStatus
    healthStatus
    implementationName
    implementationVersion
    capabilities
    connectionRevision
    lastError
  }
`;

export const AcpAgentsDocument = gql`
  query AcpAgents {
    acpAgents { ...AcpAgentFields }
  }
  ${AcpAgentFields}
`;

export const CreateAcpAgentDocument = gql`
  mutation CreateAcpAgent($input: CreateAcpAgentInput!) {
    createAcpAgent(input: $input) { ...AcpAgentFields }
  }
  ${AcpAgentFields}
`;

export const UpdateAcpAgentDocument = gql`
  mutation UpdateAcpAgent($input: UpdateAcpAgentInput!) {
    updateAcpAgent(input: $input) { ...AcpAgentFields }
  }
  ${AcpAgentFields}
`;

export const DeleteAcpAgentDocument = gql`
  mutation DeleteAcpAgent($input: DeleteAcpAgentInput!) {
    deleteAcpAgent(input: $input)
  }
`;

export const TestAcpAgentDocument = gql`
  mutation TestAcpAgent($input: TestAcpAgentInput!) {
    testAcpAgent(input: $input) { ...AcpAgentFields }
  }
  ${AcpAgentFields}
`;

export const AuthenticateAcpAgentDocument = gql`
  mutation AuthenticateAcpAgent($input: AuthenticateAcpAgentInput!) {
    authenticateAcpAgent(input: $input) { ...AcpAgentFields }
  }
  ${AcpAgentFields}
`;

export const SaveAgentModelPreferenceDocument = gql`
  mutation SaveAgentModelPreference($input: SaveAgentModelPreferenceInput!) {
    saveAgentModelPreference(input: $input) {
      providerKind
      providerAccountId
      modelProfile
      reasoningEffort
      selectionMode
      fastMode
    }
  }
`;

export const SaveWebFetchSummarizerPreferenceDocument = gql`
  mutation SaveWebFetchSummarizerPreference($input: SaveWebFetchSummarizerPreferenceInput!) {
    saveWebFetchSummarizerPreference(input: $input) { ...AgentModelPreferenceFields }
  }
  ${AgentModelPreferenceFields}
`;

export const WebSettingsRootDocument = gql`
  query WebSettingsRoot {
    webFetchSettings {
      summarizer {
        modelPreference { ...AgentModelPreferenceFields }
        modelOptions { ...AgentModelProviderOptionFields }
      }
    }
    webToolSettings {
      search { ...WebToolBindingSettingsFields }
      fetch { ...WebToolBindingSettingsFields }
      browse { ...WebToolBindingSettingsFields }
    }
  }
  ${AgentModelPreferenceFields}
  ${AgentModelProviderOptionFields}
  ${WebToolBindingSettingsFields}
`;

export const SaveWebToolProviderBindingDocument = gql`
  mutation SaveWebToolProviderBinding($input: SaveWebToolProviderBindingInput!) {
    saveWebToolProviderBinding(input: $input) { ...WebToolBindingSettingsFields }
  }
  ${WebToolBindingSettingsFields}
`;

export const SaveBrowserProviderRouteDocument = gql`
  mutation SaveBrowserProviderRoute($input: SaveBrowserProviderRouteInput!) {
    saveBrowserProviderRoute(input: $input) { ...WebToolBindingSettingsFields }
  }
  ${WebToolBindingSettingsFields}
`;

export const SaveToolProgressAuditPreferenceDocument = gql`
  mutation SaveToolProgressAuditPreference($input: SaveToolProgressAuditPreferenceInput!) {
    saveToolProgressAuditPreference(input: $input) { ...AgentModelPreferenceFields }
  }
  ${AgentModelPreferenceFields}
`;

export const PrivacySettingsDocument = gql`
  query PrivacySettings {
    privacySettings {
      reviewer {
        modelPreference { ...AgentModelPreferenceFields }
        modelOptions { ...AgentModelProviderOptionFields }
      }
    }
  }
  ${AgentModelPreferenceFields}
  ${AgentModelProviderOptionFields}
`;

export const SaveActionReviewerPreferenceDocument = gql`
  mutation SaveActionReviewerPreference($input: SaveActionReviewerPreferenceInput!) {
    saveActionReviewerPreference(input: $input) { ...AgentModelPreferenceFields }
  }
  ${AgentModelPreferenceFields}
`;
