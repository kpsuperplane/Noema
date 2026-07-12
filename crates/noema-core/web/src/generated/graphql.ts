/** Internal type. DO NOT USE DIRECTLY. */
type Exact<T extends { [key: string]: unknown }> = { [K in keyof T]: T[K] };
/** Internal type. DO NOT USE DIRECTLY. */
export type Incremental<T> = T | { [P in keyof T]?: P extends ' $fragmentName' | '__typename' ? T[P] : never };
import { TypedDocumentNode as DocumentNode } from '@graphql-typed-document-node/core';
/** Agent status exposed through GraphQL. */
export type AgentStatus =
  /** Conversation is in an error state. */
  | 'ERROR'
  /** No agent work is active. */
  | 'IDLE'
  /** Input has been accepted. */
  | 'INPUT_RECEIVED'
  /** Agent is interrupting a previous turn. */
  | 'INTERRUPTING'
  /** Agent is producing or planning. */
  | 'THINKING'
  /** Agent is waiting on a tool invocation. */
  | 'TOOL_RUNNING'
  /** A newer turn is waiting for a previous turn. */
  | 'WAITING_FOR_PREVIOUS_TURN_COMPLETION';

/** Artifact storage kind exposed through GraphQL. */
export type ArtifactStorageKind =
  /** Artifact content is referenced by an external durable URL. */
  | 'EXTERNAL_URL'
  /** Artifact bytes live in the local Noema filesystem. */
  | 'LOCAL_FILE';

/** Preview renderer selected for an artifact version detail panel. */
export type ArtifactVersionPreviewKind =
  /** The version points at an external URL. */
  | 'EXTERNAL'
  /** Local Markdown bytes are available as UTF-8 text. */
  | 'MARKDOWN'
  /** Local plain-text bytes are available as UTF-8 text. */
  | 'PLAIN_TEXT'
  /** The version exists but this first slice cannot render it inline. */
  | 'UNSUPPORTED';

/** Assistant connection exposed to clients. */
export type AssistantConnection =
  /** The daemon is using Codex for chat. */
  | 'CODEX';

/** Input for clearing a write-only provider secret. */
export type ClearProviderSecretInput = {
  providerAccountId: string;
};

/** Continue setup after adding authentication material. */
export type ContinueMcpServerSetupInput = {
  /** Durable MCP server id. */
  mcpServerId: string;
  /** OAuth client-secret credentials, when supported by the server. */
  oauthClientCredentials?: McpOAuthClientCredentialsInput | null | undefined;
  /** Secret environment variables stored on disk. */
  secretEnv?: unknown;
  /** Secret headers stored on disk. */
  secretHeaders?: unknown;
};

/** Input for reading a visible transcript page. */
export type ConversationTranscriptPageInput = {
  /** Durable Noema conversation id. */
  conversationId: string;
  /** Opaque cursor. When omitted, reads the latest page. */
  cursor?: string | null | undefined;
  /** Page size. Defaults to 80 and must be within 1..=200. */
  limit?: number | null | undefined;
};

/** Input for creating a conversation-owned external URL artifact. */
export type CreateConversationExternalArtifactInput = {
  /** Product-defined artifact kind label. */
  artifactKind: string;
  /** Owning conversation id. */
  conversationId: string;
  /** Optional artifact description. */
  description?: string | null | undefined;
  /** Durable external HTTP(S) URL for the initial version. */
  externalUrl: string;
  /** Optional media type for the external payload. */
  mediaType?: string | null | undefined;
  /** Human-readable artifact title. */
  title: string;
};

/** Add and verify an MCP server. */
export type CreateMcpServerInput = {
  /** Human-visible server name. */
  displayName: string;
  /** HTTP transport config, when `transport_kind` is `streamable_http`. */
  http?: McpHttpConfigInput | null | undefined;
  /** Stdio transport config, when `transport_kind` is `stdio`. */
  stdio?: McpStdioConfigInput | null | undefined;
  /** MCP transport kind: `stdio` or `streamable_http`. */
  transportKind: string;
};

/** Input for creating a provider account. */
export type CreateProviderAccountInput = {
  displayName?: string | null | undefined;
  providerKind: string;
  secret: string;
};

/** Input for hard-deleting a user-managed provider account. */
export type DeleteProviderAccountInput = {
  providerAccountId: string;
};

/** Local service status shown by clients. */
export type LocalServiceStatus =
  /** The local Noema service is running. */
  | 'RUNNING';

/** HTTP MCP setup config. */
export type McpHttpConfigInput = {
  /** Non-secret request headers. */
  headers?: unknown;
  /** OAuth client-secret credentials, when supported by the server. */
  oauthClientCredentials?: McpOAuthClientCredentialsInput | null | undefined;
  /** Secret request headers stored on disk. */
  secretHeaders?: unknown;
  /** MCP endpoint URL. */
  url: string;
};

/** OAuth client-secret credentials for MCP setup. */
export type McpOAuthClientCredentialsInput = {
  /** OAuth client id. */
  clientId: string;
  /** OAuth client secret stored on disk. */
  clientSecret: string;
  /** OAuth scopes to request. */
  scopes: Array<string>;
};

/** Stdio MCP setup config. */
export type McpStdioConfigInput = {
  /** Command arguments. */
  args: Array<string>;
  /** Command to launch. */
  command: string;
  /** Optional working directory. */
  cwd?: string | null | undefined;
  /** Non-secret environment variables. */
  env?: unknown;
  /** Secret environment variables stored on disk. */
  secretEnv?: unknown;
};

/** Memory service operating mode. */
export type MemoryServiceMode =
  /** Noema connects to an externally managed memory service. */
  | 'EXTERNAL'
  /** Noema manages a local memory service. */
  | 'MANAGED';

/** Memory service readiness status kind. */
export type MemoryServiceStatusKind =
  /** Service authentication failed. */
  | 'AUTH_ERROR'
  /** Service configuration has not been checked. */
  | 'NOT_CONFIGURED'
  /** Service is reachable and ready. */
  | 'READY'
  /** Managed service startup is in progress. */
  | 'STARTING'
  /** Service is not reachable. */
  | 'UNAVAILABLE';

/** Memory storage readiness shown by clients. */
export type MemoryStorageStatus =
  /** The local memory service is initializing. */
  | 'INITIALIZING'
  /** The local memory service is ready. */
  | 'READY'
  /** Memory service writes and retrieval are not available yet. */
  | 'UNAVAILABLE';

/** Multiple-choice selection mode exposed through GraphQL. */
export type MultipleChoiceSelectionMode =
  /** One or more options may be selected. */
  | 'PICK_MANY'
  /** One option may be selected. */
  | 'PICK_ONE';

/** Onboarding step status exposed through GraphQL. */
export type OnboardingStepStatus =
  /** Step blocks the user from continuing. */
  | 'BLOCKED'
  /** Step is complete. */
  | 'COMPLETE';

/** Provider account status exposed through GraphQL. */
export type ProviderAccountStatus =
  /** Account is authenticated. */
  | 'AUTHENTICATED'
  /** Status is being checked. */
  | 'CHECKING'
  /** Account is unauthenticated. */
  | 'UNAUTHENTICATED'
  /** Provider is unavailable. */
  | 'UNAVAILABLE'
  /** Status has not been checked. */
  | 'UNKNOWN';

/** Provider auth attempt status. */
export type ProviderAuthAttemptStatus =
  /** Attempt was cancelled. */
  | 'CANCELLED'
  /** Attempt completed. */
  | 'COMPLETED'
  /** Attempt expired. */
  | 'EXPIRED'
  /** Attempt failed. */
  | 'FAILED'
  /** Attempt is starting. */
  | 'STARTING'
  /** Waiting for the user. */
  | 'WAITING_FOR_USER';

/** Provider auth method exposed through GraphQL. */
export type ProviderAuthMethod =
  /** External manual flow. */
  | 'EXTERNAL_MANUAL'
  /** No auth required. */
  | 'NONE'
  /** OAuth device-code flow. */
  | 'OAUTH_DEVICE_CODE'
  /** Secret input flow. */
  | 'SECRET_INPUT';

/** Input for saving a write-only provider secret. */
export type ProviderSecretInput = {
  providerAccountId: string;
  secret: string;
};

export type ReasoningEffort =
  | 'HIGH'
  | 'LOW'
  | 'MEDIUM'
  | 'MINIMAL'
  | 'NONE'
  | 'XHIGH';

/** Input for saving an agent model preference. */
export type SaveAgentModelPreferenceInput = {
  /** Agent to update. */
  agentId: string;
  /** Provider-specific model id or profile id. */
  modelProfile: string;
  /** Provider account id to use. */
  providerAccountId: string;
  /** Optional explicit reasoning effort for reasoning-capable model profiles. */
  reasoningEffort?: ReasoningEffort | null | undefined;
};

/** Input for saving memory service settings. */
export type SaveMemoryServiceSettingsInput = {
  /** External memory service base URL. */
  baseUrl?: string | null | undefined;
  /** Memory service mode. */
  mode: MemoryServiceMode;
  /** Provider-specific model id or profile id. */
  modelProfile?: string | null | undefined;
  /** External service port, when configured. */
  port?: number | null | undefined;
  /** Provider account id to use for memory extraction. */
  providerAccountId?: string | null | undefined;
  /** Optional explicit reasoning effort for reasoning-capable model profiles. */
  reasoningEffort?: ReasoningEffort | null | undefined;
};

/** Save reviewed MCP tool calibration. */
export type SaveToolCalibrationInput = {
  /** Durable calibration id. */
  calibrationId: string;
  /** Effective export classification. */
  exportClassification: string;
  /** Calibrated MCP tool id. */
  mcpToolId: string;
  /** Effective read classification. */
  readClassification: string;
  /** Actor who reviewed the calibration, when reviewed. */
  reviewedBy?: string | null | undefined;
  /** Tool metadata fingerprint reviewed by the actor. */
  reviewedMetadataFingerprint?: string | null | undefined;
  /** Review/gateway readiness status. */
  status: string;
  /** Effective write classification. */
  writeClassification: string;
};

/** Input for saving the progress audit preference. */
export type SaveToolProgressAuditPreferenceInput = {
  /** Provider-specific model id or profile id. */
  modelProfile: string;
  /** Provider account id to use. */
  providerAccountId: string;
  /** Optional explicit reasoning effort for reasoning-capable model profiles. */
  reasoningEffort?: ReasoningEffort | null | undefined;
};

/** Input for saving the web fetch summarizer preference. */
export type SaveWebFetchSummarizerPreferenceInput = {
  /** Provider-specific model id or profile id. */
  modelProfile: string;
  /** Provider account id to use. */
  providerAccountId: string;
  /** Optional explicit reasoning effort for reasoning-capable model profiles. */
  reasoningEffort?: ReasoningEffort | null | undefined;
};

export type SaveWebToolProviderBindingInput = {
  capabilityId: string;
  providerAccountId: string;
  toolName: string;
};

/** Input for sending a conversation turn. */
export type SendConversationTurnInput = {
  /** Frontend-generated id for optimistic UI correlation. */
  clientMessageId?: string | null | undefined;
  /** Durable Noema conversation id. */
  conversationId: string;
  /** User input. */
  input: string;
};

/** Input for sending a multiple-choice selection. */
export type SendMultipleChoiceSelectionInput = {
  /** Frontend-generated id for optimistic UI correlation. */
  clientMessageId?: string | null | undefined;
  /** Durable Noema conversation id. */
  conversationId: string;
  /** Durable multiple-choice prompt item id. */
  promptItemId: string;
  /** Selected prompt option ids. */
  selectedOptionIds: Array<string>;
};

/** Start a browser OAuth setup attempt for a hosted MCP server. */
export type StartMcpServerOAuthSetupInput = {
  /** Absolute local callback URI owned by Noema web. */
  redirectUri: string;
  /** Pending MCP server setup input. */
  server: CreateMcpServerInput;
};

/** Start a browser OAuth reauthentication attempt for an existing MCP server. */
export type StartMcpServerReauthenticationOAuthSetupInput = {
  /** Durable MCP server id. */
  mcpServerId: string;
  /** Absolute local callback URI owned by Noema web. */
  redirectUri: string;
};

/** Input for starting a provider auth attempt. */
export type StartProviderAuthAttemptInput = {
  /** Requested authentication method. */
  method: ProviderAuthMethod;
  /** Stable provider account id. */
  providerAccountId: string;
  /** Provider family, such as `codex`. */
  providerKind: string;
};

/**
 * Complexity tier used by the primary agent when selecting an executor pool
 * entry.
 */
export type TaskComplexity =
  /** Large or reasoning-intensive work. */
  | 'DIFFICULT'
  /** Typical multi-step work. */
  | 'MEDIUM'
  /** Small, low-risk work. */
  | 'SIMPLE';

/** Durable task subscription event category. */
export type TaskEventKind =
  /** One task-run transcript item was inserted or updated. */
  | 'RUN_ITEM_UPSERTED'
  /** One durable task run changed. */
  | 'RUN_UPDATED'
  /** Task lifecycle/read-model state changed. */
  | 'TASK_UPDATED';

/** Input for replacing the global Task Executor safety limits. */
export type TaskExecutionPolicyInput = {
  /** Maximum active execution time in minutes, excluding queue time. */
  maxActiveMinutes: number;
  /** Maximum provider continuations before terminal-only finalization. */
  maxProviderContinuations: number;
  /** Maximum tool calls before terminal-only finalization. */
  maxToolCalls: number;
  /** Continuation interval between progress audits. */
  progressAuditInterval: number;
};

/** Input for creating or replacing one human-controlled executor pool entry. */
export type TaskModelPoolEntryInput = {
  /** Complexity tier exposed to the primary agent. */
  complexity: TaskComplexity;
  /** Whether this entry can be selected for new tasks. */
  enabled: boolean;
  /** Optional human-facing label. */
  label?: string | null | undefined;
  /** Exact provider model/profile. */
  modelProfile: string;
  /** Provider account owning the model profile. */
  providerAccountId: string;
  /** Provider family for this entry. */
  providerKind: string;
  /** Optional reasoning effort. */
  reasoningEffort?: ReasoningEffort | null | undefined;
  /** Human-controlled ordering within its tier. */
  sortOrder: number;
};

/** Turn activity status exposed through GraphQL. */
export type TurnActivityStatus =
  /** Activity completed. */
  | 'COMPLETED'
  /** Activity failed. */
  | 'FAILED'
  /** Activity started. */
  | 'STARTED';

export type LocalStatusQueryVariables = Exact<{ [key: string]: never; }>;


export type LocalStatusQuery = { localStatus: { localService: LocalServiceStatus, assistantConnection: AssistantConnection, memoryStorage: MemoryStorageStatus, primaryAgentDisplayName: string | null } };

export type OnboardingStatusQueryVariables = Exact<{ [key: string]: never; }>;


export type OnboardingStatusQuery = { onboardingStatus: { isUserOnboarded: boolean, steps: Array<{ id: string, status: OnboardingStepStatus, providerKind: string | null, providerAccountId: string | null, accountKey: string | null, displayName: string | null, providerAccountStatus: ProviderAccountStatus | null, authMethod: ProviderAuthMethod | null }> } };

export type ProviderAccountsQueryVariables = Exact<{ [key: string]: never; }>;


export type ProviderAccountsQuery = { providerAccountCatalog: Array<{ providerKind: string, displayName: string, authMethod: string, capabilities: Array<{ capabilityId: string, status: string, reliabilityContract: string, dataFlowClass: string }> }>, providerAccounts: Array<{ providerAccountId: string, providerKind: string, accountKey: string, displayName: string, authMethod: string, status: ProviderAccountStatus, isActive: boolean, isDefault: boolean, lastCheckedAt: string | null, lastAuthenticatedAt: string | null, lastErrorCode: string | null, lastErrorMessage: string | null }> };

export type CreateProviderAccountMutationVariables = Exact<{
  input: CreateProviderAccountInput;
}>;


export type CreateProviderAccountMutation = { createProviderAccount: { providerAccountId: string, providerKind: string, accountKey: string, displayName: string, authMethod: string, status: ProviderAccountStatus, isActive: boolean, isDefault: boolean, lastCheckedAt: string | null, lastAuthenticatedAt: string | null, lastErrorCode: string | null, lastErrorMessage: string | null } };

export type SaveProviderSecretInputMutationVariables = Exact<{
  input: ProviderSecretInput;
}>;


