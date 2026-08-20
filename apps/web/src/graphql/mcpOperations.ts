import { gql } from "@apollo/client";
import { CapabilityIntegrationFields } from "./capabilityIntegrationOperations";

export const McpServerFields = gql`
  fragment McpServerFields on McpServer {
    mcpServerId
    displayName
    transportKind
    healthStatus
    authStatus
    toolCount
    pendingToolCount
    browserOauthReauthenticationSupported
  }
`;

export const McpManagementRootDocument = gql`
  query McpManagementRoot {
    mcpServers { ...McpServerFields }
    capabilityIntegrations(kind: MCP) { ...CapabilityIntegrationFields }
  }
  ${McpServerFields}
  ${CapabilityIntegrationFields}
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
      connectionRevision
      policyRevision
      displayName
      transportKind
      healthStatus
      authStatus
      toolCount
      pendingToolCount
      browserOauthReauthenticationSupported
    }
  }
`;

export const AddMcpConnectionDocument = gql`
  mutation AddMcpConnection($input: AddMcpConnectionInput!) {
    addMcpConnection(input: $input) {
      ...McpServerSetupResultFields
    }
  }
  ${McpServerSetupResultFields}
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
