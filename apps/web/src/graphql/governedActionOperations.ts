import { gql } from "@apollo/client";
import { TasksGateFields, TasksTaskCardFields } from "./tasksFragments";

export const AdapterNextActionFields = gql`
  fragment AdapterNextActionFields on AdapterNextAction {
    kind
    semanticDigest
    applicationId
    expectedApplicationRevision
    grantId
    expectedGrantRevision
    connectionId
    expectedConnectionRevision
    expectedPolicyRevision
    operationIds
    missingScopes
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
      target { serviceName connectionLabel serviceId connectionId accountId }
      disclosure { recipient contentSummary }
      consequence
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

export const HumanInterventionFields = gql`
  fragment HumanInterventionFields on HumanIntervention {
      __typename
      ... on TaskAttention {
        kind
        title
        summary
        validActions
        gate {
          ...TasksGateFields
        }
        task {
          ...TasksTaskCardFields
        }
      }
      ... on GovernedAction {
        actionId
        revision
        conversationId
        taskId
        runId
        actionTask: task {
          ...TasksTaskCardFields
        }
        capabilityName
        reviewRoute
        behavior { readOnly idempotent destructive openWorld }
        safeSummary
        target { serviceName connectionLabel serviceId connectionId accountId }
        disclosure { recipient contentSummary }
        consequence
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
        taskId
        serverDisplayName
        capabilityName
        failureCode
      }
      ... on McpSetupIntervention {
        itemId
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
        taskId
        serviceDisplayName
        capabilityName
        state
        failureCode
      }
      ... on AdapterOauthClientSetupIntervention {
        profileDigest
        displayName
        oauthCredentialSetup: credentialSetup {
          credentialType
          setupUrl
          instructions
          inputKind
          fields { fieldId label }
          documentMediaType
          redirectUri
          normalizationTransform { language sourceDigest source }
          requestAuthTransform { language sourceDigest source }
        }
        dependentDefinitions { semanticDigest displayName }
      }
      ... on AdapterOauthAccountSetupIntervention {
        setupKey
        providerDisplayName
        accountLabel
        accountNextAction: nextAction { ...AdapterNextActionFields }
        dependentDefinitions {
          semanticDigest
          displayName
          action { ...AdapterNextActionFields }
        }
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
        oauthProfileDigest
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
        nextAction { ...AdapterNextActionFields }
        connectionCount
        connections {
          connectionId
          status
          grantId
          accountId
          connectionRevision
          credentialRevision
          grantRevision
          policyRevision
          grantedScopes
          allowedOperations
          policyConfigured
          operationAccess { operationId status missingScopes }
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
        transition {
          addedOperations
          changedOperations
          removedOperations
          authenticationChanged
          affectedConnections
          authenticationRequiredConnections
          consolidatedConnections
        }
      }
  }
  ${TasksGateFields}
  ${TasksTaskCardFields}
  ${AdapterNextActionFields}
`;

export const PendingHumanInterventionsDocument = gql`
  query PendingHumanInterventions($conversationId: String, $taskId: String, $projectId: String, $first: Int = 50) {
    pendingHumanInterventions(conversationId: $conversationId, taskId: $taskId, projectId: $projectId, first: $first) {
      ...HumanInterventionFields
    }
  }
  ${HumanInterventionFields}
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
