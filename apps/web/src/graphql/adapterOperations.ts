import { gql } from "@apollo/client";

const AdapterDefinitionFields = gql`
  fragment AdapterDefinitionFields on AdapterDefinition {
    semanticDigest
    definitionId
    adapterId
    displayName
    definitionRevision
    sourceReference
    origin
    authenticationMode
    scopes
    clientSetupUrl
    oauthRedirectUri
    accountIdentityOperationId
    reviewed
    superseded
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
    manifestJson
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
      policyConfigured
    }
  }
`;

export const AdapterDefinitionsDocument = gql`
  query AdapterDefinitions {
    adapterDefinitions {
      ...AdapterDefinitionFields
    }
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

export const ImportAdapterOauthClientJsonDocument = gql`
  mutation ImportAdapterOauthClientJson($input: ImportAdapterOauthClientJsonInput!) {
    importAdapterOauthClientJson(input: $input) {
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
