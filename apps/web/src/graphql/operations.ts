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
        noema { selectionMode modelProfile reasoningEffort }
        simpleTasks { selectionMode modelProfile reasoningEffort }
        mediumTasks { selectionMode modelProfile reasoningEffort }
        difficultTasks { selectionMode modelProfile reasoningEffort }
        taskReviewer { selectionMode modelProfile reasoningEffort }
        webFetchSummarizer { selectionMode modelProfile reasoningEffort }
        toolProgressAudit { selectionMode modelProfile reasoningEffort }
        actionReviewer { selectionMode modelProfile reasoningEffort }
        memoryConsolidation { selectionMode modelProfile reasoningEffort }
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

export const ProviderAccountsDocument = gql`
  query ProviderAccounts {
    providerAccountCatalog {
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
        selectionMode
      }
      modelOptions {
        providerKind
        providerAccountId
        providerDisplayName
        status
        disabledReason
        profiles {
          id
          label
          disabledReason
          reasoningEfforts
          defaultReasoningEffort
        }
        recommendations { useCase modelProfile reasoningEffort disabledReason }
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
      selectionMode
    }
  }
`;

export const WebFetchSettingsDocument = gql`
  query WebFetchSettings {
    webFetchSettings {
      summarizer {
        modelPreference {
          providerKind
          providerAccountId
          modelProfile
          reasoningEffort
          selectionMode
        }
        modelOptions {
          providerKind
          providerAccountId
          providerDisplayName
          status
          disabledReason
          profiles {
            id
            label
            disabledReason
            reasoningEfforts
            defaultReasoningEffort
          }
          recommendations { useCase modelProfile reasoningEffort disabledReason }
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
      selectionMode
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
        modelPreference {
          providerKind
          providerAccountId
          modelProfile
          reasoningEffort
          selectionMode
        }
        modelOptions {
          providerKind
          providerAccountId
          providerDisplayName
          status
          disabledReason
          profiles {
            id
            label
            disabledReason
            reasoningEfforts
            defaultReasoningEffort
          }
          recommendations { useCase modelProfile reasoningEffort disabledReason }
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
      selectionMode
    }
  }
`;

export const PrivacySettingsDocument = gql`
  query PrivacySettings {
    privacySettings {
      reviewer {
        modelPreference {
          providerKind
          providerAccountId
          modelProfile
          reasoningEffort
          selectionMode
        }
        modelOptions {
          providerKind
          providerAccountId
          providerDisplayName
          status
          disabledReason
          profiles {
            id
            label
            disabledReason
            reasoningEfforts
            defaultReasoningEffort
          }
          recommendations { useCase modelProfile reasoningEffort disabledReason }
        }
      }
    }
  }
`;

export const SaveActionReviewerPreferenceDocument = gql`
  mutation SaveActionReviewerPreference($input: SaveActionReviewerPreferenceInput!) {
    saveActionReviewerPreference(input: $input) {
      providerKind
      providerAccountId
      modelProfile
      reasoningEffort
      selectionMode
    }
  }
`;