export type SaveProviderSecretInputMutation = { saveProviderSecretInput: { providerAccountId: string, providerKind: string, accountKey: string, displayName: string, authMethod: string, status: ProviderAccountStatus, isActive: boolean, isDefault: boolean, lastCheckedAt: string | null, lastAuthenticatedAt: string | null, lastErrorCode: string | null, lastErrorMessage: string | null } };

export type ClearProviderSecretMutationVariables = Exact<{
  input: ClearProviderSecretInput;
}>;


export type ClearProviderSecretMutation = { clearProviderSecret: { providerAccountId: string, providerKind: string, accountKey: string, displayName: string, authMethod: string, status: ProviderAccountStatus, isActive: boolean, isDefault: boolean, lastCheckedAt: string | null, lastAuthenticatedAt: string | null, lastErrorCode: string | null, lastErrorMessage: string | null } };

export type DeleteProviderAccountMutationVariables = Exact<{
  input: DeleteProviderAccountInput;
}>;


export type DeleteProviderAccountMutation = { deleteProviderAccount: boolean };

export type AgentsQueryVariables = Exact<{ [key: string]: never; }>;


export type AgentsQuery = { agents: Array<{ agentId: string, displayName: string | null, isPrimary: boolean, modelPreference: { providerKind: string, providerAccountId: string, modelProfile: string, reasoningEffort: ReasoningEffort | null } | null, modelOptions: Array<{ providerKind: string, providerAccountId: string, providerDisplayName: string, status: ProviderAccountStatus, disabledReason: string | null, defaultModelProfile: string | null, profiles: Array<{ id: string, label: string, disabledReason: string | null, reasoningEfforts: Array<ReasoningEffort>, defaultReasoningEffort: ReasoningEffort | null }> }> }> };

export type SaveAgentModelPreferenceMutationVariables = Exact<{
  input: SaveAgentModelPreferenceInput;
}>;


export type SaveAgentModelPreferenceMutation = { saveAgentModelPreference: { providerKind: string, providerAccountId: string, modelProfile: string, reasoningEffort: ReasoningEffort | null } };

export type WebFetchSettingsQueryVariables = Exact<{ [key: string]: never; }>;


export type WebFetchSettingsQuery = { webFetchSettings: { summarizer: { defaultModelProfile: string, modelPreference: { providerKind: string, providerAccountId: string, modelProfile: string, reasoningEffort: ReasoningEffort | null } | null, modelOptions: Array<{ providerKind: string, providerAccountId: string, providerDisplayName: string, status: ProviderAccountStatus, disabledReason: string | null, defaultModelProfile: string | null, profiles: Array<{ id: string, label: string, disabledReason: string | null, reasoningEfforts: Array<ReasoningEffort>, defaultReasoningEffort: ReasoningEffort | null }> }> } } };

export type SaveWebFetchSummarizerPreferenceMutationVariables = Exact<{
  input: SaveWebFetchSummarizerPreferenceInput;
}>;


export type SaveWebFetchSummarizerPreferenceMutation = { saveWebFetchSummarizerPreference: { providerKind: string, providerAccountId: string, modelProfile: string, reasoningEffort: ReasoningEffort | null } };

export type WebToolSettingsQueryVariables = Exact<{ [key: string]: never; }>;


export type WebToolSettingsQuery = { webToolSettings: { search: { toolName: string, capabilityId: string, activeProviderAccountId: string, providerOptions: Array<{ providerAccountId: string, providerKind: string, accountKey: string, displayName: string, capabilityId: string, reliabilityContract: string, dataFlowClass: string, citations: boolean, directUrlFetch: boolean }> }, fetch: { toolName: string, capabilityId: string, activeProviderAccountId: string, providerOptions: Array<{ providerAccountId: string, providerKind: string, accountKey: string, displayName: string, capabilityId: string, reliabilityContract: string, dataFlowClass: string, citations: boolean, directUrlFetch: boolean }> } } };

export type SaveWebToolProviderBindingMutationVariables = Exact<{
  input: SaveWebToolProviderBindingInput;
}>;


export type SaveWebToolProviderBindingMutation = { saveWebToolProviderBinding: { toolName: string, capabilityId: string, activeProviderAccountId: string, providerOptions: Array<{ providerAccountId: string, providerKind: string, accountKey: string, displayName: string, capabilityId: string, reliabilityContract: string, dataFlowClass: string, citations: boolean, directUrlFetch: boolean }> } };

export type UsageSettingsQueryVariables = Exact<{ [key: string]: never; }>;


export type UsageSettingsQuery = { usageSettings: { progressAudit: { defaultModelProfile: string, modelPreference: { providerKind: string, providerAccountId: string, modelProfile: string, reasoningEffort: ReasoningEffort | null } | null, modelOptions: Array<{ providerKind: string, providerAccountId: string, providerDisplayName: string, status: ProviderAccountStatus, disabledReason: string | null, defaultModelProfile: string | null, profiles: Array<{ id: string, label: string, disabledReason: string | null, reasoningEfforts: Array<ReasoningEffort>, defaultReasoningEffort: ReasoningEffort | null }> }> } } };

export type SaveToolProgressAuditPreferenceMutationVariables = Exact<{
  input: SaveToolProgressAuditPreferenceInput;
}>;


export type SaveToolProgressAuditPreferenceMutation = { saveToolProgressAuditPreference: { providerKind: string, providerAccountId: string, modelProfile: string, reasoningEffort: ReasoningEffort | null } };

export type MemorySettingsQueryVariables = Exact<{ [key: string]: never; }>;


export type MemorySettingsQuery = { memorySettings: { mode: MemoryServiceMode, baseUrl: string | null, port: number | null, status: { status: MemoryServiceStatusKind, checkedAt: string | null, lastErrorCode: string | null, lastErrorMessage: string | null }, modelPreference: { providerKind: string, providerAccountId: string, modelProfile: string, reasoningEffort: ReasoningEffort | null } | null, modelOptions: Array<{ providerKind: string, providerAccountId: string, providerDisplayName: string, status: ProviderAccountStatus, disabledReason: string | null, defaultModelProfile: string | null, profiles: Array<{ id: string, label: string, disabledReason: string | null, reasoningEfforts: Array<ReasoningEffort>, defaultReasoningEffort: ReasoningEffort | null }> }> } };

export type MemoryGraphQueryVariables = Exact<{
  page?: number | null | undefined;
  limit?: number | null | undefined;
}>;


export type MemoryGraphQuery = { memoryGraph: { status: { status: MemoryServiceStatusKind, lastErrorCode: string | null, lastErrorMessage: string | null }, article: { title: string, subtitle: string, markdown: string, isGenerated: boolean, generatedAt: string | null }, documents: Array<{ id: string, title: string | null, summary: string | null, type: string | null, createdAt: string, updatedAt: string, memoryEntries: Array<{ id: string, documentId: string, content: string | null, summary: string | null, title: string | null, type: string | null, metadata: unknown, createdAt: string, updatedAt: string, spaceContainerTag: string | null, relation: string | null, parentMemoryId: string | null, rootMemoryId: string | null, memoryRelations: unknown, isLatest: boolean | null, spaceId: string | null, source: { kind: string | null, conversationId: string | null, turnId: string | null, itemId: string | null, messageText: string | null } | null }> }>, pageInfo: { page: number, limit: number, hasMore: boolean, total: number | null } } };

export type RegenerateMemoryArticleMutationVariables = Exact<{ [key: string]: never; }>;


export type RegenerateMemoryArticleMutation = { regenerateMemoryArticle: { title: string, subtitle: string, markdown: string, isGenerated: boolean, generatedAt: string | null } };

export type SaveMemoryServiceSettingsMutationVariables = Exact<{
  input: SaveMemoryServiceSettingsInput;
}>;


export type SaveMemoryServiceSettingsMutation = { saveMemoryServiceSettings: { mode: MemoryServiceMode, baseUrl: string | null, port: number | null, status: { status: MemoryServiceStatusKind, checkedAt: string | null, lastErrorCode: string | null, lastErrorMessage: string | null } } };

export type CheckMemoryServiceMutationVariables = Exact<{ [key: string]: never; }>;


export type CheckMemoryServiceMutation = { checkMemoryService: { status: MemoryServiceStatusKind, checkedAt: string | null, lastErrorCode: string | null, lastErrorMessage: string | null } };

export type McpSettingsQueryVariables = Exact<{ [key: string]: never; }>;


export type McpSettingsQuery = { mcpServers: Array<{ mcpServerId: string, displayName: string, transportKind: string, enabled: boolean, healthStatus: string, authStatus: string, toolCount: number, browserOauthReauthenticationSupported: boolean }> };

export type McpToolsQueryVariables = Exact<{
  mcpServerId: string;
}>;


export type McpToolsQuery = { mcpTools: Array<{ mcpToolId: string, mcpServerId: string, name: string, description: string | null, inputSchema: unknown, outputSchema: unknown, annotations: unknown, metadataFingerprint: string, calibration: { calibrationId: string, mcpToolId: string, status: string, readClassification: string, writeClassification: string, exportClassification: string, reviewedBy: string | null, reviewedMetadataFingerprint: string | null } | null }> };

export type SaveToolCalibrationsMutationVariables = Exact<{
  inputs: Array<SaveToolCalibrationInput> | SaveToolCalibrationInput;
}>;


export type SaveToolCalibrationsMutation = { saveToolCalibrations: Array<{ calibrationId: string, mcpToolId: string, status: string, readClassification: string, writeClassification: string, exportClassification: string }> };

export type AutofillToolCalibrationsMutationVariables = Exact<{
  mcpServerId: string;
}>;


export type AutofillToolCalibrationsMutation = { autofillToolCalibrations: { suggestions: Array<{ mcpToolId: string, readClassification: string, writeClassification: string, exportClassification: string, disabled: boolean | null }> } };

export type McpServerSetupResultFieldsFragment = { setupStatus: string, discoveryStatus: string | null, discoveredToolCount: number, setupError: string | null, auth: { oauthClientCredentialsSupported: boolean, oauthAuthorizationSupported: boolean, scopes: Array<string> } | null, server: { mcpServerId: string, displayName: string, transportKind: string, enabled: boolean, healthStatus: string, authStatus: string, toolCount: number, browserOauthReauthenticationSupported: boolean } | null };

export type McpOAuthSetupAttemptFieldsFragment = { attemptId: string, status: string, authorizationUrl: string | null, errorMessage: string | null, setupResult: { setupStatus: string, discoveryStatus: string | null, discoveredToolCount: number, setupError: string | null, auth: { oauthClientCredentialsSupported: boolean, oauthAuthorizationSupported: boolean, scopes: Array<string> } | null, server: { mcpServerId: string, displayName: string, transportKind: string, enabled: boolean, healthStatus: string, authStatus: string, toolCount: number, browserOauthReauthenticationSupported: boolean } | null } | null };

export type CreateMcpServerMutationVariables = Exact<{
  input: CreateMcpServerInput;
}>;


export type CreateMcpServerMutation = { createMcpServer: { setupStatus: string, discoveryStatus: string | null, discoveredToolCount: number, setupError: string | null, auth: { oauthClientCredentialsSupported: boolean, oauthAuthorizationSupported: boolean, scopes: Array<string> } | null, server: { mcpServerId: string, displayName: string, transportKind: string, enabled: boolean, healthStatus: string, authStatus: string, toolCount: number, browserOauthReauthenticationSupported: boolean } | null } };

export type ContinueMcpServerSetupMutationVariables = Exact<{
  input: ContinueMcpServerSetupInput;
}>;


export type ContinueMcpServerSetupMutation = { continueMcpServerSetup: { setupStatus: string, discoveryStatus: string | null, discoveredToolCount: number, setupError: string | null, auth: { oauthClientCredentialsSupported: boolean, oauthAuthorizationSupported: boolean, scopes: Array<string> } | null, server: { mcpServerId: string, displayName: string, transportKind: string, enabled: boolean, healthStatus: string, authStatus: string, toolCount: number, browserOauthReauthenticationSupported: boolean } | null } };

export type StartMcpServerOauthSetupMutationVariables = Exact<{
  input: StartMcpServerOAuthSetupInput;
}>;


export type StartMcpServerOauthSetupMutation = { startMcpServerOauthSetup: { attemptId: string, status: string, authorizationUrl: string | null, errorMessage: string | null, setupResult: { setupStatus: string, discoveryStatus: string | null, discoveredToolCount: number, setupError: string | null, auth: { oauthClientCredentialsSupported: boolean, oauthAuthorizationSupported: boolean, scopes: Array<string> } | null, server: { mcpServerId: string, displayName: string, transportKind: string, enabled: boolean, healthStatus: string, authStatus: string, toolCount: number, browserOauthReauthenticationSupported: boolean } | null } | null } };

export type StartMcpServerReauthenticationOauthSetupMutationVariables = Exact<{
  input: StartMcpServerReauthenticationOAuthSetupInput;
}>;


export type StartMcpServerReauthenticationOauthSetupMutation = { startMcpServerReauthenticationOauthSetup: { attemptId: string, status: string, authorizationUrl: string | null, errorMessage: string | null, setupResult: { setupStatus: string, discoveryStatus: string | null, discoveredToolCount: number, setupError: string | null, auth: { oauthClientCredentialsSupported: boolean, oauthAuthorizationSupported: boolean, scopes: Array<string> } | null, server: { mcpServerId: string, displayName: string, transportKind: string, enabled: boolean, healthStatus: string, authStatus: string, toolCount: number, browserOauthReauthenticationSupported: boolean } | null } | null } };

export type McpOauthSetupAttemptQueryVariables = Exact<{
  attemptId: string;
}>;


export type McpOauthSetupAttemptQuery = { mcpOauthSetupAttempt: { attemptId: string, status: string, authorizationUrl: string | null, errorMessage: string | null, setupResult: { setupStatus: string, discoveryStatus: string | null, discoveredToolCount: number, setupError: string | null, auth: { oauthClientCredentialsSupported: boolean, oauthAuthorizationSupported: boolean, scopes: Array<string> } | null, server: { mcpServerId: string, displayName: string, transportKind: string, enabled: boolean, healthStatus: string, authStatus: string, toolCount: number, browserOauthReauthenticationSupported: boolean } | null } | null } | null };

export type DeleteMcpServerMutationVariables = Exact<{
  mcpServerId: string;
}>;


export type DeleteMcpServerMutation = { deleteMcpServer: boolean };

export type ProviderAuthAttemptQueryVariables = Exact<{
  attemptId: string;
}>;


export type ProviderAuthAttemptQuery = { providerAuthAttempt: { attemptId: string, providerKind: string, providerAccountId: string, method: ProviderAuthMethod, status: ProviderAuthAttemptStatus, verificationUrl: string | null, userCode: string | null, instructions: string | null, errorCode: string | null, errorMessage: string | null } | null };

export type StartProviderAuthAttemptMutationVariables = Exact<{
  input: StartProviderAuthAttemptInput;
}>;


export type StartProviderAuthAttemptMutation = { startProviderAuthAttempt: { attemptId: string, providerKind: string, providerAccountId: string, method: ProviderAuthMethod, status: ProviderAuthAttemptStatus, verificationUrl: string | null, userCode: string | null, instructions: string | null, errorCode: string | null, errorMessage: string | null } };

export type ConversationItemFieldsFragment = { itemId: string, cursor: string, turnId: string | null, metadata: unknown, item:
    | { __typename: 'A2UiCard', id: string, schema: string, payload: unknown }
    | { __typename: 'Activity', id: string, activityKind: string, status: TurnActivityStatus, title: string, summary: string | null, metadata: unknown }
    | { __typename: 'ArtifactReference', artifactId: string, artifactVersionId: string | null, title: string, artifactKind: string, storageKind: string, externalUrl: string | null, downloadUrl: string | null, mediaType: string | null }
    | { __typename: 'AssistantText', text: string }
    | { __typename: 'ErrorNotice', message: string, recoverable: boolean }
    | { __typename: 'MultipleChoicePrompt', prompt: string, selectionMode: MultipleChoiceSelectionMode, options: Array<{ id: string, label: string }> }
    | { __typename: 'MultipleChoiceSelection', promptItemId: string, selectionMode: MultipleChoiceSelectionMode, selectedOptions: Array<{ id: string, label: string }> }
    | { __typename: 'TaskReference', taskId: string, title: string, revision: number, taskStatus: string }
    | { __typename: 'UserText', text: string }
   };

