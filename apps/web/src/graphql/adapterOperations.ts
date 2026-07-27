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
    reviewed
    superseded
    operations {
      operationId
      method
      path
      effect
      admission
      argumentNames
    }
    manifestJson
    acceptsOauthClientJson
    connectionCount
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
