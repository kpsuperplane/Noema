/** Internal type. DO NOT USE DIRECTLY. */
type Exact<T extends { [key: string]: unknown }> = { [K in keyof T]: T[K] };
/** Internal type. DO NOT USE DIRECTLY. */
export type Incremental<T> = T | { [P in keyof T]?: P extends ' $fragmentName' | '__typename' ? T[P] : never };
import { TypedDocumentNode as DocumentNode } from '@graphql-typed-document-node/core';
/** Agent status exposed through GraphQL. */
export type GraphqlAgentStatus =
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

/** Assistant connection exposed to clients. */
export type GraphqlAssistantConnection =
  /** The daemon is using Codex for chat. */
  | 'CODEX';

/** Local service status shown by clients. */
export type GraphqlLocalServiceStatus =
  /** The local Noema service is running. */
  | 'RUNNING';

/** Input filters for bounded graph-memory inspection. */
export type GraphqlMemoryGraphInput = {
  /** Optional bounded result limit. */
  limit?: number | null | undefined;
  /** Optional predicate id. */
  predicateId?: string | null | undefined;
  /** Optional text query matched against predicate labels and public content. */
  query?: string | null | undefined;
  /** Optional exact sensitivity filter. */
  sensitivity?: string | null | undefined;
  /** Optional claim lifecycle statuses. Defaults are owned by the store. */
  statuses?: Array<string> | null | undefined;
};

/** Memory storage readiness shown by clients. */
export type GraphqlMemoryStorageStatus =
  /** The canonical memory store is initializing. */
  | 'INITIALIZING'
  /** The canonical memory store is ready. */
  | 'READY'
  /** Graph memory writes and retrieval are not available yet. */
  | 'UNAVAILABLE';

/** Onboarding step status exposed through GraphQL. */
export type GraphqlOnboardingStepStatus =
  /** Step blocks the user from continuing. */
  | 'BLOCKED'
  /** Step is complete. */
  | 'COMPLETE';

/** Provider account status exposed through GraphQL. */
export type GraphqlProviderAccountStatus =
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
export type GraphqlProviderAuthAttemptStatus =
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
export type GraphqlProviderAuthMethod =
  /** External manual flow. */
  | 'EXTERNAL_MANUAL'
  /** No auth required. */
  | 'NONE'
  /** OAuth device-code flow. */
  | 'OAUTH_DEVICE_CODE'
  /** Secret input flow. */
  | 'SECRET_INPUT';

/** Input for sending a conversation turn. */
export type GraphqlSendConversationTurnInput = {
  /** Frontend-generated id for optimistic UI correlation. */
  clientMessageId?: string | null | undefined;
  /** Durable Noema conversation id. */
  conversationId: string;
  /** User input. */
  input: string;
};

/** Input for starting a provider auth attempt. */
export type GraphqlStartProviderAuthAttemptInput = {
  /** Requested authentication method. */
  method: GraphqlProviderAuthMethod;
  /** Stable provider account id. */
  providerAccountId: string;
  /** Provider family, such as `codex`. */
  providerKind: string;
};

/** Turn activity status exposed through GraphQL. */
export type GraphqlTurnActivityStatus =
  /** Activity completed. */
  | 'COMPLETED'
  /** Activity failed. */
  | 'FAILED'
  /** Activity started. */
  | 'STARTED';

export type LocalStatusQueryVariables = Exact<{ [key: string]: never; }>;


export type LocalStatusQuery = { localStatus: { localService: GraphqlLocalServiceStatus, assistantConnection: GraphqlAssistantConnection, memoryStorage: GraphqlMemoryStorageStatus, primaryAgentDisplayName: string | null } };

export type OnboardingStatusQueryVariables = Exact<{ [key: string]: never; }>;


export type OnboardingStatusQuery = { onboardingStatus: { isUserOnboarded: boolean, steps: Array<{ id: string, status: GraphqlOnboardingStepStatus, providerKind: string | null, providerAccountId: string | null, accountKey: string | null, displayName: string | null, providerAccountStatus: GraphqlProviderAccountStatus | null, authMethod: GraphqlProviderAuthMethod | null }> } };

export type ProviderAccountsQueryVariables = Exact<{ [key: string]: never; }>;