export type ConversationTranscriptPageFieldsFragment = { items: Array<{ itemId: string, cursor: string, turnId: string | null, metadata: unknown, item:
      | { __typename: 'A2UiCard', id: string, schema: string, payload: unknown }
      | { __typename: 'Activity', id: string, activityKind: string, status: TurnActivityStatus, title: string, summary: string | null, metadata: unknown }
      | { __typename: 'ArtifactReference', artifactId: string, artifactVersionId: string | null, title: string, artifactKind: string, storageKind: string, externalUrl: string | null, downloadUrl: string | null, mediaType: string | null }
      | { __typename: 'AssistantText', text: string }
      | { __typename: 'ErrorNotice', message: string, recoverable: boolean }
      | { __typename: 'MultipleChoicePrompt', prompt: string, selectionMode: MultipleChoiceSelectionMode, options: Array<{ id: string, label: string }> }
      | { __typename: 'MultipleChoiceSelection', promptItemId: string, selectionMode: MultipleChoiceSelectionMode, selectedOptions: Array<{ id: string, label: string }> }
      | { __typename: 'TaskReference', taskId: string, title: string, revision: number, taskStatus: string }
      | { __typename: 'UserText', text: string }
     }>, pageInfo: { beforeCursor: string | null, hasMoreBefore: boolean, limit: number } };

export type ChatBootQueryVariables = Exact<{
  transcriptLimit?: number | null | undefined;
}>;


export type ChatBootQuery = { localStatus: { localService: LocalServiceStatus, assistantConnection: AssistantConnection, memoryStorage: MemoryStorageStatus, primaryAgentDisplayName: string | null }, onboardingStatus: { isUserOnboarded: boolean, steps: Array<{ id: string, status: OnboardingStepStatus, providerKind: string | null, providerAccountId: string | null, accountKey: string | null, displayName: string | null, providerAccountStatus: ProviderAccountStatus | null, authMethod: ProviderAuthMethod | null }> }, primaryConversation: { conversationId: string, provider: string, latestTranscriptPage: { items: Array<{ itemId: string, cursor: string, turnId: string | null, metadata: unknown, item:
          | { __typename: 'A2UiCard', id: string, schema: string, payload: unknown }
          | { __typename: 'Activity', id: string, activityKind: string, status: TurnActivityStatus, title: string, summary: string | null, metadata: unknown }
          | { __typename: 'ArtifactReference', artifactId: string, artifactVersionId: string | null, title: string, artifactKind: string, storageKind: string, externalUrl: string | null, downloadUrl: string | null, mediaType: string | null }
          | { __typename: 'AssistantText', text: string }
          | { __typename: 'ErrorNotice', message: string, recoverable: boolean }
          | { __typename: 'MultipleChoicePrompt', prompt: string, selectionMode: MultipleChoiceSelectionMode, options: Array<{ id: string, label: string }> }
          | { __typename: 'MultipleChoiceSelection', promptItemId: string, selectionMode: MultipleChoiceSelectionMode, selectedOptions: Array<{ id: string, label: string }> }
          | { __typename: 'TaskReference', taskId: string, title: string, revision: number, taskStatus: string }
          | { __typename: 'UserText', text: string }
         }>, pageInfo: { beforeCursor: string | null, hasMoreBefore: boolean, limit: number } } } | null };

export type PrimaryConversationQueryVariables = Exact<{ [key: string]: never; }>;


export type PrimaryConversationQuery = { primaryConversation: { conversationId: string, provider: string } | null };

export type EnsurePrimaryConversationMutationVariables = Exact<{ [key: string]: never; }>;


export type EnsurePrimaryConversationMutation = { ensurePrimaryConversation: { conversationId: string, provider: string } };

export type ConversationTranscriptPageQueryVariables = Exact<{
  input: ConversationTranscriptPageInput;
}>;


export type ConversationTranscriptPageQuery = { conversationTranscriptPage: { items: Array<{ itemId: string, cursor: string, turnId: string | null, metadata: unknown, item:
        | { __typename: 'A2UiCard', id: string, schema: string, payload: unknown }
        | { __typename: 'Activity', id: string, activityKind: string, status: TurnActivityStatus, title: string, summary: string | null, metadata: unknown }
        | { __typename: 'ArtifactReference', artifactId: string, artifactVersionId: string | null, title: string, artifactKind: string, storageKind: string, externalUrl: string | null, downloadUrl: string | null, mediaType: string | null }
        | { __typename: 'AssistantText', text: string }
        | { __typename: 'ErrorNotice', message: string, recoverable: boolean }
        | { __typename: 'MultipleChoicePrompt', prompt: string, selectionMode: MultipleChoiceSelectionMode, options: Array<{ id: string, label: string }> }
        | { __typename: 'MultipleChoiceSelection', promptItemId: string, selectionMode: MultipleChoiceSelectionMode, selectedOptions: Array<{ id: string, label: string }> }
        | { __typename: 'TaskReference', taskId: string, title: string, revision: number, taskStatus: string }
        | { __typename: 'UserText', text: string }
       }>, pageInfo: { beforeCursor: string | null, hasMoreBefore: boolean, limit: number } } };

export type SendConversationTurnMutationVariables = Exact<{
  input: SendConversationTurnInput;
}>;


export type SendConversationTurnMutation = { sendConversationTurn: { conversationId: string, clientMessageId: string | null } };

export type SendMultipleChoiceSelectionMutationVariables = Exact<{
  input: SendMultipleChoiceSelectionInput;
}>;


export type SendMultipleChoiceSelectionMutation = { sendMultipleChoiceSelection: { conversationId: string, clientMessageId: string | null } };

export type ConversationEventsSubscriptionVariables = Exact<{
  conversationId: string;
}>;


export type ConversationEventsSubscription = { conversationEvents:
    | { __typename: 'AgentStatusEvent', conversationId: string, status: AgentStatus }
    | { __typename: 'AssistantTextDeltaEvent', conversationId: string, streamId: string, responseIndex: number, delta: string, deltaTurnId: string }
    | { __typename: 'ConversationItemEvent', conversationId: string, clientMessageId: string | null, itemId: string, cursor: string | null, turnId: string | null, metadata: unknown, item:
        | { __typename: 'A2UiCard', id: string, schema: string, payload: unknown }
        | { __typename: 'Activity', id: string, activityKind: string, status: TurnActivityStatus, title: string, summary: string | null, metadata: unknown }
        | { __typename: 'ArtifactReference', artifactId: string, artifactVersionId: string | null, title: string, artifactKind: string, storageKind: string, externalUrl: string | null, downloadUrl: string | null, mediaType: string | null }
        | { __typename: 'AssistantText', text: string }
        | { __typename: 'ErrorNotice', message: string, recoverable: boolean }
        | { __typename: 'MultipleChoicePrompt', prompt: string, selectionMode: MultipleChoiceSelectionMode, options: Array<{ id: string, label: string }> }
        | { __typename: 'MultipleChoiceSelection', promptItemId: string, selectionMode: MultipleChoiceSelectionMode, selectedOptions: Array<{ id: string, label: string }> }
        | { __typename: 'TaskReference', taskId: string, title: string, revision: number, taskStatus: string }
        | { __typename: 'UserText', text: string }
       }
    | { __typename: 'SubscriptionReadyEvent' }
    | { __typename: 'TurnCompletedEvent', conversationId: string, clientMessageId: string | null }
   };

export type ArtifactsQueryVariables = Exact<{
  ownerObjectType: string;
  ownerObjectId: string;
  limit?: number | null | undefined;
}>;


export type ArtifactsQuery = { artifacts: Array<{ artifactId: string, ownerObjectType: string, ownerObjectId: string, title: string, description: string | null, artifactKind: string, storageKind: ArtifactStorageKind, currentVersion: { artifactVersionId: string, versionIndex: number, externalUrl: string | null, downloadUrl: string | null, mediaType: string | null } }> };

export type ArtifactVersionDetailQueryVariables = Exact<{
  artifactVersionId: string;
}>;


export type ArtifactVersionDetailQuery = { artifactVersionDetail: { artifactVersionId: string, artifactId: string, versionIndex: number, title: string, artifactKind: string, storageKind: ArtifactStorageKind, mediaType: string | null, previewKind: ArtifactVersionPreviewKind, markdown: string | null, plainText: string | null, downloadUrl: string | null, externalUrl: string | null, versions: Array<{ artifactVersionId: string, versionIndex: number, downloadUrl: string | null, externalUrl: string | null, mediaType: string | null }> } | null };

export type TaskDetailQueryVariables = Exact<{
  taskId: string;
}>;


export type TaskDetailQuery = { task: { taskId: string, title: string, requestMarkdown: string, complexity: TaskComplexity, status: string, ownerHumanId: string, createdByAgentId: string, creationToolCallId: string | null, poolEntryId: string, revisionIndex: number, maxReviewRounds: number, finalSubmissionId: string | null, latestRunId: string | null, terminalReason: string | null, errorCode: string | null, errorMessage: string | null, createdAt: string, updatedAt: string, completedAt: string | null, resumable: boolean, cancellable: boolean, blockingQuestion: string | null, source: { conversationId: string | null, turnId: string | null, itemId: string | null }, executorModel: { providerKind: string, providerAccountId: string, selectionMode: string, modelProfile: string | null, reasoningEffort: ReasoningEffort | null, selectionSource: string | null }, reviewerModel: { providerKind: string, providerAccountId: string, selectionMode: string, modelProfile: string | null, reasoningEffort: ReasoningEffort | null, selectionSource: string | null }, criteria: Array<{ criterionId: string, ordinal: number, description: string, expectedEvidence: string | null }>, submissions: Array<{ submissionId: string, executorRunId: string, revisionIndex: number, summary: string, resultMarkdown: string, createdAt: string, criteria: Array<{ criterionId: string, evidenceMarkdown: string }>, artifacts: Array<{ artifactId: string, artifactVersionId: string, title: string, artifactKind: string, storageKind: string, mediaType: string | null, downloadUrl: string | null, externalUrl: string | null }> }>, reviews: Array<{ reviewId: string, reviewerRunId: string, reviewedSubmissionId: string, overallVerdict: string, overallFeedback: string, createdAt: string, criteria: Array<{ criterionId: string, outcome: string, evidenceMarkdown: string | null, feedback: string | null }> }>, runs: Array<{ runId: string, runKind: string, agentId: string, revisionIndex: number, attemptIndex: number, status: string, actualProviderKind: string | null, actualModelProfile: string | null, triggeringSubmissionId: string | null, triggeringReviewId: string | null, errorCode: string | null, errorMessage: string | null, providerCallCount: number, toolCallCount: number, cachedInputTokens: number, activeMilliseconds: number, inputTokens: number, outputTokens: number, queuedAt: string, startedAt: string | null, endedAt: string | null, createdAt: string, updatedAt: string, model: { providerKind: string, providerAccountId: string, selectionMode: string, modelProfile: string | null, reasoningEffort: ReasoningEffort | null, selectionSource: string | null }, executionPolicy: { maxProviderContinuations: number, maxToolCalls: number, maxActiveMinutes: number, progressAuditInterval: number } }> } | null };

export type TaskReferenceStatusQueryVariables = Exact<{
  taskId: string;
}>;


export type TaskReferenceStatusQuery = { task: { taskId: string, title: string, status: string, revisionIndex: number } | null };

export type ResumeTaskMutationVariables = Exact<{
  taskId: string;
  message?: string | null | undefined;
}>;


export type ResumeTaskMutation = { resumeTask: { taskId: string, status: string, latestRunId: string | null, errorCode: string | null, errorMessage: string | null, updatedAt: string } };

export type CancelTaskMutationVariables = Exact<{
  taskId: string;
}>;


export type CancelTaskMutation = { cancelTask: { taskId: string, status: string, latestRunId: string | null, errorCode: string | null, errorMessage: string | null, updatedAt: string } };

export type TaskRunItemsQueryVariables = Exact<{
  runId: string;
  after?: string | null | undefined;
  first?: number | null | undefined;
}>;


export type TaskRunItemsQuery = { taskRunItems: { items: Array<{ itemId: string, runId: string, sequenceIndex: number, roundIndex: number, kind: string, status: string, correlationId: string | null, parentItemId: string | null, contentText: string | null, payload: unknown, createdAt: string, updatedAt: string }>, pageInfo: { endCursor: string | null, hasNextPage: boolean } } };

export type TaskEventsSubscriptionVariables = Exact<{
  taskId: string;
  after?: string | null | undefined;
}>;


export type TaskEventsSubscription = { taskEvents: { cursor: string, kind: TaskEventKind, taskId: string, runId: string | null, status: string | null, createdAt: string, item: { itemId: string, runId: string, sequenceIndex: number, roundIndex: number, kind: string, status: string, correlationId: string | null, parentItemId: string | null, contentText: string | null, payload: unknown, createdAt: string, updatedAt: string } | null, run: { runId: string, runKind: string, agentId: string, revisionIndex: number, attemptIndex: number, status: string, actualProviderKind: string | null, actualModelProfile: string | null, triggeringSubmissionId: string | null, triggeringReviewId: string | null, errorCode: string | null, errorMessage: string | null, providerCallCount: number, toolCallCount: number, cachedInputTokens: number, activeMilliseconds: number, inputTokens: number, outputTokens: number, queuedAt: string, startedAt: string | null, endedAt: string | null, createdAt: string, updatedAt: string, model: { providerKind: string, providerAccountId: string, selectionMode: string, modelProfile: string | null, reasoningEffort: ReasoningEffort | null, selectionSource: string | null }, executionPolicy: { maxProviderContinuations: number, maxToolCalls: number, maxActiveMinutes: number, progressAuditInterval: number } } | null } };

export type TaskExecutionPolicyQueryVariables = Exact<{ [key: string]: never; }>;


export type TaskExecutionPolicyQuery = { taskExecutionPolicy: { maxProviderContinuations: number, maxToolCalls: number, maxActiveMinutes: number, progressAuditInterval: number } };

export type UpdateTaskExecutionPolicyMutationVariables = Exact<{
  input: TaskExecutionPolicyInput;
}>;


export type UpdateTaskExecutionPolicyMutation = { updateTaskExecutionPolicy: { maxProviderContinuations: number, maxToolCalls: number, maxActiveMinutes: number, progressAuditInterval: number } };

export type TaskModelPoolsQueryVariables = Exact<{ [key: string]: never; }>;


export type TaskModelPoolsQuery = { taskModelPools: Array<{ poolEntryId: string, complexity: TaskComplexity, label: string | null, providerKind: string, providerAccountId: string, modelProfile: string, reasoningEffort: ReasoningEffort | null, enabled: boolean, sortOrder: number, createdAt: string, updatedAt: string }> };

export type UpdateTaskModelPoolEntryMutationVariables = Exact<{
  poolEntryId: string;
  input: TaskModelPoolEntryInput;
}>;


export type UpdateTaskModelPoolEntryMutation = { updateTaskModelPoolEntry: { poolEntryId: string, complexity: TaskComplexity, label: string | null, providerKind: string, providerAccountId: string, modelProfile: string, reasoningEffort: ReasoningEffort | null, enabled: boolean, sortOrder: number, createdAt: string, updatedAt: string } };

export type CreateConversationExternalArtifactMutationVariables = Exact<{
  input: CreateConversationExternalArtifactInput;
}>;


export type CreateConversationExternalArtifactMutation = { createConversationExternalArtifact: { artifactId: string, ownerObjectType: string, ownerObjectId: string, title: string, storageKind: ArtifactStorageKind, currentVersion: { versionIndex: number, externalUrl: string | null, downloadUrl: string | null } } };

