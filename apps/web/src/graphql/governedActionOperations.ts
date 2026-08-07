import { gql } from "@apollo/client";
import { WorkGateFields, WorkTaskCardFields } from "./workFragments";

export const ResolveGovernedActionDocument = gql`
  mutation ResolveGovernedAction($input: ResolveGovernedActionInput!) {
    resolveGovernedAction(input: $input) {
      actionId
      revision
      conversationId
      taskId
      runId
      capabilityName
      reviewRoute
      behavior { readOnly idempotent destructive openWorld }
      safeSummary
      arguments
      assessment {
        status
        authorization
        risk
        reasonCodes
        explanation
      }
      browserSessionAvailable
      state
      output
      failureCode
    }
  }
`;

export const PendingHumanInterventionsDocument = gql`
  query PendingHumanInterventions($conversationId: String, $taskId: String, $projectId: String, $first: Int = 50) {
    pendingHumanInterventions(conversationId: $conversationId, taskId: $taskId, projectId: $projectId, first: $first) {
      __typename
      ... on TaskAttention {
        kind
        title
        summary
        validActions
        gate {
          ...WorkGateFields
        }
        task {
          ...WorkTaskCardFields
        }
      }
      ... on GovernedAction {
        actionId
        revision
        conversationId
        taskId
        runId
        capabilityName
        reviewRoute
        behavior { readOnly idempotent destructive openWorld }
        safeSummary
        arguments
        assessment {
          status
          authorization
          risk
          reasonCodes
          explanation
        }
        browserSessionAvailable
        failureCode
      }
      ... on McpAuthenticationIntervention {
        requestId
        revision
        conversationId
        taskId
        runId
        mcpServerId
        serverDisplayName
        capabilityName
        failureCode
      }
      ... on McpSetupIntervention {
        itemId
        setupConversationId: conversationId
        setupStatus
        displayName
        description
        serviceUrl
        endpointUrl
        oauthSupported
        discoveredToolCount
        setupMcpServerId: mcpServerId
        connectionRevision
        policyRevision
        toolCount
      }
      ... on AdapterAuthenticationIntervention {
        requestId
        revision
        conversationId
        taskId
        runId
        adapterConnectionId
        serviceDisplayName
        capabilityName
        state
        failureCode
      }
      ... on AdapterDefinition {
        semanticDigest
        definitionId
        adapterId
        displayName
        definitionRevision
        sourceReference
        origin
        authenticationMode
        scopes
        credentialSetup {
          credentialType
          setupUrl
          instructions
          inputKind
          fields {
            fieldId
            label
          }
          documentMediaType
          redirectUri
          normalizationTransform {
            language
            sourceDigest
            source
          }
          requestAuthTransform {
            language
            sourceDigest
            source
          }
        }
        accountIdentityOperationId
        reviewed
        connectionCount
        connections {
          connectionId
          status
          accountKind
          connectionRevision
          credentialRevision
          grantRevision
          policyRevision
          grantedScopes
          allowedOperations
          policyConfigured
        }
        operations {
          operationId
          method
          path
          readOnly
          idempotent
          destructive
          openWorld
          argumentNames
          responseTransform {
            language
            sourceDigest
            source
            acceptedContentTypes
            outputSchemaJson
          }
        }
      }
    }
  }
  ${WorkGateFields}
  ${WorkTaskCardFields}
`;

export const StartMcpAuthenticationDocument = gql`
  mutation StartMcpAuthentication($input: StartMcpAuthenticationInput!) {
    startMcpAuthentication(input: $input) {
      attemptId
      status
      authorizationUrl
      errorMessage
    }
  }
`;

export const SkipMcpAuthenticationDocument = gql`
  mutation SkipMcpAuthentication($input: SkipMcpAuthenticationInput!) {
    skipMcpAuthentication(input: $input) {
      requestId
      revision
      state
      failureCode
    }
  }
`;

export const ResolveMcpSetupInterventionDocument = gql`
  mutation ResolveMcpSetupIntervention($input: ResolveMcpSetupInterventionInput!) {
    resolveMcpSetupIntervention(input: $input)
  }
`;

export const StartAdapterAuthenticationDocument = gql`
  mutation StartAdapterAuthentication($input: StartAdapterAuthenticationInput!) {
    startAdapterAuthentication(input: $input) {
      attemptId
      authorizationUrl
      expiresAtEpochSeconds
    }
  }
`;

export const SkipAdapterAuthenticationDocument = gql`
  mutation SkipAdapterAuthentication($input: SkipAdapterAuthenticationInput!) {
    skipAdapterAuthentication(input: $input) {
      requestId
      revision
      state
      failureCode
    }
  }
`;