export type ProviderAccountsQuery = { providerAccounts: Array<{ providerKind: string, accountKey: string, displayName: string, authMethod: string, status: GraphqlProviderAccountStatus, isActive: boolean, isDefault: boolean, lastCheckedAt: string | null, lastAuthenticatedAt: string | null, lastErrorCode: string | null, lastErrorMessage: string | null }> };

export type ProviderAuthAttemptQueryVariables = Exact<{
  attemptId: string;
}>;


export type ProviderAuthAttemptQuery = { providerAuthAttempt: { attemptId: string, providerKind: string, providerAccountId: string, method: GraphqlProviderAuthMethod, status: GraphqlProviderAuthAttemptStatus, verificationUrl: string | null, userCode: string | null, instructions: string | null, errorCode: string | null, errorMessage: string | null } | null };

export type MemoryGraphQueryVariables = Exact<{
  input?: GraphqlMemoryGraphInput | null | undefined;
}>;


export type MemoryGraphQuery = { memoryGraph: { nodes: Array<{ nodeId: string, entityId: string, label: string, entityType: string, redacted: boolean, claimCount: number }>, edges: Array<{ claimId: string, sourceNodeId: string, targetNodeId: string, predicateId: string, predicateLabel: string, fact: string, factRedacted: boolean, status: string, sensitivity: string, confidence: number | null, evidenceCount: number, createdAt: string, updatedAt: string }>, summary: { returnedClaimCount: number, returnedNodeCount: number, limit: number, truncated: boolean } } };

export type MemoryGraphClaimDetailQueryVariables = Exact<{
  claimId: string;
}>;


export type MemoryGraphClaimDetailQuery = { memoryClaim: { claimId: string, fact: string, predicateId: string, predicateLabel: string, subjectEntityId: string, subjectEntityName: string, subjectEntityType: string, objectEntityId: string | null, objectEntityName: string | null, objectEntityType: string | null, status: string, sensitivity: string, confidence: number | null, evidenceCount: number, createdAt: string, updatedAt: string, evidence: Array<{ evidenceId: string | null, sourceItemId: string | null, authority: string, excerpt: string | null, observedAt: string | null, createdAt: string }> } | null };

export type StartProviderAuthAttemptMutationVariables = Exact<{
  input: GraphqlStartProviderAuthAttemptInput;
}>;


export type StartProviderAuthAttemptMutation = { startProviderAuthAttempt: { attemptId: string, providerKind: string, providerAccountId: string, method: GraphqlProviderAuthMethod, status: GraphqlProviderAuthAttemptStatus, verificationUrl: string | null, userCode: string | null, instructions: string | null, errorCode: string | null, errorMessage: string | null } };

export type StartPrimaryConversationMutationVariables = Exact<{ [key: string]: never; }>;


export type StartPrimaryConversationMutation = { startPrimaryConversation: { conversationId: string, provider: string, replay: Array<{ itemId: string, turnId: string | null, item:
        | { __typename: 'GraphqlA2UiCard', id: string, schema: string, payload: unknown }
        | { __typename: 'GraphqlActivity', id: string, activityKind: string, status: GraphqlTurnActivityStatus, title: string, summary: string | null, metadata: unknown }
        | { __typename: 'GraphqlAssistantText', text: string }
        | { __typename: 'GraphqlErrorNotice', message: string, recoverable: boolean }
        | { __typename: 'GraphqlUserText', text: string }
       }> } };

export type SendConversationTurnMutationVariables = Exact<{
  input: GraphqlSendConversationTurnInput;
}>;


export type SendConversationTurnMutation = { sendConversationTurn: { conversationId: string, clientMessageId: string | null } };

export type ConversationEventsSubscriptionVariables = Exact<{
  conversationId: string;
}>;


export type ConversationEventsSubscription = { conversationEvents:
    | { __typename: 'GraphqlAgentStatusEvent', conversationId: string, status: GraphqlAgentStatus }
    | { __typename: 'GraphqlAssistantTextDeltaEvent', conversationId: string, streamId: string, delta: string, deltaTurnId: string }
    | { __typename: 'GraphqlConversationItemEvent', conversationId: string, clientMessageId: string | null, itemId: string, turnId: string | null, metadata: unknown, item:
        | { __typename: 'GraphqlA2UiCard', id: string, schema: string, payload: unknown }
        | { __typename: 'GraphqlActivity', id: string, activityKind: string, status: GraphqlTurnActivityStatus, title: string, summary: string | null, metadata: unknown }
        | { __typename: 'GraphqlAssistantText', text: string }
        | { __typename: 'GraphqlErrorNotice', message: string, recoverable: boolean }
        | { __typename: 'GraphqlUserText', text: string }
       }
    | { __typename: 'GraphqlSubscriptionReadyEvent' }
    | { __typename: 'GraphqlTurnCompletedEvent', conversationId: string, clientMessageId: string | null }
   };


