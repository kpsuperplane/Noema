import { gql } from "@apollo/client";

export const McpSettingsDocument = gql`
  query McpSettings {
    mcpServers {
      mcpServerId
      displayName
      transportKind
      enabled
      dataSharingPolicy
      unsafeActionPolicy
      policyRevision
      healthStatus
      authStatus
      toolCount
      availableToolCount
      pendingToolCount
      defaultedToolCount
      disabledToolCount
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
      policy {
        mcpToolId
        readOnly { value source }
        idempotent { value source }
        destructive { value source }
        openWorld { value source }
        status
        policyRevision
        metadataFingerprint
      }
    }
  }
`;

export const SaveMcpProviderPolicyDocument = gql`
  mutation SaveMcpProviderPolicy($input: SaveMcpProviderPolicyInput!) {
    saveMcpProviderPolicy(input: $input) {
      mcpServerId
      dataSharingPolicy
      unsafeActionPolicy
      policyRevision
      enabled
    }
  }
`;

export const SaveMcpToolOverrideDocument = gql`
  mutation SaveMcpToolOverride($input: SaveMcpToolOverrideInput!) {
    saveMcpToolOverride(input: $input) {
      mcpToolId
      readOnly { value source }
      idempotent { value source }
      destructive { value source }
      openWorld { value source }
      status
      policyRevision
      metadataFingerprint
    }
  }
`;

export const ResetMcpToolPolicyDocument = gql`
  mutation ResetMcpToolPolicy($mcpToolId: String!) {
    resetMcpToolPolicy(mcpToolId: $mcpToolId) { mcpToolId status policyRevision }
  }
`;

export const SetMcpToolEnabledDocument = gql`
  mutation SetMcpToolEnabled($mcpToolId: String!, $enabled: Boolean!) {
    setMcpToolEnabled(mcpToolId: $mcpToolId, enabled: $enabled) {
      mcpToolId
      status
      policyRevision
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
      dataSharingPolicy
      unsafeActionPolicy
      policyRevision
      healthStatus
      authStatus
      toolCount
      availableToolCount
      pendingToolCount
      defaultedToolCount
      disabledToolCount
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
