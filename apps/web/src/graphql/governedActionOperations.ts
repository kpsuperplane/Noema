import { gql } from "@apollo/client";

export const PendingGovernedActionsDocument = gql`
  query PendingGovernedActions($conversationId: String, $taskId: String, $first: Int = 50) {
    pendingGovernedActions(conversationId: $conversationId, taskId: $taskId, first: $first) {
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
      state
      failureCode
    }
  }
`;

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
      state
      output
      failureCode
    }
  }
`;

export const PendingHumanInterventionsDocument = gql`
  query PendingHumanInterventions($conversationId: String, $taskId: String, $first: Int = 50) {
    pendingHumanInterventions(conversationId: $conversationId, taskId: $taskId, first: $first) {
      __typename
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
      ... on AdapterDefinition {
        semanticDigest
        displayName
        definitionRevision
        sourceReference
        origin
        authenticationMode
        scopes
        clientSetupUrl
        oauthRedirectUri
        manifestJson
        reviewed
        acceptsOauthClientJson
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
        }
        operations {
          operationId
          method
          path
          effect
          admission
          argumentNames
        }
      }
    }
  }
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