export const LocalStatusDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"query","name":{"kind":"Name","value":"LocalStatus"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"localStatus"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"localService"}},{"kind":"Field","name":{"kind":"Name","value":"assistantConnection"}},{"kind":"Field","name":{"kind":"Name","value":"memoryStorage"}},{"kind":"Field","name":{"kind":"Name","value":"primaryAgentDisplayName"}}]}}]}}]} as unknown as DocumentNode<LocalStatusQuery, LocalStatusQueryVariables>;
export const OnboardingStatusDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"query","name":{"kind":"Name","value":"OnboardingStatus"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"onboardingStatus"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"isUserOnboarded"}},{"kind":"Field","name":{"kind":"Name","value":"steps"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"providerKind"}},{"kind":"Field","name":{"kind":"Name","value":"providerAccountId"}},{"kind":"Field","name":{"kind":"Name","value":"accountKey"}},{"kind":"Field","name":{"kind":"Name","value":"displayName"}},{"kind":"Field","name":{"kind":"Name","value":"providerAccountStatus"}},{"kind":"Field","name":{"kind":"Name","value":"authMethod"}}]}}]}}]}}]} as unknown as DocumentNode<OnboardingStatusQuery, OnboardingStatusQueryVariables>;
export const ProviderAccountsDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"query","name":{"kind":"Name","value":"ProviderAccounts"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"providerAccounts"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"providerKind"}},{"kind":"Field","name":{"kind":"Name","value":"accountKey"}},{"kind":"Field","name":{"kind":"Name","value":"displayName"}},{"kind":"Field","name":{"kind":"Name","value":"authMethod"}},{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"isActive"}},{"kind":"Field","name":{"kind":"Name","value":"isDefault"}},{"kind":"Field","name":{"kind":"Name","value":"lastCheckedAt"}},{"kind":"Field","name":{"kind":"Name","value":"lastAuthenticatedAt"}},{"kind":"Field","name":{"kind":"Name","value":"lastErrorCode"}},{"kind":"Field","name":{"kind":"Name","value":"lastErrorMessage"}}]}}]}}]} as unknown as DocumentNode<ProviderAccountsQuery, ProviderAccountsQueryVariables>;
export const ProviderAuthAttemptDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"query","name":{"kind":"Name","value":"ProviderAuthAttempt"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"attemptId"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"String"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"providerAuthAttempt"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"attemptId"},"value":{"kind":"Variable","name":{"kind":"Name","value":"attemptId"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"attemptId"}},{"kind":"Field","name":{"kind":"Name","value":"providerKind"}},{"kind":"Field","name":{"kind":"Name","value":"providerAccountId"}},{"kind":"Field","name":{"kind":"Name","value":"method"}},{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"verificationUrl"}},{"kind":"Field","name":{"kind":"Name","value":"userCode"}},{"kind":"Field","name":{"kind":"Name","value":"instructions"}},{"kind":"Field","name":{"kind":"Name","value":"errorCode"}},{"kind":"Field","name":{"kind":"Name","value":"errorMessage"}}]}}]}}]} as unknown as DocumentNode<ProviderAuthAttemptQuery, ProviderAuthAttemptQueryVariables>;
export const MemoryGraphDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"query","name":{"kind":"Name","value":"MemoryGraph"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"input"}},"type":{"kind":"NamedType","name":{"kind":"Name","value":"GraphqlMemoryGraphInput"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"memoryGraph"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"input"},"value":{"kind":"Variable","name":{"kind":"Name","value":"input"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"nodes"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"nodeId"}},{"kind":"Field","name":{"kind":"Name","value":"entityId"}},{"kind":"Field","name":{"kind":"Name","value":"label"}},{"kind":"Field","name":{"kind":"Name","value":"entityType"}},{"kind":"Field","name":{"kind":"Name","value":"redacted"}},{"kind":"Field","name":{"kind":"Name","value":"claimCount"}}]}},{"kind":"Field","name":{"kind":"Name","value":"edges"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"claimId"}},{"kind":"Field","name":{"kind":"Name","value":"sourceNodeId"}},{"kind":"Field","name":{"kind":"Name","value":"targetNodeId"}},{"kind":"Field","name":{"kind":"Name","value":"predicateId"}},{"kind":"Field","name":{"kind":"Name","value":"predicateLabel"}},{"kind":"Field","name":{"kind":"Name","value":"fact"}},{"kind":"Field","name":{"kind":"Name","value":"factRedacted"}},{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"sensitivity"}},{"kind":"Field","name":{"kind":"Name","value":"confidence"}},{"kind":"Field","name":{"kind":"Name","value":"evidenceCount"}},{"kind":"Field","name":{"kind":"Name","value":"createdAt"}},{"kind":"Field","name":{"kind":"Name","value":"updatedAt"}}]}},{"kind":"Field","name":{"kind":"Name","value":"summary"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"returnedClaimCount"}},{"kind":"Field","name":{"kind":"Name","value":"returnedNodeCount"}},{"kind":"Field","name":{"kind":"Name","value":"limit"}},{"kind":"Field","name":{"kind":"Name","value":"truncated"}}]}}]}}]}}]} as unknown as DocumentNode<MemoryGraphQuery, MemoryGraphQueryVariables>;
export const MemoryGraphClaimDetailDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"query","name":{"kind":"Name","value":"MemoryGraphClaimDetail"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"claimId"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"String"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"memoryClaim"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"claimId"},"value":{"kind":"Variable","name":{"kind":"Name","value":"claimId"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"claimId"}},{"kind":"Field","name":{"kind":"Name","value":"fact"}},{"kind":"Field","name":{"kind":"Name","value":"predicateId"}},{"kind":"Field","name":{"kind":"Name","value":"predicateLabel"}},{"kind":"Field","name":{"kind":"Name","value":"subjectEntityId"}},{"kind":"Field","name":{"kind":"Name","value":"subjectEntityName"}},{"kind":"Field","name":{"kind":"Name","value":"subjectEntityType"}},{"kind":"Field","name":{"kind":"Name","value":"objectEntityId"}},{"kind":"Field","name":{"kind":"Name","value":"objectEntityName"}},{"kind":"Field","name":{"kind":"Name","value":"objectEntityType"}},{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"sensitivity"}},{"kind":"Field","name":{"kind":"Name","value":"confidence"}},{"kind":"Field","name":{"kind":"Name","value":"evidenceCount"}},{"kind":"Field","name":{"kind":"Name","value":"createdAt"}},{"kind":"Field","name":{"kind":"Name","value":"updatedAt"}},{"kind":"Field","name":{"kind":"Name","value":"evidence"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"evidenceId"}},{"kind":"Field","name":{"kind":"Name","value":"sourceItemId"}},{"kind":"Field","name":{"kind":"Name","value":"authority"}},{"kind":"Field","name":{"kind":"Name","value":"excerpt"}},{"kind":"Field","name":{"kind":"Name","value":"observedAt"}},{"kind":"Field","name":{"kind":"Name","value":"createdAt"}}]}}]}}]}}]} as unknown as DocumentNode<MemoryGraphClaimDetailQuery, MemoryGraphClaimDetailQueryVariables>;
export const StartProviderAuthAttemptDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"StartProviderAuthAttempt"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"input"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"GraphqlStartProviderAuthAttemptInput"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"startProviderAuthAttempt"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"input"},"value":{"kind":"Variable","name":{"kind":"Name","value":"input"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"attemptId"}},{"kind":"Field","name":{"kind":"Name","value":"providerKind"}},{"kind":"Field","name":{"kind":"Name","value":"providerAccountId"}},{"kind":"Field","name":{"kind":"Name","value":"method"}},{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"verificationUrl"}},{"kind":"Field","name":{"kind":"Name","value":"userCode"}},{"kind":"Field","name":{"kind":"Name","value":"instructions"}},{"kind":"Field","name":{"kind":"Name","value":"errorCode"}},{"kind":"Field","name":{"kind":"Name","value":"errorMessage"}}]}}]}}]} as unknown as DocumentNode<StartProviderAuthAttemptMutation, StartProviderAuthAttemptMutationVariables>;
export const StartPrimaryConversationDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"StartPrimaryConversation"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"startPrimaryConversation"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"conversationId"}},{"kind":"Field","name":{"kind":"Name","value":"provider"}},{"kind":"Field","name":{"kind":"Name","value":"replay"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"itemId"}},{"kind":"Field","name":{"kind":"Name","value":"turnId"}},{"kind":"Field","name":{"kind":"Name","value":"item"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"__typename"}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"GraphqlUserText"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"text"}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"GraphqlAssistantText"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"text"}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"GraphqlActivity"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"activityKind"}},{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"title"}},{"kind":"Field","name":{"kind":"Name","value":"summary"}},{"kind":"Field","name":{"kind":"Name","value":"metadata"}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"GraphqlA2UiCard"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"schema"}},{"kind":"Field","name":{"kind":"Name","value":"payload"}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"GraphqlErrorNotice"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"message"}},{"kind":"Field","name":{"kind":"Name","value":"recoverable"}}]}}]}}]}}]}}]}}]} as unknown as DocumentNode<StartPrimaryConversationMutation, StartPrimaryConversationMutationVariables>;
export const SendConversationTurnDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"mutation","name":{"kind":"Name","value":"SendConversationTurn"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"input"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"GraphqlSendConversationTurnInput"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"sendConversationTurn"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"input"},"value":{"kind":"Variable","name":{"kind":"Name","value":"input"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"conversationId"}},{"kind":"Field","name":{"kind":"Name","value":"clientMessageId"}}]}}]}}]} as unknown as DocumentNode<SendConversationTurnMutation, SendConversationTurnMutationVariables>;
export const ConversationEventsDocument = {"kind":"Document","definitions":[{"kind":"OperationDefinition","operation":"subscription","name":{"kind":"Name","value":"ConversationEvents"},"variableDefinitions":[{"kind":"VariableDefinition","variable":{"kind":"Variable","name":{"kind":"Name","value":"conversationId"}},"type":{"kind":"NonNullType","type":{"kind":"NamedType","name":{"kind":"Name","value":"String"}}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"conversationEvents"},"arguments":[{"kind":"Argument","name":{"kind":"Name","value":"conversationId"},"value":{"kind":"Variable","name":{"kind":"Name","value":"conversationId"}}}],"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"__typename"}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"GraphqlConversationItemEvent"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"conversationId"}},{"kind":"Field","name":{"kind":"Name","value":"clientMessageId"}},{"kind":"Field","name":{"kind":"Name","value":"itemId"}},{"kind":"Field","name":{"kind":"Name","value":"turnId"}},{"kind":"Field","name":{"kind":"Name","value":"metadata"}},{"kind":"Field","name":{"kind":"Name","value":"item"},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"__typename"}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"GraphqlUserText"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"text"}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"GraphqlAssistantText"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"text"}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"GraphqlActivity"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"activityKind"}},{"kind":"Field","name":{"kind":"Name","value":"status"}},{"kind":"Field","name":{"kind":"Name","value":"title"}},{"kind":"Field","name":{"kind":"Name","value":"summary"}},{"kind":"Field","name":{"kind":"Name","value":"metadata"}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"GraphqlA2UiCard"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"id"}},{"kind":"Field","name":{"kind":"Name","value":"schema"}},{"kind":"Field","name":{"kind":"Name","value":"payload"}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"GraphqlErrorNotice"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"message"}},{"kind":"Field","name":{"kind":"Name","value":"recoverable"}}]}}]}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"GraphqlAssistantTextDeltaEvent"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"conversationId"}},{"kind":"Field","alias":{"kind":"Name","value":"deltaTurnId"},"name":{"kind":"Name","value":"turnId"}},{"kind":"Field","name":{"kind":"Name","value":"streamId"}},{"kind":"Field","name":{"kind":"Name","value":"delta"}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"GraphqlAgentStatusEvent"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"conversationId"}},{"kind":"Field","name":{"kind":"Name","value":"status"}}]}},{"kind":"InlineFragment","typeCondition":{"kind":"NamedType","name":{"kind":"Name","value":"GraphqlTurnCompletedEvent"}},"selectionSet":{"kind":"SelectionSet","selections":[{"kind":"Field","name":{"kind":"Name","value":"conversationId"}},{"kind":"Field","name":{"kind":"Name","value":"clientMessageId"}}]}}]}}]}}]} as unknown as DocumentNode<ConversationEventsSubscription, ConversationEventsSubscriptionVariables>;