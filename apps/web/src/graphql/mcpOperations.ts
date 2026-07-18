import { gql } from "@apollo/client";

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
