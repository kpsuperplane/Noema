import { gql } from "@apollo/client";
import { CapabilityIntegrationFields } from "./capabilityIntegrationOperations";

export const AdapterDefinitionFields = gql`
  fragment AdapterDefinitionFields on AdapterDefinition {
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
    superseded
    nextAction {
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
    connectionActions {
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
    operations {
      operationId
      method
      path
      readOnly
      idempotent
      destructive
      openWorld
      argumentNames
      acceptedScopeSets { scopes }
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
    manifestJson
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
      operationAccess {
        operationId
        status
        missingScopes
      }
    }
  }
`;

export const AdapterOauthStateFields = gql`
  fragment AdapterOauthStateFields on AdapterOauthState {
    profiles {
      profileDigest profileId displayName grantAudience
      credentialSetup {
        credentialType setupUrl instructions inputKind
        fields { fieldId label }
        documentMediaType redirectUri
      }
    }
    applications {
      applicationId profileDigest providerDisplayName callbackMode clientId
      projectLabel revision status grantCount accountCount
    }
    accounts { accountId profileDigest accountLabel revision grantIds }
    grants {
      grantId applicationId accountId accountLabel providerDisplayName audience
      desiredScopes grantedScopes authorityRevision tokenRevision status connectionIds
    }
  }
`;

export const AdapterManagementRootDocument = gql`
  query AdapterManagementRoot {
    adapterManagement {
      library { id definitionId name description revision semanticDigest sourceReference operationIds }
      definitions { ...AdapterDefinitionFields }
      oauthState { ...AdapterOauthStateFields }
      integrations { ...CapabilityIntegrationFields }
    }
  }
  ${AdapterDefinitionFields}
  ${AdapterOauthStateFields}
  ${CapabilityIntegrationFields}
`;

export const ConnectAdapterLibraryDocument = gql`
  mutation ConnectAdapterLibrary($input: ConnectAdapterLibraryInput!) {
    connectAdapterLibrary(input: $input) { ...AdapterDefinitionFields }
  }
  ${AdapterDefinitionFields}
`;

export const ApproveAdapterDefinitionDocument = gql`
  mutation ApproveAdapterDefinition($input: ApproveAdapterDefinitionInput!) {
    approveAdapterDefinition(input: $input) {
      ...AdapterDefinitionFields
    }
  }
  ${AdapterDefinitionFields}
`;

export const CancelAdapterDefinitionDocument = gql`
  mutation CancelAdapterDefinition($input: CancelAdapterDefinitionInput!) {
    cancelAdapterDefinition(input: $input)
  }
`;

export const SetupAdapterConnectionDocument = gql`
  mutation SetupAdapterConnection($input: SetupAdapterConnectionInput!) {
    setupAdapterConnection(input: $input) {
      ...AdapterDefinitionFields
    }
  }
  ${AdapterDefinitionFields}
`;

export const DeleteAdapterConnectionDocument = gql`
  mutation DeleteAdapterConnection($input: DeleteAdapterConnectionInput!) {
    deleteAdapterConnection(input: $input)
  }
`;

export const DeleteAdapterServiceDocument = gql`
  mutation DeleteAdapterService($input: DeleteAdapterServiceInput!) {
    deleteAdapterService(input: $input)
  }
`;

export const StartAdapterOauthSetupDocument = gql`
  mutation StartAdapterOauthSetup($input: StartAdapterOauthSetupInput!) {
    startAdapterOauthSetup(input: $input) {
      attemptId
      authorizationUrl
      expiresAtEpochSeconds
    }
  }
`;

export const ImportAdapterOauthApplicationDocument = gql`
  mutation ImportAdapterOauthApplication($input: ImportAdapterOauthApplicationInput!) {
    importAdapterOauthApplication(input: $input) {
      applicationId
      profileDigest
      revision
      status
    }
  }
`;

export const ReplaceAdapterOauthApplicationDocument = gql`
  mutation ReplaceAdapterOauthApplication($input: ReplaceAdapterOauthApplicationInput!) {
    replaceAdapterOauthApplication(input: $input) {
      applicationId
      revision
      status
    }
  }
`;

export const AttachAdapterOauthConnectionDocument = gql`
  mutation AttachAdapterOauthConnection($input: AttachAdapterOauthConnectionInput!) {
    attachAdapterOauthConnection(input: $input) {
      semanticDigest
      connectionCount
      connections { connectionId grantId policyConfigured }
    }
  }
`;

export const DisconnectAdapterOauthGrantDocument = gql`
  mutation DisconnectAdapterOauthGrant($input: DisconnectAdapterOauthGrantInput!) {
    disconnectAdapterOauthGrant(input: $input) { grantId authorityRevision status connectionIds }
  }
`;

export const SaveAdapterOauthGrantLabelDocument = gql`
  mutation SaveAdapterOauthGrantLabel($input: SaveAdapterOauthGrantLabelInput!) {
    saveAdapterOauthGrantLabel(input: $input) { grantId accountLabel authorityRevision }
  }
`;

export const DeleteAdapterOauthApplicationDocument = gql`
  mutation DeleteAdapterOauthApplication($input: DeleteAdapterOauthApplicationInput!) {
    deleteAdapterOauthApplication(input: $input)
  }
`;

export const SetAdapterConnectionActiveDocument = gql`
  mutation SetAdapterConnectionActive($input: SetAdapterConnectionActiveInput!) {
    setAdapterConnectionActive(input: $input) { semanticDigest connectionCount }
  }
`;

export const AdapterOauthAttemptDocument = gql`
  query AdapterOauthAttempt($attemptId: String!) {
    adapterOauthAttempt(attemptId: $attemptId) {
      attemptId
      semanticDigest
      grantId
      grantRevision
      status
    }
  }
`;

export const AdapterOauthAttemptEventsDocument = gql`
  subscription AdapterOauthAttemptEvents($attemptId: String!) {
    adapterOauthAttemptEvents(attemptId: $attemptId) {
      attemptId
      semanticDigest
      grantId
      grantRevision
      status
    }
  }
`;