export const McpServerSetupResultFieldsFragmentDoc = {"kind":"Document","definitions":[{"kind":"FragmentDefinition","name":{"kind":"Name","value":"McpServerSetupResultFields"},"typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"McpServerSetupResult"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"setupStatus"}},{"kind":"Field","name":{"kind":"Name","value":"discoveryStatus"}},{"kind":"Field","name":{"kind":"Name","value":"discoveredToolCount"}},{"kind":"Field","name":{"kind":"Name","value":"setupError"}},{"kind":"Field","name":{"kind":"Name","value":"auth"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"oauthClientCredentialsSupported"}},{"kind":"Field","name":{"kind":"Name","value":"oauthAuthorizationSupported"}},{"kind":"Field","name":{"kind":"Name","value":"scopes"}}]}},{"kind":"Field","name":{"kind":"Name","value":"server"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"mcpServerId"}},{"kind":"Field","name":{"kind":"Name","value":"displayName"}},{"kind":"Field","name":{"kind":"Name","value":"transportKind"}},{"kind":"Field","name":{"kind":"Name","value":"enabled"}},{"kind":"Field","name":{"kind":"Name","value":"healthStatus"}},{"kind":"Field","name":{"kind":"Name","value":"authStatus"}},{"kind":"Field","name":{"kind":"Name","value":"toolCount"}},{"kind":"Field","name":{"kind":"Name","value":"browserOauthReauthenticationSupported"}}]}}]}}]} as unknown as DocumentNode<McpServerSetupResultFieldsFragment, unknown>;
export const McpOAuthSetupAttemptFieldsFragmentDoc = {"kind":"Document","definitions":[{"kind":"FragmentDefinition","name":{"kind":"Name","value":"McpOAuthSetupAttemptFields"},"typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"McpOAuthSetupAttempt"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"attemptId"}},{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"authorizationUrl"}},{"kind":"Field","name":{"kind":"Name","value":"errorMessage"}},{"kind":"Field","name":{"kind":"Name","value":"setupResult"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"FragmentSpread","name":{"kind":"Name","value":"McpServerSetupResultFields"}}]}}]}},{"kind":"FragmentDefinition","name":{"kind":"Name","value":"McpServerSetupResultFields"},"typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"McpServerSetupResult"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"setupStatus"}},{"kind":"Field","name":{"kind":"Name","value":"discoveryStatus"}},{"kind":"Field","name":{"kind":"Name","value":"discoveredToolCount"}},{"kind":"Field","name":{"kind":"Name","value":"setupError"}},{"kind":"Field","name":{"kind":"Name","value":"auth"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"oauthClientCredentialsSupported"}},{"kind":"Field","name":{"kind":"Name","value":"oauthAuthorizationSupported"}},{"kind":"Field","name":{"kind":"Name","value":"scopes"}}]}},{"kind":"Field","name":{"kind":"Name","value":"server"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"mcpServerId"}},{"kind":"Field","name":{"kind":"Name","value":"displayName"}},{"kind":"Field","name":{"kind":"Name","value":"transportKind"}},{"kind":"Field","name":{"kind":"Name","value":"enabled"}},{"kind":"Field","name":{"kind":"Name","value":"healthStatus"}},{"kind":"Field","name":{"kind":"Name","value":"authStatus"}},{"kind":"Field","name":{"kind":"Name","value":"toolCount"}},{"kind":"Field","name":{"kind":"Name","value":"browserOauthReauthenticationSupported"}}]}}]}}]} as unknown as DocumentNode<McpOAuthSetupAttemptFieldsFragment, unknown>;
export const ConversationItemFieldsFragmentDoc = {"kind":"Document","definitions":[{"kind":"FragmentDefinition","name":{"kind":"Name","value":"ConversationItemFields"},"typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"ConversationItem"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"itemId"}},{"kind":"Field","name":{"kind":"Name","value":"cursor"}},{"kind":"Field","name":{"kind":"Name","value":"turnId"}},{"kind":"Field","name":{"kind":"Name","value":"metadata"}},{"kind":"Field","name":{"kind":"Name","value":"item"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"__typename"}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"UserText"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"text"}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"AssistantText"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"text"}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"Activity"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"activityKind"}},{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"title"}},{"kind":"Field","name":{"kind":"Name","value":"summary"}},{"kind":"Field","name":{"kind":"Name","value":"metadata"}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"A2UiCard"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"schema"}},{"kind":"Field","name":{"kind":"Name","value":"payload"}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"MultipleChoicePrompt"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"prompt"}},{"kind":"Field","name":{"kind":"Name","value":"selectionMode"}},{"kind":"Field","name":{"kind":"Name","value":"options"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"label"}}]}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"MultipleChoiceSelection"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"promptItemId"}},{"kind":"Field","name":{"kind":"Name","value":"selectionMode"}},{"kind":"Field","name":{"kind":"Name","value":"selectedOptions"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"label"}}]}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"ErrorNotice"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"message"}},{"kind":"Field","name":{"kind":"Name","value":"recoverable"}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"ArtifactReference"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"artifactId"}},{"kind":"Field","name":{"kind":"Name","value":"artifactVersionId"}},{"kind":"Field","name":{"kind":"Name","value":"title"}},{"kind":"Field","name":{"kind":"Name","value":"artifactKind"}},{"kind":"Field","name":{"kind":"Name","value":"storageKind"}},{"kind":"Field","name":{"kind":"Name","value":"externalUrl"}},{"kind":"Field","name":{"kind":"Name","value":"downloadUrl"}},{"kind":"Field","name":{"kind":"Name","value":"mediaType"}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"TaskReference"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"taskId"}},{"kind":"Field","name":{"kind":"Name","value":"title"}},{"kind":"Field","alias":{"kind":"Name","value":"taskStatus"},"name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"revision"}}]}}]}}]}}]} as unknown as DocumentNode<ConversationItemFieldsFragment, unknown>;
export const ConversationTranscriptPageFieldsFragmentDoc = {"kind":"Document","definitions":[{"kind":"FragmentDefinition","name":{"kind":"Name","value":"ConversationTranscriptPageFields"},"typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"ConversationTranscriptPage"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"items"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"FragmentSpread","name":{"kind":"Name","value":"ConversationItemFields"}}]}},{"kind":"Field","name":{"kind":"Name","value":"pageInfo"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"beforeCursor"}},{"kind":"Field","name":{"kind":"Name","value":"hasMoreBefore"}},{"kind":"Field","name":{"kind":"Name","value":"limit"}}]}}]}},{"kind":"FragmentDefinition","name":{"kind":"Name","value":"ConversationItemFields"},"typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"ConversationItem"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"itemId"}},{"kind":"Field","name":{"kind":"Name","value":"cursor"}},{"kind":"Field","name":{"kind":"Name","value":"turnId"}},{"kind":"Field","name":{"kind":"Name","value":"metadata"}},{"kind":"Field","name":{"kind":"Name","value":"item"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"__typename"}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"UserText"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"text"}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"AssistantText"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"text"}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"Activity"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"activityKind"}},{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"title"}},{"kind":"Field","name":{"kind":"Name","value":"summary"}},{"kind":"Field","name":{"kind":"Name","value":"metadata"}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"A2UiCard"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"schema"}},{"kind":"Field","name":{"kind":"Name","value":"payload"}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"MultipleChoicePrompt"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"prompt"}},{"kind":"Field","name":{"kind":"Name","value":"selectionMode"}},{"kind":"Field","name":{"kind":"Name","value":"options"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"label"}}]}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"MultipleChoiceSelection"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"promptItemId"}},{"kind":"Field","name":{"kind":"Name","value":"selectionMode"}},{"kind":"Field","name":{"kind":"Name","value":"selectedOptions"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"label"}}]}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"ErrorNotice"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"message"}},{"kind":"Field","name":{"kind":"Name","value":"recoverable"}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"ArtifactReference"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"artifactId"}},{"kind":"Field","name":{"kind":"Name","value":"artifactVersionId"}},{"kind":"Field","name":{"kind":"Name","value":"title"}},{"kind":"Field","name":{"kind":"Name","value":"artifactKind"}},{"kind":"Field","name":{"kind":"Name","value":"storageKind"}},{"kind":"Field","name":{"kind":"Name","value":"externalUrl"}},{"kind":"Field","name":{"kind":"Name","value":"downloadUrl"}},{"kind":"Field","name":{"kind":"Name","value":"mediaType"}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"TaskReference"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"taskId"}},{"kind":"Field","name":{"kind":"Name","value":"title"}},{"kind":"Field","alias":{"kind":"Name","value":"taskStatus"},"name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"revision"}}]}}]}}]}}]} as unknown as DocumentNode<ConversationTranscriptPageFieldsFragment, unknown>;
export const LocalStatusDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"query","name":{"kind":"Name","value":"LocalStatus"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"localStatus"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"localService"}},{"kind":"Field","name":{"kind":"Name","value":"assistantConnection"}},{"kind":"Field","name":{"kind":"Name","value":"memoryStorage"}},{"kind":"Field","name":{"kind":"Name","value":"primaryAgentDisplayName"}}]}}]}}]} as unknown as DocumentNode<LocalStatusQuery, LocalStatusQueryVariables>;
export const OnboardingStatusDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"query","name":{"kind":"Name","value":"OnboardingStatus"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"onboardingStatus"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"isUserOnboarded"}},{"kind":"Field","name":{"kind":"Name","value":"steps"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"providerKind"}},{"kind":"Field","name":{"kind":"Name","value":"providerAccountId"}},{"kind":"Field","name":{"kind":"Name","value":"accountKey"}},{"kind":"Field","name":{"kind":"Name","value":"displayName"}},{"kind":"Field","name":{"kind":"Name","value":"providerAccountStatus"}},{"kind":"Field","name":{"kind":"Name","value":"authMethod"}}]}}]}}]}}]} as unknown as DocumentNode<OnboardingStatusQuery, OnboardingStatusQueryVariables>;
export const ProviderAccountsDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"query","name":{"kind":"Name","value":"ProviderAccounts"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"providerAccountCatalog"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"providerKind"}},{"kind":"Field","name":{"kind":"Name","value":"displayName"}},{"kind":"Field","name":{"kind":"Name","value":"authMethod"}},{"kind":"Field","name":{"kind":"Name","value":"capabilities"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"capabilityId"}},{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"reliabilityContract"}},{"kind":"Field","name":{"kind":"Name","value":"dataFlowClass"}}]}}]}},{"kind":"Field","name":{"kind":"Name","value":"providerAccounts"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"providerAccountId"}},{"kind":"Field","name":{"kind":"Name","value":"providerKind"}},{"kind":"Field","name":{"kind":"Name","value":"accountKey"}},{"kind":"Field","name":{"kind":"Name","value":"displayName"}},{"kind":"Field","name":{"kind":"Name","value":"authMethod"}},{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"isActive"}},{"kind":"Field","name":{"kind":"Name","value":"isDefault"}},{"kind":"Field","name":{"kind":"Name","value":"lastCheckedAt"}},{"kind":"Field","name":{"kind":"Name","value":"lastAuthenticatedAt"}},{"kind":"Field","name":{"kind":"Name","value":"lastErrorCode"}},{"kind":"Field","name":{"kind":"Name","value":"lastErrorMessage"}}]}}]}}]} as unknown as DocumentNode<ProviderAccountsQuery, ProviderAccountsQueryVariables>;
export const CreateProviderAccountDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"CreateProviderAccount"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"input"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"CreateProviderAccountInput"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"createProviderAccount"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"input"},"value":{"kind":"Variable","name":{"kind":"Name","value":"input"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"providerAccountId"}},{"kind":"Field","name":{"kind":"Name","value":"providerKind"}},{"kind":"Field","name":{"kind":"Name","value":"accountKey"}},{"kind":"Field","name":{"kind":"Name","value":"displayName"}},{"kind":"Field","name":{"kind":"Name","value":"authMethod"}},{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"isActive"}},{"kind":"Field","name":{"kind":"Name","value":"isDefault"}},{"kind":"Field","name":{"kind":"Name","value":"lastCheckedAt"}},{"kind":"Field","name":{"kind":"Name","value":"lastAuthenticatedAt"}},{"kind":"Field","name":{"kind":"Name","value":"lastErrorCode"}},{"kind":"Field","name":{"kind":"Name","value":"lastErrorMessage"}}]}}]}}]} as unknown as DocumentNode<CreateProviderAccountMutation, CreateProviderAccountMutationVariables>;
export const SaveProviderSecretInputDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"SaveProviderSecretInput"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"input"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"ProviderSecretInput"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"saveProviderSecretInput"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"input"},"value":{"kind":"Variable","name":{"kind":"Name","value":"input"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"providerAccountId"}},{"kind":"Field","name":{"kind":"Name","value":"providerKind"}},{"kind":"Field","name":{"kind":"Name","value":"accountKey"}},{"kind":"Field","name":{"kind":"Name","value":"displayName"}},{"kind":"Field","name":{"kind":"Name","value":"authMethod"}},{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"isActive"}},{"kind":"Field","name":{"kind":"Name","value":"isDefault"}},{"kind":"Field","name":{"kind":"Name","value":"lastCheckedAt"}},{"kind":"Field","name":{"kind":"Name","value":"lastAuthenticatedAt"}},{"kind":"Field","name":{"kind":"Name","value":"lastErrorCode"}},{"kind":"Field","name":{"kind":"Name","value":"lastErrorMessage"}}]}}]}}]} as unknown as DocumentNode<SaveProviderSecretInputMutation, SaveProviderSecretInputMutationVariables>;
export const ClearProviderSecretDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"ClearProviderSecret"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"input"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"ClearProviderSecretInput"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"clearProviderSecret"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"input"},"value":{"kind":"Variable","name":{"kind":"Name","value":"input"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"providerAccountId"}},{"kind":"Field","name":{"kind":"Name","value":"providerKind"}},{"kind":"Field","name":{"kind":"Name","value":"accountKey"}},{"kind":"Field","name":{"kind":"Name","value":"displayName"}},{"kind":"Field","name":{"kind":"Name","value":"authMethod"}},{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"isActive"}},{"kind":"Field","name":{"kind":"Name","value":"isDefault"}},{"kind":"Field","name":{"kind":"Name","value":"lastCheckedAt"}},{"kind":"Field","name":{"kind":"Name","value":"lastAuthenticatedAt"}},{"kind":"Field","name":{"kind":"Name","value":"lastErrorCode"}},{"kind":"Field","name":{"kind":"Name","value":"lastErrorMessage"}}]}}]}}]} as unknown as DocumentNode<ClearProviderSecretMutation, ClearProviderSecretMutationVariables>;
export const DeleteProviderAccountDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"DeleteProviderAccount"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"input"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"DeleteProviderAccountInput"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"deleteProviderAccount"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"input"},"value":{"kind":"Variable","name":{"kind":"Name","value":"input"}}}]}]}}]} as unknown as DocumentNode<DeleteProviderAccountMutation, DeleteProviderAccountMutationVariables>;
export const AgentsDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"query","name":{"kind":"Name","value":"Agents"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"agents"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"agentId"}},{"kind":"Field","name":{"kind":"Name","value":"displayName"}},{"kind":"Field","name":{"kind":"Name","value":"isPrimary"}},{"kind":"Field","name":{"kind":"Name","value":"modelPreference"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"providerKind"}},{"kind":"Field","name":{"kind":"Name","value":"providerAccountId"}},{"kind":"Field","name":{"kind":"Name","value":"modelProfile"}},{"kind":"Field","name":{"kind":"Name","value":"reasoningEffort"}}]}},{"kind":"Field","name":{"kind":"Name","value":"modelOptions"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"providerKind"}},{"kind":"Field","name":{"kind":"Name","value":"providerAccountId"}},{"kind":"Field","name":{"kind":"Name","value":"providerDisplayName"}},{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"disabledReason"}},{"kind":"Field","name":{"kind":"Name","value":"defaultModelProfile"}},{"kind":"Field","name":{"kind":"Name","value":"profiles"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"label"}},{"kind":"Field","name":{"kind":"Name","value":"disabledReason"}},{"kind":"Field","name":{"kind":"Name","value":"reasoningEfforts"}},{"kind":"Field","name":{"kind":"Name","value":"defaultReasoningEffort"}}]}}]}}]}}]}}]} as unknown as DocumentNode<AgentsQuery, AgentsQueryVariables>;
export const SaveAgentModelPreferenceDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"SaveAgentModelPreference"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"input"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"SaveAgentModelPreferenceInput"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"saveAgentModelPreference"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"input"},"value":{"kind":"Variable","name":{"kind":"Name","value":"input"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"providerKind"}},{"kind":"Field","name":{"kind":"Name","value":"providerAccountId"}},{"kind":"Field","name":{"kind":"Name","value":"modelProfile"}},{"kind":"Field","name":{"kind":"Name","value":"reasoningEffort"}}]}}]}}]} as unknown as DocumentNode<SaveAgentModelPreferenceMutation, SaveAgentModelPreferenceMutationVariables>;
export const WebFetchSettingsDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"query","name":{"kind":"Name","value":"WebFetchSettings"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"webFetchSettings"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"summarizer"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"defaultModelProfile"}},{"kind":"Field","name":{"kind":"Name","value":"modelPreference"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"providerKind"}},{"kind":"Field","name":{"kind":"Name","value":"providerAccountId"}},{"kind":"Field","name":{"kind":"Name","value":"modelProfile"}},{"kind":"Field","name":{"kind":"Name","value":"reasoningEffort"}}]}},{"kind":"Field","name":{"kind":"Name","value":"modelOptions"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"providerKind"}},{"kind":"Field","name":{"kind":"Name","value":"providerAccountId"}},{"kind":"Field","name":{"kind":"Name","value":"providerDisplayName"}},{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"disabledReason"}},{"kind":"Field","name":{"kind":"Name","value":"defaultModelProfile"}},{"kind":"Field","name":{"kind":"Name","value":"profiles"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"label"}},{"kind":"Field","name":{"kind":"Name","value":"disabledReason"}},{"kind":"Field","name":{"kind":"Name","value":"reasoningEfforts"}},{"kind":"Field","name":{"kind":"Name","value":"defaultReasoningEffort"}}]}}]}}]}}]}}]}}]} as unknown as DocumentNode<WebFetchSettingsQuery, WebFetchSettingsQueryVariables>;
export const SaveWebFetchSummarizerPreferenceDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"SaveWebFetchSummarizerPreference"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"input"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"SaveWebFetchSummarizerPreferenceInput"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"saveWebFetchSummarizerPreference"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"input"},"value":{"kind":"Variable","name":{"kind":"Name","value":"input"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"providerKind"}},{"kind":"Field","name":{"kind":"Name","value":"providerAccountId"}},{"kind":"Field","name":{"kind":"Name","value":"modelProfile"}},{"kind":"Field","name":{"kind":"Name","value":"reasoningEffort"}}]}}]}}]} as unknown as DocumentNode<SaveWebFetchSummarizerPreferenceMutation, SaveWebFetchSummarizerPreferenceMutationVariables>;
export const WebToolSettingsDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"query","name":{"kind":"Name","value":"WebToolSettings"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"webToolSettings"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"search"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"toolName"}},{"kind":"Field","name":{"kind":"Name","value":"capabilityId"}},{"kind":"Field","name":{"kind":"Name","value":"activeProviderAccountId"}},{"kind":"Field","name":{"kind":"Name","value":"providerOptions"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"providerAccountId"}},{"kind":"Field","name":{"kind":"Name","value":"providerKind"}},{"kind":"Field","name":{"kind":"Name","value":"accountKey"}},{"kind":"Field","name":{"kind":"Name","value":"displayName"}},{"kind":"Field","name":{"kind":"Name","value":"capabilityId"}},{"kind":"Field","name":{"kind":"Name","value":"reliabilityContract"}},{"kind":"Field","name":{"kind":"Name","value":"dataFlowClass"}},{"kind":"Field","name":{"kind":"Name","value":"citations"}},{"kind":"Field","name":{"kind":"Name","value":"directUrlFetch"}}]}}]}},{"kind":"Field","name":{"kind":"Name","value":"fetch"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"toolName"}},{"kind":"Field","name":{"kind":"Name","value":"capabilityId"}},{"kind":"Field","name":{"kind":"Name","value":"activeProviderAccountId"}},{"kind":"Field","name":{"kind":"Name","value":"providerOptions"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"providerAccountId"}},{"kind":"Field","name":{"kind":"Name","value":"providerKind"}},{"kind":"Field","name":{"kind":"Name","value":"accountKey"}},{"kind":"Field","name":{"kind":"Name","value":"displayName"}},{"kind":"Field","name":{"kind":"Name","value":"capabilityId"}},{"kind":"Field","name":{"kind":"Name","value":"reliabilityContract"}},{"kind":"Field","name":{"kind":"Name","value":"dataFlowClass"}},{"kind":"Field","name":{"kind":"Name","value":"citations"}},{"kind":"Field","name":{"kind":"Name","value":"directUrlFetch"}}]}}]}}]}}]}}]} as unknown as DocumentNode<WebToolSettingsQuery, WebToolSettingsQueryVariables>;
export const SaveWebToolProviderBindingDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"SaveWebToolProviderBinding"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"input"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"SaveWebToolProviderBindingInput"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"saveWebToolProviderBinding"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"input"},"value":{"kind":"Variable","name":{"kind":"Name","value":"input"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"toolName"}},{"kind":"Field","name":{"kind":"Name","value":"capabilityId"}},{"kind":"Field","name":{"kind":"Name","value":"activeProviderAccountId"}},{"kind":"Field","name":{"kind":"Name","value":"providerOptions"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"providerAccountId"}},{"kind":"Field","name":{"kind":"Name","value":"providerKind"}},{"kind":"Field","name":{"kind":"Name","value":"accountKey"}},{"kind":"Field","name":{"kind":"Name","value":"displayName"}},{"kind":"Field","name":{"kind":"Name","value":"capabilityId"}},{"kind":"Field","name":{"kind":"Name","value":"reliabilityContract"}},{"kind":"Field","name":{"kind":"Name","value":"dataFlowClass"}},{"kind":"Field","name":{"kind":"Name","value":"citations"}},{"kind":"Field","name":{"kind":"Name","value":"directUrlFetch"}}]}}]}}]}}]} as unknown as DocumentNode<SaveWebToolProviderBindingMutation, SaveWebToolProviderBindingMutationVariables>;
export const UsageSettingsDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"query","name":{"kind":"Name","value":"UsageSettings"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"usageSettings"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"progressAudit"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"defaultModelProfile"}},{"kind":"Field","name":{"kind":"Name","value":"modelPreference"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"providerKind"}},{"kind":"Field","name":{"kind":"Name","value":"providerAccountId"}},{"kind":"Field","name":{"kind":"Name","value":"modelProfile"}},{"kind":"Field","name":{"kind":"Name","value":"reasoningEffort"}}]}},{"kind":"Field","name":{"kind":"Name","value":"modelOptions"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"providerKind"}},{"kind":"Field","name":{"kind":"Name","value":"providerAccountId"}},{"kind":"Field","name":{"kind":"Name","value":"providerDisplayName"}},{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"disabledReason"}},{"kind":"Field","name":{"kind":"Name","value":"defaultModelProfile"}},{"kind":"Field","name":{"kind":"Name","value":"profiles"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"label"}},{"kind":"Field","name":{"kind":"Name","value":"disabledReason"}},{"kind":"Field","name":{"kind":"Name","value":"reasoningEfforts"}},{"kind":"Field","name":{"kind":"Name","value":"defaultReasoningEffort"}}]}}]}}]}}]}}]}}]} as unknown as DocumentNode<UsageSettingsQuery, UsageSettingsQueryVariables>;
export const SaveToolProgressAuditPreferenceDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"SaveToolProgressAuditPreference"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"input"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"SaveToolProgressAuditPreferenceInput"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"saveToolProgressAuditPreference"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"input"},"value":{"kind":"Variable","name":{"kind":"Name","value":"input"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"providerKind"}},{"kind":"Field","name":{"kind":"Name","value":"providerAccountId"}},{"kind":"Field","name":{"kind":"Name","value":"modelProfile"}},{"kind":"Field","name":{"kind":"Name","value":"reasoningEffort"}}]}}]}}]} as unknown as DocumentNode<SaveToolProgressAuditPreferenceMutation, SaveToolProgressAuditPreferenceMutationVariables>;
export const MemorySettingsDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"query","name":{"kind":"Name","value":"MemorySettings"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"memorySettings"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"mode"}},{"kind":"Field","name":{"kind":"Name","value":"baseUrl"}},{"kind":"Field","name":{"kind":"Name","value":"port"}},{"kind":"Field","name":{"kind":"Name","value":"status"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"checkedAt"}},{"kind":"Field","name":{"kind":"Name","value":"lastErrorCode"}},{"kind":"Field","name":{"kind":"Name","value":"lastErrorMessage"}}]}},{"kind":"Field","name":{"kind":"Name","value":"modelPreference"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"providerKind"}},{"kind":"Field","name":{"kind":"Name","value":"providerAccountId"}},{"kind":"Field","name":{"kind":"Name","value":"modelProfile"}},{"kind":"Field","name":{"kind":"Name","value":"reasoningEffort"}}]}},{"kind":"Field","name":{"kind":"Name","value":"modelOptions"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"providerKind"}},{"kind":"Field","name":{"kind":"Name","value":"providerAccountId"}},{"kind":"Field","name":{"kind":"Name","value":"providerDisplayName"}},{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"disabledReason"}},{"kind":"Field","name":{"kind":"Name","value":"defaultModelProfile"}},{"kind":"Field","name":{"kind":"Name","value":"profiles"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"label"}},{"kind":"Field","name":{"kind":"Name","value":"disabledReason"}},{"kind":"Field","name":{"kind":"Name","value":"reasoningEfforts"}},{"kind":"Field","name":{"kind":"Name","value":"defaultReasoningEffort"}}]}}]}}]}}]}}]} as unknown as DocumentNode<MemorySettingsQuery, MemorySettingsQueryVariables>;
export const MemoryGraphDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"query","name":{"kind":"Name","value":"MemoryGraph"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"page"}},"type":{"kind":"NamedType","name":{"kind":"Name","value":"Int"}}},{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"limit"}},"type":{"kind":"NamedType","name":{"kind":"Name","value":"Int"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"memoryGraph"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"input"},"value":{"kind":"ObjectValue","fields":[{"kind":"ObjectField","name":{"kind":"Name","value":"page"},"value":{"kind":"Variable","name":{"kind":"Name","value":"page"}}},{"kind":"ObjectField","name":{"kind":"Name","value":"limit"},"value":{"kind":"Variable","name":{"kind":"Name","value":"limit"}}}]}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"status"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"lastErrorCode"}},{"kind":"Field","name":{"kind":"Name","value":"lastErrorMessage"}}]}},{"kind":"Field","name":{"kind":"Name","value":"article"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"title"}},{"kind":"Field","name":{"kind":"Name","value":"subtitle"}},{"kind":"Field","name":{"kind":"Name","value":"markdown"}},{"kind":"Field","name":{"kind":"Name","value":"isGenerated"}},{"kind":"Field","name":{"kind":"Name","value":"generatedAt"}}]}},{"kind":"Field","name":{"kind":"Name","value":"documents"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"title"}},{"kind":"Field","name":{"kind":"Name","value":"summary"}},{"kind":"Field","name":{"kind":"Name","value":"type"}},{"kind":"Field","name":{"kind":"Name","value":"createdAt"}},{"kind":"Field","name":{"kind":"Name","value":"updatedAt"}},{"kind":"Field","name":{"kind":"Name","value":"memoryEntries"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"documentId"}},{"kind":"Field","name":{"kind":"Name","value":"content"}},{"kind":"Field","name":{"kind":"Name","value":"summary"}},{"kind":"Field","name":{"kind":"Name","value":"title"}},{"kind":"Field","name":{"kind":"Name","value":"type"}},{"kind":"Field","name":{"kind":"Name","value":"source"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"kind"}},{"kind":"Field","name":{"kind":"Name","value":"conversationId"}},{"kind":"Field","name":{"kind":"Name","value":"turnId"}},{"kind":"Field","name":{"kind":"Name","value":"itemId"}},{"kind":"Field","name":{"kind":"Name","value":"messageText"}}]}},{"kind":"Field","name":{"kind":"Name","value":"metadata"}},{"kind":"Field","name":{"kind":"Name","value":"createdAt"}},{"kind":"Field","name":{"kind":"Name","value":"updatedAt"}},{"kind":"Field","name":{"kind":"Name","value":"spaceContainerTag"}},{"kind":"Field","name":{"kind":"Name","value":"relation"}},{"kind":"Field","name":{"kind":"Name","value":"parentMemoryId"}},{"kind":"Field","name":{"kind":"Name","value":"rootMemoryId"}},{"kind":"Field","name":{"kind":"Name","value":"memoryRelations"}},{"kind":"Field","name":{"kind":"Name","value":"isLatest"}},{"kind":"Field","name":{"kind":"Name","value":"spaceId"}}]}}]}},{"kind":"Field","name":{"kind":"Name","value":"pageInfo"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"page"}},{"kind":"Field","name":{"kind":"Name","value":"limit"}},{"kind":"Field","name":{"kind":"Name","value":"hasMore"}},{"kind":"Field","name":{"kind":"Name","value":"total"}}]}}]}}]}}]} as unknown as DocumentNode<MemoryGraphQuery, MemoryGraphQueryVariables>;
export const RegenerateMemoryArticleDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"RegenerateMemoryArticle"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"regenerateMemoryArticle"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"title"}},{"kind":"Field","name":{"kind":"Name","value":"subtitle"}},{"kind":"Field","name":{"kind":"Name","value":"markdown"}},{"kind":"Field","name":{"kind":"Name","value":"isGenerated"}},{"kind":"Field","name":{"kind":"Name","value":"generatedAt"}}]}}]}}]} as unknown as DocumentNode<RegenerateMemoryArticleMutation, RegenerateMemoryArticleMutationVariables>;
export const SaveMemoryServiceSettingsDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"SaveMemoryServiceSettings"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"input"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"SaveMemoryServiceSettingsInput"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"saveMemoryServiceSettings"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"input"},"value":{"kind":"Variable","name":{"kind":"Name","value":"input"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"mode"}},{"kind":"Field","name":{"kind":"Name","value":"baseUrl"}},{"kind":"Field","name":{"kind":"Name","value":"port"}},{"kind":"Field","name":{"kind":"Name","value":"status"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"checkedAt"}},{"kind":"Field","name":{"kind":"Name","value":"lastErrorCode"}},{"kind":"Field","name":{"kind":"Name","value":"lastErrorMessage"}}]}}]}}]}}]} as unknown as DocumentNode<SaveMemoryServiceSettingsMutation, SaveMemoryServiceSettingsMutationVariables>;
export const CheckMemoryServiceDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"CheckMemoryService"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"checkMemoryService"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"checkedAt"}},{"kind":"Field","name":{"kind":"Name","value":"lastErrorCode"}},{"kind":"Field","name":{"kind":"Name","value":"lastErrorMessage"}}]}}]}}]} as unknown as DocumentNode<CheckMemoryServiceMutation, CheckMemoryServiceMutationVariables>;
export const McpSettingsDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"query","name":{"kind":"Name","value":"McpSettings"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"mcpServers"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"mcpServerId"}},{"kind":"Field","name":{"kind":"Name","value":"displayName"}},{"kind":"Field","name":{"kind":"Name","value":"transportKind"}},{"kind":"Field","name":{"kind":"Name","value":"enabled"}},{"kind":"Field","name":{"kind":"Name","value":"healthStatus"}},{"kind":"Field","name":{"kind":"Name","value":"authStatus"}},{"kind":"Field","name":{"kind":"Name","value":"toolCount"}},{"kind":"Field","name":{"kind":"Name","value":"browserOauthReauthenticationSupported"}}]}}]}}]} as unknown as DocumentNode<McpSettingsQuery, McpSettingsQueryVariables>;
export const McpToolsDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"query","name":{"kind":"Name","value":"McpTools"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"mcpServerId"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"String"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"mcpTools"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"mcpServerId"},"value":{"kind":"Variable","name":{"kind":"Name","value":"mcpServerId"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"mcpToolId"}},{"kind":"Field","name":{"kind":"Name","value":"mcpServerId"}},{"kind":"Field","name":{"kind":"Name","value":"name"}},{"kind":"Field","name":{"kind":"Name","value":"description"}},{"kind":"Field","name":{"kind":"Name","value":"inputSchema"}},{"kind":"Field","name":{"kind":"Name","value":"outputSchema"}},{"kind":"Field","name":{"kind":"Name","value":"annotations"}},{"kind":"Field","name":{"kind":"Name","value":"metadataFingerprint"}},{"kind":"Field","name":{"kind":"Name","value":"calibration"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"calibrationId"}},{"kind":"Field","name":{"kind":"Name","value":"mcpToolId"}},{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"readClassification"}},{"kind":"Field","name":{"kind":"Name","value":"writeClassification"}},{"kind":"Field","name":{"kind":"Name","value":"exportClassification"}},{"kind":"Field","name":{"kind":"Name","value":"reviewedBy"}},{"kind":"Field","name":{"kind":"Name","value":"reviewedMetadataFingerprint"}}]}}]}}]}}]} as unknown as DocumentNode<McpToolsQuery, McpToolsQueryVariables>;
export const SaveToolCalibrationsDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"SaveToolCalibrations"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"inputs"}},"type":{"kind":"NonNullType","type":{"kind":"ListType","type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"SaveToolCalibrationInput"}}}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"saveToolCalibrations"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"inputs"},"value":{"kind":"Variable","name":{"kind":"Name","value":"inputs"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"calibrationId"}},{"kind":"Field","name":{"kind":"Name","value":"mcpToolId"}},{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"readClassification"}},{"kind":"Field","name":{"kind":"Name","value":"writeClassification"}},{"kind":"Field","name":{"kind":"Name","value":"exportClassification"}}]}}]}}]} as unknown as DocumentNode<SaveToolCalibrationsMutation, SaveToolCalibrationsMutationVariables>;
export const AutofillToolCalibrationsDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"AutofillToolCalibrations"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"mcpServerId"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"String"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"autofillToolCalibrations"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"mcpServerId"},"value":{"kind":"Variable","name":{"kind":"Name","value":"mcpServerId"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"suggestions"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"mcpToolId"}},{"kind":"Field","name":{"kind":"Name","value":"readClassification"}},{"kind":"Field","name":{"kind":"Name","value":"writeClassification"}},{"kind":"Field","name":{"kind":"Name","value":"exportClassification"}},{"kind":"Field","name":{"kind":"Name","value":"disabled"}}]}}]}}]}}]} as unknown as DocumentNode<AutofillToolCalibrationsMutation, AutofillToolCalibrationsMutationVariables>;
export const CreateMcpServerDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"CreateMcpServer"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"input"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"CreateMcpServerInput"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"createMcpServer"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"input"},"value":{"kind":"Variable","name":{"kind":"Name","value":"input"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"FragmentSpread","name":{"kind":"Name","value":"McpServerSetupResultFields"}}]}}]}},{"kind":"FragmentDefinition","name":{"kind":"Name","value":"McpServerSetupResultFields"},"typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"McpServerSetupResult"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"setupStatus"}},{"kind":"Field","name":{"kind":"Name","value":"discoveryStatus"}},{"kind":"Field","name":{"kind":"Name","value":"discoveredToolCount"}},{"kind":"Field","name":{"kind":"Name","value":"setupError"}},{"kind":"Field","name":{"kind":"Name","value":"auth"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"oauthClientCredentialsSupported"}},{"kind":"Field","name":{"kind":"Name","value":"oauthAuthorizationSupported"}},{"kind":"Field","name":{"kind":"Name","value":"scopes"}}]}},{"kind":"Field","name":{"kind":"Name","value":"server"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"mcpServerId"}},{"kind":"Field","name":{"kind":"Name","value":"displayName"}},{"kind":"Field","name":{"kind":"Name","value":"transportKind"}},{"kind":"Field","name":{"kind":"Name","value":"enabled"}},{"kind":"Field","name":{"kind":"Name","value":"healthStatus"}},{"kind":"Field","name":{"kind":"Name","value":"authStatus"}},{"kind":"Field","name":{"kind":"Name","value":"toolCount"}},{"kind":"Field","name":{"kind":"Name","value":"browserOauthReauthenticationSupported"}}]}}]}}]} as unknown as DocumentNode<CreateMcpServerMutation, CreateMcpServerMutationVariables>;
export const ContinueMcpServerSetupDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"ContinueMcpServerSetup"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"input"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"ContinueMcpServerSetupInput"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"continueMcpServerSetup"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"input"},"value":{"kind":"Variable","name":{"kind":"Name","value":"input"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"FragmentSpread","name":{"kind":"Name","value":"McpServerSetupResultFields"}}]}}]}},{"kind":"FragmentDefinition","name":{"kind":"Name","value":"McpServerSetupResultFields"},"typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"McpServerSetupResult"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"setupStatus"}},{"kind":"Field","name":{"kind":"Name","value":"discoveryStatus"}},{"kind":"Field","name":{"kind":"Name","value":"discoveredToolCount"}},{"kind":"Field","name":{"kind":"Name","value":"setupError"}},{"kind":"Field","name":{"kind":"Name","value":"auth"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"oauthClientCredentialsSupported"}},{"kind":"Field","name":{"kind":"Name","value":"oauthAuthorizationSupported"}},{"kind":"Field","name":{"kind":"Name","value":"scopes"}}]}},{"kind":"Field","name":{"kind":"Name","value":"server"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"mcpServerId"}},{"kind":"Field","name":{"kind":"Name","value":"displayName"}},{"kind":"Field","name":{"kind":"Name","value":"transportKind"}},{"kind":"Field","name":{"kind":"Name","value":"enabled"}},{"kind":"Field","name":{"kind":"Name","value":"healthStatus"}},{"kind":"Field","name":{"kind":"Name","value":"authStatus"}},{"kind":"Field","name":{"kind":"Name","value":"toolCount"}},{"kind":"Field","name":{"kind":"Name","value":"browserOauthReauthenticationSupported"}}]}}]}}]} as unknown as DocumentNode<ContinueMcpServerSetupMutation, ContinueMcpServerSetupMutationVariables>;
export const StartMcpServerOauthSetupDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"StartMcpServerOauthSetup"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"input"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"StartMcpServerOAuthSetupInput"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"startMcpServerOauthSetup"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"input"},"value":{"kind":"Variable","name":{"kind":"Name","value":"input"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"FragmentSpread","name":{"kind":"Name","value":"McpOAuthSetupAttemptFields"}}]}}]}},{"kind":"FragmentDefinition","name":{"kind":"Name","value":"McpServerSetupResultFields"},"typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"McpServerSetupResult"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"setupStatus"}},{"kind":"Field","name":{"kind":"Name","value":"discoveryStatus"}},{"kind":"Field","name":{"kind":"Name","value":"discoveredToolCount"}},{"kind":"Field","name":{"kind":"Name","value":"setupError"}},{"kind":"Field","name":{"kind":"Name","value":"auth"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"oauthClientCredentialsSupported"}},{"kind":"Field","name":{"kind":"Name","value":"oauthAuthorizationSupported"}},{"kind":"Field","name":{"kind":"Name","value":"scopes"}}]}},{"kind":"Field","name":{"kind":"Name","value":"server"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"mcpServerId"}},{"kind":"Field","name":{"kind":"Name","value":"displayName"}},{"kind":"Field","name":{"kind":"Name","value":"transportKind"}},{"kind":"Field","name":{"kind":"Name","value":"enabled"}},{"kind":"Field","name":{"kind":"Name","value":"healthStatus"}},{"kind":"Field","name":{"kind":"Name","value":"authStatus"}},{"kind":"Field","name":{"kind":"Name","value":"toolCount"}},{"kind":"Field","name":{"kind":"Name","value":"browserOauthReauthenticationSupported"}}]}}]}},{"kind":"FragmentDefinition","name":{"kind":"Name","value":"McpOAuthSetupAttemptFields"},"typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"McpOAuthSetupAttempt"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"attemptId"}},{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"authorizationUrl"}},{"kind":"Field","name":{"kind":"Name","value":"errorMessage"}},{"kind":"Field","name":{"kind":"Name","value":"setupResult"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"FragmentSpread","name":{"kind":"Name","value":"McpServerSetupResultFields"}}]}}]}}]} as unknown as DocumentNode<StartMcpServerOauthSetupMutation, StartMcpServerOauthSetupMutationVariables>;
export const StartMcpServerReauthenticationOauthSetupDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"StartMcpServerReauthenticationOauthSetup"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"input"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"StartMcpServerReauthenticationOAuthSetupInput"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"startMcpServerReauthenticationOauthSetup"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"input"},"value":{"kind":"Variable","name":{"kind":"Name","value":"input"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"FragmentSpread","name":{"kind":"Name","value":"McpOAuthSetupAttemptFields"}}]}}]}},{"kind":"FragmentDefinition","name":{"kind":"Name","value":"McpServerSetupResultFields"},"typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"McpServerSetupResult"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"setupStatus"}},{"kind":"Field","name":{"kind":"Name","value":"discoveryStatus"}},{"kind":"Field","name":{"kind":"Name","value":"discoveredToolCount"}},{"kind":"Field","name":{"kind":"Name","value":"setupError"}},{"kind":"Field","name":{"kind":"Name","value":"auth"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"oauthClientCredentialsSupported"}},{"kind":"Field","name":{"kind":"Name","value":"oauthAuthorizationSupported"}},{"kind":"Field","name":{"kind":"Name","value":"scopes"}}]}},{"kind":"Field","name":{"kind":"Name","value":"server"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"mcpServerId"}},{"kind":"Field","name":{"kind":"Name","value":"displayName"}},{"kind":"Field","name":{"kind":"Name","value":"transportKind"}},{"kind":"Field","name":{"kind":"Name","value":"enabled"}},{"kind":"Field","name":{"kind":"Name","value":"healthStatus"}},{"kind":"Field","name":{"kind":"Name","value":"authStatus"}},{"kind":"Field","name":{"kind":"Name","value":"toolCount"}},{"kind":"Field","name":{"kind":"Name","value":"browserOauthReauthenticationSupported"}}]}}]}},{"kind":"FragmentDefinition","name":{"kind":"Name","value":"McpOAuthSetupAttemptFields"},"typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"McpOAuthSetupAttempt"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"attemptId"}},{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"authorizationUrl"}},{"kind":"Field","name":{"kind":"Name","value":"errorMessage"}},{"kind":"Field","name":{"kind":"Name","value":"setupResult"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"FragmentSpread","name":{"kind":"Name","value":"McpServerSetupResultFields"}}]}}]}}]} as unknown as DocumentNode<StartMcpServerReauthenticationOauthSetupMutation, StartMcpServerReauthenticationOauthSetupMutationVariables>;
export const McpOauthSetupAttemptDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"query","name":{"kind":"Name","value":"McpOauthSetupAttempt"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"attemptId"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"String"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"mcpOauthSetupAttempt"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"attemptId"},"value":{"kind":"Variable","name":{"kind":"Name","value":"attemptId"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"FragmentSpread","name":{"kind":"Name","value":"McpOAuthSetupAttemptFields"}}]}}]}},{"kind":"FragmentDefinition","name":{"kind":"Name","value":"McpServerSetupResultFields"},"typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"McpServerSetupResult"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"setupStatus"}},{"kind":"Field","name":{"kind":"Name","value":"discoveryStatus"}},{"kind":"Field","name":{"kind":"Name","value":"discoveredToolCount"}},{"kind":"Field","name":{"kind":"Name","value":"setupError"}},{"kind":"Field","name":{"kind":"Name","value":"auth"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"oauthClientCredentialsSupported"}},{"kind":"Field","name":{"kind":"Name","value":"oauthAuthorizationSupported"}},{"kind":"Field","name":{"kind":"Name","value":"scopes"}}]}},{"kind":"Field","name":{"kind":"Name","value":"server"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"mcpServerId"}},{"kind":"Field","name":{"kind":"Name","value":"displayName"}},{"kind":"Field","name":{"kind":"Name","value":"transportKind"}},{"kind":"Field","name":{"kind":"Name","value":"enabled"}},{"kind":"Field","name":{"kind":"Name","value":"healthStatus"}},{"kind":"Field","name":{"kind":"Name","value":"authStatus"}},{"kind":"Field","name":{"kind":"Name","value":"toolCount"}},{"kind":"Field","name":{"kind":"Name","value":"browserOauthReauthenticationSupported"}}]}}]}},{"kind":"FragmentDefinition","name":{"kind":"Name","value":"McpOAuthSetupAttemptFields"},"typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"McpOAuthSetupAttempt"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"attemptId"}},{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"authorizationUrl"}},{"kind":"Field","name":{"kind":"Name","value":"errorMessage"}},{"kind":"Field","name":{"kind":"Name","value":"setupResult"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"FragmentSpread","name":{"kind":"Name","value":"McpServerSetupResultFields"}}]}}]}}]} as unknown as DocumentNode<McpOauthSetupAttemptQuery, McpOauthSetupAttemptQueryVariables>;
export const DeleteMcpServerDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"DeleteMcpServer"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"mcpServerId"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"String"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"deleteMcpServer"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"mcpServerId"},"value":{"kind":"Variable","name":{"kind":"Name","value":"mcpServerId"}}}]}]}}]} as unknown as DocumentNode<DeleteMcpServerMutation, DeleteMcpServerMutationVariables>;
export const ProviderAuthAttemptDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"query","name":{"kind":"Name","value":"ProviderAuthAttempt"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"attemptId"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"String"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"providerAuthAttempt"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"attemptId"},"value":{"kind":"Variable","name":{"kind":"Name","value":"attemptId"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"attemptId"}},{"kind":"Field","name":{"kind":"Name","value":"providerKind"}},{"kind":"Field","name":{"kind":"Name","value":"providerAccountId"}},{"kind":"Field","name":{"kind":"Name","value":"method"}},{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"verificationUrl"}},{"kind":"Field","name":{"kind":"Name","value":"userCode"}},{"kind":"Field","name":{"kind":"Name","value":"instructions"}},{"kind":"Field","name":{"kind":"Name","value":"errorCode"}},{"kind":"Field","name":{"kind":"Name","value":"errorMessage"}}]}}]}}]} as unknown as DocumentNode<ProviderAuthAttemptQuery, ProviderAuthAttemptQueryVariables>;
export const StartProviderAuthAttemptDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"StartProviderAuthAttempt"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"input"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"StartProviderAuthAttemptInput"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"startProviderAuthAttempt"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"input"},"value":{"kind":"Variable","name":{"kind":"Name","value":"input"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"attemptId"}},{"kind":"Field","name":{"kind":"Name","value":"providerKind"}},{"kind":"Field","name":{"kind":"Name","value":"providerAccountId"}},{"kind":"Field","name":{"kind":"Name","value":"method"}},{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"verificationUrl"}},{"kind":"Field","name":{"kind":"Name","value":"userCode"}},{"kind":"Field","name":{"kind":"Name","value":"instructions"}},{"kind":"Field","name":{"kind":"Name","value":"errorCode"}},{"kind":"Field","name":{"kind":"Name","value":"errorMessage"}}]}}]}}]} as unknown as DocumentNode<StartProviderAuthAttemptMutation, StartProviderAuthAttemptMutationVariables>;
export const ChatBootDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"query","name":{"kind":"Name","value":"ChatBoot"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"transcriptLimit"}},"type":{"kind":"NamedType","name":{"kind":"Name","value":"Int"}},"defaultValue":{"kind":"IntValue","value":"80"}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"localStatus"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"localService"}},{"kind":"Field","name":{"kind":"Name","value":"assistantConnection"}},{"kind":"Field","name":{"kind":"Name","value":"memoryStorage"}},{"kind":"Field","name":{"kind":"Name","value":"primaryAgentDisplayName"}}]}},{"kind":"Field","name":{"kind":"Name","value":"onboardingStatus"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"isUserOnboarded"}},{"kind":"Field","name":{"kind":"Name","value":"steps"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"providerKind"}},{"kind":"Field","name":{"kind":"Name","value":"providerAccountId"}},{"kind":"Field","name":{"kind":"Name","value":"accountKey"}},{"kind":"Field","name":{"kind":"Name","value":"displayName"}},{"kind":"Field","name":{"kind":"Name","value":"providerAccountStatus"}},{"kind":"Field","name":{"kind":"Name","value":"authMethod"}}]}}]}},{"kind":"Field","name":{"kind":"Name","value":"primaryConversation"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"conversationId"}},{"kind":"Field","name":{"kind":"Name","value":"provider"}},{"kind":"Field","name":{"kind":"Name","value":"latestTranscriptPage"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"limit"},"value":{"kind":"Variable","name":{"kind":"Name","value":"transcriptLimit"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"FragmentSpread","name":{"kind":"Name","value":"ConversationTranscriptPageFields"}}]}}]}}]}},{"kind":"FragmentDefinition","name":{"kind":"Name","value":"ConversationItemFields"},"typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"ConversationItem"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"itemId"}},{"kind":"Field","name":{"kind":"Name","value":"cursor"}},{"kind":"Field","name":{"kind":"Name","value":"turnId"}},{"kind":"Field","name":{"kind":"Name","value":"metadata"}},{"kind":"Field","name":{"kind":"Name","value":"item"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"__typename"}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"UserText"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"text"}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"AssistantText"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"text"}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"Activity"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"activityKind"}},{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"title"}},{"kind":"Field","name":{"kind":"Name","value":"summary"}},{"kind":"Field","name":{"kind":"Name","value":"metadata"}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"A2UiCard"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"schema"}},{"kind":"Field","name":{"kind":"Name","value":"payload"}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"MultipleChoicePrompt"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"prompt"}},{"kind":"Field","name":{"kind":"Name","value":"selectionMode"}},{"kind":"Field","name":{"kind":"Name","value":"options"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"label"}}]}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"MultipleChoiceSelection"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"promptItemId"}},{"kind":"Field","name":{"kind":"Name","value":"selectionMode"}},{"kind":"Field","name":{"kind":"Name","value":"selectedOptions"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"label"}}]}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"ErrorNotice"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"message"}},{"kind":"Field","name":{"kind":"Name","value":"recoverable"}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"ArtifactReference"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"artifactId"}},{"kind":"Field","name":{"kind":"Name","value":"artifactVersionId"}},{"kind":"Field","name":{"kind":"Name","value":"title"}},{"kind":"Field","name":{"kind":"Name","value":"artifactKind"}},{"kind":"Field","name":{"kind":"Name","value":"storageKind"}},{"kind":"Field","name":{"kind":"Name","value":"externalUrl"}},{"kind":"Field","name":{"kind":"Name","value":"downloadUrl"}},{"kind":"Field","name":{"kind":"Name","value":"mediaType"}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"TaskReference"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"taskId"}},{"kind":"Field","name":{"kind":"Name","value":"title"}},{"kind":"Field","alias":{"kind":"Name","value":"taskStatus"},"name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"revision"}}]}}]}}]}},{"kind":"FragmentDefinition","name":{"kind":"Name","value":"ConversationTranscriptPageFields"},"typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"ConversationTranscriptPage"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"items"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"FragmentSpread","name":{"kind":"Name","value":"ConversationItemFields"}}]}},{"kind":"Field","name":{"kind":"Name","value":"pageInfo"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"beforeCursor"}},{"kind":"Field","name":{"kind":"Name","value":"hasMoreBefore"}},{"kind":"Field","name":{"kind":"Name","value":"limit"}}]}}]}}]} as unknown as DocumentNode<ChatBootQuery, ChatBootQueryVariables>;
export const PrimaryConversationDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"query","name":{"kind":"Name","value":"PrimaryConversation"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"primaryConversation"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"conversationId"}},{"kind":"Field","name":{"kind":"Name","value":"provider"}}]}}]}}]} as unknown as DocumentNode<PrimaryConversationQuery, PrimaryConversationQueryVariables>;
export const EnsurePrimaryConversationDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"EnsurePrimaryConversation"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"ensurePrimaryConversation"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"conversationId"}},{"kind":"Field","name":{"kind":"Name","value":"provider"}}]}}]}}]} as unknown as DocumentNode<EnsurePrimaryConversationMutation, EnsurePrimaryConversationMutationVariables>;
export const ConversationTranscriptPageDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"query","name":{"kind":"Name","value":"ConversationTranscriptPage"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"input"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"ConversationTranscriptPageInput"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"conversationTranscriptPage"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"input"},"value":{"kind":"Variable","name":{"kind":"Name","value":"input"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"FragmentSpread","name":{"kind":"Name","value":"ConversationTranscriptPageFields"}}]}}]}},{"kind":"FragmentDefinition","name":{"kind":"Name","value":"ConversationItemFields"},"typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"ConversationItem"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"itemId"}},{"kind":"Field","name":{"kind":"Name","value":"cursor"}},{"kind":"Field","name":{"kind":"Name","value":"turnId"}},{"kind":"Field","name":{"kind":"Name","value":"metadata"}},{"kind":"Field","name":{"kind":"Name","value":"item"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"__typename"}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"UserText"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"text"}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"AssistantText"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"text"}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"Activity"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"activityKind"}},{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"title"}},{"kind":"Field","name":{"kind":"Name","value":"summary"}},{"kind":"Field","name":{"kind":"Name","value":"metadata"}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"A2UiCard"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"schema"}},{"kind":"Field","name":{"kind":"Name","value":"payload"}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"MultipleChoicePrompt"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"prompt"}},{"kind":"Field","name":{"kind":"Name","value":"selectionMode"}},{"kind":"Field","name":{"kind":"Name","value":"options"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"label"}}]}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"MultipleChoiceSelection"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"promptItemId"}},{"kind":"Field","name":{"kind":"Name","value":"selectionMode"}},{"kind":"Field","name":{"kind":"Name","value":"selectedOptions"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"label"}}]}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"ErrorNotice"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"message"}},{"kind":"Field","name":{"kind":"Name","value":"recoverable"}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"ArtifactReference"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"artifactId"}},{"kind":"Field","name":{"kind":"Name","value":"artifactVersionId"}},{"kind":"Field","name":{"kind":"Name","value":"title"}},{"kind":"Field","name":{"kind":"Name","value":"artifactKind"}},{"kind":"Field","name":{"kind":"Name","value":"storageKind"}},{"kind":"Field","name":{"kind":"Name","value":"externalUrl"}},{"kind":"Field","name":{"kind":"Name","value":"downloadUrl"}},{"kind":"Field","name":{"kind":"Name","value":"mediaType"}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"TaskReference"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"taskId"}},{"kind":"Field","name":{"kind":"Name","value":"title"}},{"kind":"Field","alias":{"kind":"Name","value":"taskStatus"},"name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"revision"}}]}}]}}]}},{"kind":"FragmentDefinition","name":{"kind":"Name","value":"ConversationTranscriptPageFields"},"typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"ConversationTranscriptPage"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"items"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"FragmentSpread","name":{"kind":"Name","value":"ConversationItemFields"}}]}},{"kind":"Field","name":{"kind":"Name","value":"pageInfo"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"beforeCursor"}},{"kind":"Field","name":{"kind":"Name","value":"hasMoreBefore"}},{"kind":"Field","name":{"kind":"Name","value":"limit"}}]}}]}}]} as unknown as DocumentNode<ConversationTranscriptPageQuery, ConversationTranscriptPageQueryVariables>;
export const SendConversationTurnDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"SendConversationTurn"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"input"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"SendConversationTurnInput"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"sendConversationTurn"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"input"},"value":{"kind":"Variable","name":{"kind":"Name","value":"input"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"conversationId"}},{"kind":"Field","name":{"kind":"Name","value":"clientMessageId"}}]}}]}}]} as unknown as DocumentNode<SendConversationTurnMutation, SendConversationTurnMutationVariables>;
export const SendMultipleChoiceSelectionDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"SendMultipleChoiceSelection"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"input"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"SendMultipleChoiceSelectionInput"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"sendMultipleChoiceSelection"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"input"},"value":{"kind":"Variable","name":{"kind":"Name","value":"input"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"conversationId"}},{"kind":"Field","name":{"kind":"Name","value":"clientMessageId"}}]}}]}}]} as unknown as DocumentNode<SendMultipleChoiceSelectionMutation, SendMultipleChoiceSelectionMutationVariables>;
export const ConversationEventsDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"subscription","name":{"kind":"Name","value":"ConversationEvents"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"conversationId"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"String"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"conversationEvents"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"conversationId"},"value":{"kind":"Variable","name":{"kind":"Name","value":"conversationId"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"__typename"}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"ConversationItemEvent"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"conversationId"}},{"kind":"Field","name":{"kind":"Name","value":"clientMessageId"}},{"kind":"Field","name":{"kind":"Name","value":"itemId"}},{"kind":"Field","name":{"kind":"Name","value":"cursor"}},{"kind":"Field","name":{"kind":"Name","value":"turnId"}},{"kind":"Field","name":{"kind":"Name","value":"metadata"}},{"kind":"Field","name":{"kind":"Name","value":"item"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"__typename"}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"UserText"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"text"}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"AssistantText"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"text"}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"Activity"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"activityKind"}},{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"title"}},{"kind":"Field","name":{"kind":"Name","value":"summary"}},{"kind":"Field","name":{"kind":"Name","value":"metadata"}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"A2UiCard"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"schema"}},{"kind":"Field","name":{"kind":"Name","value":"payload"}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"MultipleChoicePrompt"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"prompt"}},{"kind":"Field","name":{"kind":"Name","value":"selectionMode"}},{"kind":"Field","name":{"kind":"Name","value":"options"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"label"}}]}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"MultipleChoiceSelection"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"promptItemId"}},{"kind":"Field","name":{"kind":"Name","value":"selectionMode"}},{"kind":"Field","name":{"kind":"Name","value":"selectedOptions"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"label"}}]}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"ErrorNotice"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"message"}},{"kind":"Field","name":{"kind":"Name","value":"recoverable"}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"ArtifactReference"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"artifactId"}},{"kind":"Field","name":{"kind":"Name","value":"artifactVersionId"}},{"kind":"Field","name":{"kind":"Name","value":"title"}},{"kind":"Field","name":{"kind":"Name","value":"artifactKind"}},{"kind":"Field","name":{"kind":"Name","value":"storageKind"}},{"kind":"Field","name":{"kind":"Name","value":"externalUrl"}},{"kind":"Field","name":{"kind":"Name","value":"downloadUrl"}},{"kind":"Field","name":{"kind":"Name","value":"mediaType"}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"TaskReference"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"taskId"}},{"kind":"Field","name":{"kind":"Name","value":"title"}},{"kind":"Field","alias":{"kind":"Name","value":"taskStatus"},"name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"revision"}}]}}]}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"AssistantTextDeltaEvent"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"conversationId"}},{"kind":"Field","alias":{"kind":"Name","value":"deltaTurnId"},"name":{"kind":"Name","value":"turnId"}},{"kind":"Field","name":{"kind":"Name","value":"streamId"}},{"kind":"Field","name":{"kind":"Name","value":"responseIndex"}},{"kind":"Field","name":{"kind":"Name","value":"delta"}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"AgentStatusEvent"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"conversationId"}},{"kind":"Field","name":{"kind":"Name","value":"status"}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"TurnCompletedEvent"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"conversationId"}},{"kind":"Field","name":{"kind":"Name","value":"clientMessageId"}}]}}]}}]}}]} as unknown as DocumentNode<ConversationEventsSubscription, ConversationEventsSubscriptionVariables>;
export const ArtifactsDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"query","name":{"kind":"Name","value":"Artifacts"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"ownerObjectType"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"String"}}}},{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"ownerObjectId"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"String"}}}},{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"limit"}},"type":{"kind":"NamedType","name":{"kind":"Name","value":"Int"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"artifacts"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"ownerObjectType"},"value":{"kind":"Variable","name":{"kind":"Name","value":"ownerObjectType"}}},{"kind":"Argument","name":{"kind":"Name","value":"ownerObjectId"},"value":{"kind":"Variable","name":{"kind":"Name","value":"ownerObjectId"}}},{"kind":"Argument","name":{"kind":"Name","value":"limit"},"value":{"kind":"Variable","name":{"kind":"Name","value":"limit"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"artifactId"}},{"kind":"Field","name":{"kind":"Name","value":"ownerObjectType"}},{"kind":"Field","name":{"kind":"Name","value":"ownerObjectId"}},{"kind":"Field","name":{"kind":"Name","value":"title"}},{"kind":"Field","name":{"kind":"Name","value":"description"}},{"kind":"Field","name":{"kind":"Name","value":"artifactKind"}},{"kind":"Field","name":{"kind":"Name","value":"storageKind"}},{"kind":"Field","name":{"kind":"Name","value":"currentVersion"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"artifactVersionId"}},{"kind":"Field","name":{"kind":"Name","value":"versionIndex"}},{"kind":"Field","name":{"kind":"Name","value":"externalUrl"}},{"kind":"Field","name":{"kind":"Name","value":"downloadUrl"}},{"kind":"Field","name":{"kind":"Name","value":"mediaType"}}]}}]}}]}}]} as unknown as DocumentNode<ArtifactsQuery, ArtifactsQueryVariables>;
export const ArtifactVersionDetailDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"query","name":{"kind":"Name","value":"ArtifactVersionDetail"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"artifactVersionId"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"String"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"artifactVersionDetail"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"artifactVersionId"},"value":{"kind":"Variable","name":{"kind":"Name","value":"artifactVersionId"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"artifactVersionId"}},{"kind":"Field","name":{"kind":"Name","value":"artifactId"}},{"kind":"Field","name":{"kind":"Name","value":"versionIndex"}},{"kind":"Field","name":{"kind":"Name","value":"title"}},{"kind":"Field","name":{"kind":"Name","value":"artifactKind"}},{"kind":"Field","name":{"kind":"Name","value":"storageKind"}},{"kind":"Field","name":{"kind":"Name","value":"mediaType"}},{"kind":"Field","name":{"kind":"Name","value":"previewKind"}},{"kind":"Field","name":{"kind":"Name","value":"markdown"}},{"kind":"Field","name":{"kind":"Name","value":"plainText"}},{"kind":"Field","name":{"kind":"Name","value":"downloadUrl"}},{"kind":"Field","name":{"kind":"Name","value":"externalUrl"}},{"kind":"Field","name":{"kind":"Name","value":"versions"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"artifactVersionId"}},{"kind":"Field","name":{"kind":"Name","value":"versionIndex"}},{"kind":"Field","name":{"kind":"Name","value":"downloadUrl"}},{"kind":"Field","name":{"kind":"Name","value":"externalUrl"}},{"kind":"Field","name":{"kind":"Name","value":"mediaType"}}]}}]}}]}}]} as unknown as DocumentNode<ArtifactVersionDetailQuery, ArtifactVersionDetailQueryVariables>;
export const TaskDetailDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"query","name":{"kind":"Name","value":"TaskDetail"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"taskId"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"String"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"task"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"taskId"},"value":{"kind":"Variable","name":{"kind":"Name","value":"taskId"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"taskId"}},{"kind":"Field","name":{"kind":"Name","value":"title"}},{"kind":"Field","name":{"kind":"Name","value":"requestMarkdown"}},{"kind":"Field","name":{"kind":"Name","value":"complexity"}},{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"ownerHumanId"}},{"kind":"Field","name":{"kind":"Name","value":"source"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"conversationId"}},{"kind":"Field","name":{"kind":"Name","value":"turnId"}},{"kind":"Field","name":{"kind":"Name","value":"itemId"}}]}},{"kind":"Field","name":{"kind":"Name","value":"createdByAgentId"}},{"kind":"Field","name":{"kind":"Name","value":"creationToolCallId"}},{"kind":"Field","name":{"kind":"Name","value":"poolEntryId"}},{"kind":"Field","name":{"kind":"Name","value":"executorModel"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"providerKind"}},{"kind":"Field","name":{"kind":"Name","value":"providerAccountId"}},{"kind":"Field","name":{"kind":"Name","value":"selectionMode"}},{"kind":"Field","name":{"kind":"Name","value":"modelProfile"}},{"kind":"Field","name":{"kind":"Name","value":"reasoningEffort"}},{"kind":"Field","name":{"kind":"Name","value":"selectionSource"}}]}},{"kind":"Field","name":{"kind":"Name","value":"reviewerModel"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"providerKind"}},{"kind":"Field","name":{"kind":"Name","value":"providerAccountId"}},{"kind":"Field","name":{"kind":"Name","value":"selectionMode"}},{"kind":"Field","name":{"kind":"Name","value":"modelProfile"}},{"kind":"Field","name":{"kind":"Name","value":"reasoningEffort"}},{"kind":"Field","name":{"kind":"Name","value":"selectionSource"}}]}},{"kind":"Field","name":{"kind":"Name","value":"revisionIndex"}},{"kind":"Field","name":{"kind":"Name","value":"maxReviewRounds"}},{"kind":"Field","name":{"kind":"Name","value":"finalSubmissionId"}},{"kind":"Field","name":{"kind":"Name","value":"latestRunId"}},{"kind":"Field","name":{"kind":"Name","value":"terminalReason"}},{"kind":"Field","name":{"kind":"Name","value":"errorCode"}},{"kind":"Field","name":{"kind":"Name","value":"errorMessage"}},{"kind":"Field","name":{"kind":"Name","value":"createdAt"}},{"kind":"Field","name":{"kind":"Name","value":"updatedAt"}},{"kind":"Field","name":{"kind":"Name","value":"completedAt"}},{"kind":"Field","name":{"kind":"Name","value":"resumable"}},{"kind":"Field","name":{"kind":"Name","value":"cancellable"}},{"kind":"Field","name":{"kind":"Name","value":"blockingQuestion"}},{"kind":"Field","name":{"kind":"Name","value":"criteria"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"criterionId"}},{"kind":"Field","name":{"kind":"Name","value":"ordinal"}},{"kind":"Field","name":{"kind":"Name","value":"description"}},{"kind":"Field","name":{"kind":"Name","value":"expectedEvidence"}}]}},{"kind":"Field","name":{"kind":"Name","value":"submissions"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"submissionId"}},{"kind":"Field","name":{"kind":"Name","value":"executorRunId"}},{"kind":"Field","name":{"kind":"Name","value":"revisionIndex"}},{"kind":"Field","name":{"kind":"Name","value":"summary"}},{"kind":"Field","name":{"kind":"Name","value":"resultMarkdown"}},{"kind":"Field","name":{"kind":"Name","value":"criteria"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"criterionId"}},{"kind":"Field","name":{"kind":"Name","value":"evidenceMarkdown"}}]}},{"kind":"Field","name":{"kind":"Name","value":"artifacts"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"artifactId"}},{"kind":"Field","name":{"kind":"Name","value":"artifactVersionId"}},{"kind":"Field","name":{"kind":"Name","value":"title"}},{"kind":"Field","name":{"kind":"Name","value":"artifactKind"}},{"kind":"Field","name":{"kind":"Name","value":"storageKind"}},{"kind":"Field","name":{"kind":"Name","value":"mediaType"}},{"kind":"Field","name":{"kind":"Name","value":"downloadUrl"}},{"kind":"Field","name":{"kind":"Name","value":"externalUrl"}}]}},{"kind":"Field","name":{"kind":"Name","value":"createdAt"}}]}},{"kind":"Field","name":{"kind":"Name","value":"reviews"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"reviewId"}},{"kind":"Field","name":{"kind":"Name","value":"reviewerRunId"}},{"kind":"Field","name":{"kind":"Name","value":"reviewedSubmissionId"}},{"kind":"Field","name":{"kind":"Name","value":"overallVerdict"}},{"kind":"Field","name":{"kind":"Name","value":"overallFeedback"}},{"kind":"Field","name":{"kind":"Name","value":"criteria"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"criterionId"}},{"kind":"Field","name":{"kind":"Name","value":"outcome"}},{"kind":"Field","name":{"kind":"Name","value":"evidenceMarkdown"}},{"kind":"Field","name":{"kind":"Name","value":"feedback"}}]}},{"kind":"Field","name":{"kind":"Name","value":"createdAt"}}]}},{"kind":"Field","name":{"kind":"Name","value":"runs"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"runId"}},{"kind":"Field","name":{"kind":"Name","value":"runKind"}},{"kind":"Field","name":{"kind":"Name","value":"agentId"}},{"kind":"Field","name":{"kind":"Name","value":"revisionIndex"}},{"kind":"Field","name":{"kind":"Name","value":"attemptIndex"}},{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"model"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"providerKind"}},{"kind":"Field","name":{"kind":"Name","value":"providerAccountId"}},{"kind":"Field","name":{"kind":"Name","value":"selectionMode"}},{"kind":"Field","name":{"kind":"Name","value":"modelProfile"}},{"kind":"Field","name":{"kind":"Name","value":"reasoningEffort"}},{"kind":"Field","name":{"kind":"Name","value":"selectionSource"}}]}},{"kind":"Field","name":{"kind":"Name","value":"actualProviderKind"}},{"kind":"Field","name":{"kind":"Name","value":"actualModelProfile"}},{"kind":"Field","name":{"kind":"Name","value":"triggeringSubmissionId"}},{"kind":"Field","name":{"kind":"Name","value":"triggeringReviewId"}},{"kind":"Field","name":{"kind":"Name","value":"errorCode"}},{"kind":"Field","name":{"kind":"Name","value":"errorMessage"}},{"kind":"Field","name":{"kind":"Name","value":"executionPolicy"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"maxProviderContinuations"}},{"kind":"Field","name":{"kind":"Name","value":"maxToolCalls"}},{"kind":"Field","name":{"kind":"Name","value":"maxActiveMinutes"}},{"kind":"Field","name":{"kind":"Name","value":"progressAuditInterval"}}]}},{"kind":"Field","name":{"kind":"Name","value":"providerCallCount"}},{"kind":"Field","name":{"kind":"Name","value":"toolCallCount"}},{"kind":"Field","name":{"kind":"Name","value":"cachedInputTokens"}},{"kind":"Field","name":{"kind":"Name","value":"activeMilliseconds"}},{"kind":"Field","name":{"kind":"Name","value":"inputTokens"}},{"kind":"Field","name":{"kind":"Name","value":"outputTokens"}},{"kind":"Field","name":{"kind":"Name","value":"queuedAt"}},{"kind":"Field","name":{"kind":"Name","value":"startedAt"}},{"kind":"Field","name":{"kind":"Name","value":"endedAt"}},{"kind":"Field","name":{"kind":"Name","value":"createdAt"}},{"kind":"Field","name":{"kind":"Name","value":"updatedAt"}}]}}]}}]}}]} as unknown as DocumentNode<TaskDetailQuery, TaskDetailQueryVariables>;
export const TaskReferenceStatusDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"query","name":{"kind":"Name","value":"TaskReferenceStatus"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"taskId"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"String"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"task"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"taskId"},"value":{"kind":"Variable","name":{"kind":"Name","value":"taskId"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"taskId"}},{"kind":"Field","name":{"kind":"Name","value":"title"}},{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"revisionIndex"}}]}}]}}]} as unknown as DocumentNode<TaskReferenceStatusQuery, TaskReferenceStatusQueryVariables>;
export const ResumeTaskDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"ResumeTask"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"taskId"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"String"}}}},{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"message"}},"type":{"kind":"NamedType","name":{"kind":"Name","value":"String"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"resumeTask"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"taskId"},"value":{"kind":"Variable","name":{"kind":"Name","value":"taskId"}}},{"kind":"Argument","name":{"kind":"Name","value":"message"},"value":{"kind":"Variable","name":{"kind":"Name","value":"message"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"taskId"}},{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"latestRunId"}},{"kind":"Field","name":{"kind":"Name","value":"errorCode"}},{"kind":"Field","name":{"kind":"Name","value":"errorMessage"}},{"kind":"Field","name":{"kind":"Name","value":"updatedAt"}}]}}]}}]} as unknown as DocumentNode<ResumeTaskMutation, ResumeTaskMutationVariables>;
export const CancelTaskDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"CancelTask"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"taskId"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"String"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"cancelTask"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"taskId"},"value":{"kind":"Variable","name":{"kind":"Name","value":"taskId"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"taskId"}},{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"latestRunId"}},{"kind":"Field","name":{"kind":"Name","value":"errorCode"}},{"kind":"Field","name":{"kind":"Name","value":"errorMessage"}},{"kind":"Field","name":{"kind":"Name","value":"updatedAt"}}]}}]}}]} as unknown as DocumentNode<CancelTaskMutation, CancelTaskMutationVariables>;
export const TaskRunItemsDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"query","name":{"kind":"Name","value":"TaskRunItems"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"runId"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"String"}}}},{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"after"}},"type":{"kind":"NamedType","name":{"kind":"Name","value":"String"}}},{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"first"}},"type":{"kind":"NamedType","name":{"kind":"Name","value":"Int"}},"defaultValue":{"kind":"IntValue","value":"50"}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"taskRunItems"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"runId"},"value":{"kind":"Variable","name":{"kind":"Name","value":"runId"}}},{"kind":"Argument","name":{"kind":"Name","value":"after"},"value":{"kind":"Variable","name":{"kind":"Name","value":"after"}}},{"kind":"Argument","name":{"kind":"Name","value":"first"},"value":{"kind":"Variable","name":{"kind":"Name","value":"first"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"items"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"itemId"}},{"kind":"Field","name":{"kind":"Name","value":"runId"}},{"kind":"Field","name":{"kind":"Name","value":"sequenceIndex"}},{"kind":"Field","name":{"kind":"Name","value":"roundIndex"}},{"kind":"Field","name":{"kind":"Name","value":"kind"}},{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"correlationId"}},{"kind":"Field","name":{"kind":"Name","value":"parentItemId"}},{"kind":"Field","name":{"kind":"Name","value":"contentText"}},{"kind":"Field","name":{"kind":"Name","value":"payload"}},{"kind":"Field","name":{"kind":"Name","value":"createdAt"}},{"kind":"Field","name":{"kind":"Name","value":"updatedAt"}}]}},{"kind":"Field","name":{"kind":"Name","value":"pageInfo"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"endCursor"}},{"kind":"Field","name":{"kind":"Name","value":"hasNextPage"}}]}}]}}]}}]} as unknown as DocumentNode<TaskRunItemsQuery, TaskRunItemsQueryVariables>;
export const TaskEventsDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"subscription","name":{"kind":"Name","value":"TaskEvents"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"taskId"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"String"}}}},{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"after"}},"type":{"kind":"NamedType","name":{"kind":"Name","value":"String"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"taskEvents"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"taskId"},"value":{"kind":"Variable","name":{"kind":"Name","value":"taskId"}}},{"kind":"Argument","name":{"kind":"Name","value":"after"},"value":{"kind":"Variable","name":{"kind":"Name","value":"after"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"cursor"}},{"kind":"Field","name":{"kind":"Name","value":"kind"}},{"kind":"Field","name":{"kind":"Name","value":"taskId"}},{"kind":"Field","name":{"kind":"Name","value":"runId"}},{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"createdAt"}},{"kind":"Field","name":{"kind":"Name","value":"item"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"itemId"}},{"kind":"Field","name":{"kind":"Name","value":"runId"}},{"kind":"Field","name":{"kind":"Name","value":"sequenceIndex"}},{"kind":"Field","name":{"kind":"Name","value":"roundIndex"}},{"kind":"Field","name":{"kind":"Name","value":"kind"}},{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"correlationId"}},{"kind":"Field","name":{"kind":"Name","value":"parentItemId"}},{"kind":"Field","name":{"kind":"Name","value":"contentText"}},{"kind":"Field","name":{"kind":"Name","value":"payload"}},{"kind":"Field","name":{"kind":"Name","value":"createdAt"}},{"kind":"Field","name":{"kind":"Name","value":"updatedAt"}}]}},{"kind":"Field","name":{"kind":"Name","value":"run"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"runId"}},{"kind":"Field","name":{"kind":"Name","value":"runKind"}},{"kind":"Field","name":{"kind":"Name","value":"agentId"}},{"kind":"Field","name":{"kind":"Name","value":"revisionIndex"}},{"kind":"Field","name":{"kind":"Name","value":"attemptIndex"}},{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"model"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"providerKind"}},{"kind":"Field","name":{"kind":"Name","value":"providerAccountId"}},{"kind":"Field","name":{"kind":"Name","value":"selectionMode"}},{"kind":"Field","name":{"kind":"Name","value":"modelProfile"}},{"kind":"Field","name":{"kind":"Name","value":"reasoningEffort"}},{"kind":"Field","name":{"kind":"Name","value":"selectionSource"}}]}},{"kind":"Field","name":{"kind":"Name","value":"actualProviderKind"}},{"kind":"Field","name":{"kind":"Name","value":"actualModelProfile"}},{"kind":"Field","name":{"kind":"Name","value":"triggeringSubmissionId"}},{"kind":"Field","name":{"kind":"Name","value":"triggeringReviewId"}},{"kind":"Field","name":{"kind":"Name","value":"errorCode"}},{"kind":"Field","name":{"kind":"Name","value":"errorMessage"}},{"kind":"Field","name":{"kind":"Name","value":"executionPolicy"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"maxProviderContinuations"}},{"kind":"Field","name":{"kind":"Name","value":"maxToolCalls"}},{"kind":"Field","name":{"kind":"Name","value":"maxActiveMinutes"}},{"kind":"Field","name":{"kind":"Name","value":"progressAuditInterval"}}]}},{"kind":"Field","name":{"kind":"Name","value":"providerCallCount"}},{"kind":"Field","name":{"kind":"Name","value":"toolCallCount"}},{"kind":"Field","name":{"kind":"Name","value":"cachedInputTokens"}},{"kind":"Field","name":{"kind":"Name","value":"activeMilliseconds"}},{"kind":"Field","name":{"kind":"Name","value":"inputTokens"}},{"kind":"Field","name":{"kind":"Name","value":"outputTokens"}},{"kind":"Field","name":{"kind":"Name","value":"queuedAt"}},{"kind":"Field","name":{"kind":"Name","value":"startedAt"}},{"kind":"Field","name":{"kind":"Name","value":"endedAt"}},{"kind":"Field","name":{"kind":"Name","value":"createdAt"}},{"kind":"Field","name":{"kind":"Name","value":"updatedAt"}}]}}]}}]}}]} as unknown as DocumentNode<TaskEventsSubscription, TaskEventsSubscriptionVariables>;
export const TaskExecutionPolicyDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"query","name":{"kind":"Name","value":"TaskExecutionPolicy"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"taskExecutionPolicy"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"maxProviderContinuations"}},{"kind":"Field","name":{"kind":"Name","value":"maxToolCalls"}},{"kind":"Field","name":{"kind":"Name","value":"maxActiveMinutes"}},{"kind":"Field","name":{"kind":"Name","value":"progressAuditInterval"}}]}}]}}]} as unknown as DocumentNode<TaskExecutionPolicyQuery, TaskExecutionPolicyQueryVariables>;
export const UpdateTaskExecutionPolicyDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"UpdateTaskExecutionPolicy"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"input"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"TaskExecutionPolicyInput"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"updateTaskExecutionPolicy"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"input"},"value":{"kind":"Variable","name":{"kind":"Name","value":"input"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"maxProviderContinuations"}},{"kind":"Field","name":{"kind":"Name","value":"maxToolCalls"}},{"kind":"Field","name":{"kind":"Name","value":"maxActiveMinutes"}},{"kind":"Field","name":{"kind":"Name","value":"progressAuditInterval"}}]}}]}}]} as unknown as DocumentNode<UpdateTaskExecutionPolicyMutation, UpdateTaskExecutionPolicyMutationVariables>;
export const TaskModelPoolsDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"query","name":{"kind":"Name","value":"TaskModelPools"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"taskModelPools"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"poolEntryId"}},{"kind":"Field","name":{"kind":"Name","value":"complexity"}},{"kind":"Field","name":{"kind":"Name","value":"label"}},{"kind":"Field","name":{"kind":"Name","value":"providerKind"}},{"kind":"Field","name":{"kind":"Name","value":"providerAccountId"}},{"kind":"Field","name":{"kind":"Name","value":"modelProfile"}},{"kind":"Field","name":{"kind":"Name","value":"reasoningEffort"}},{"kind":"Field","name":{"kind":"Name","value":"enabled"}},{"kind":"Field","name":{"kind":"Name","value":"sortOrder"}},{"kind":"Field","name":{"kind":"Name","value":"createdAt"}},{"kind":"Field","name":{"kind":"Name","value":"updatedAt"}}]}}]}}]} as unknown as DocumentNode<TaskModelPoolsQuery, TaskModelPoolsQueryVariables>;
export const UpdateTaskModelPoolEntryDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"UpdateTaskModelPoolEntry"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"poolEntryId"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"String"}}}},{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"input"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"TaskModelPoolEntryInput"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"updateTaskModelPoolEntry"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"poolEntryId"},"value":{"kind":"Variable","name":{"kind":"Name","value":"poolEntryId"}}},{"kind":"Argument","name":{"kind":"Name","value":"input"},"value":{"kind":"Variable","name":{"kind":"Name","value":"input"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"poolEntryId"}},{"kind":"Field","name":{"kind":"Name","value":"complexity"}},{"kind":"Field","name":{"kind":"Name","value":"label"}},{"kind":"Field","name":{"kind":"Name","value":"providerKind"}},{"kind":"Field","name":{"kind":"Name","value":"providerAccountId"}},{"kind":"Field","name":{"kind":"Name","value":"modelProfile"}},{"kind":"Field","name":{"kind":"Name","value":"reasoningEffort"}},{"kind":"Field","name":{"kind":"Name","value":"enabled"}},{"kind":"Field","name":{"kind":"Name","value":"sortOrder"}},{"kind":"Field","name":{"kind":"Name","value":"createdAt"}},{"kind":"Field","name":{"kind":"Name","value":"updatedAt"}}]}}]}}]} as unknown as DocumentNode<UpdateTaskModelPoolEntryMutation, UpdateTaskModelPoolEntryMutationVariables>;
export const CreateConversationExternalArtifactDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"CreateConversationExternalArtifact"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"input"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"CreateConversationExternalArtifactInput"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"createConversationExternalArtifact"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"input"},"value":{"kind":"Variable","name":{"kind":"Name","value":"input"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"artifactId"}},{"kind":"Field","name":{"kind":"Name","value":"ownerObjectType"}},{"kind":"Field","name":{"kind":"Name","value":"ownerObjectId"}},{"kind":"Field","name":{"kind":"Name","value":"title"}},{"kind":"Field","name":{"kind":"Name","value":"storageKind"}},{"kind":"Field","name":{"kind":"Name","value":"currentVersion"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"versionIndex"}},{"kind":"Field","name":{"kind":"Name","value":"externalUrl"}},{"kind":"Field","name":{"kind":"Name","value":"downloadUrl"}}]}}]}}]}}]} as unknown as DocumentNode<CreateConversationExternalArtifactMutation, CreateConversationExternalArtifactMutationVariables>;