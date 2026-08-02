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
