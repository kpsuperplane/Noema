import { gql } from "@apollo/client";

const ApnsProviderStatusFields = gql`
  fragment ApnsProviderStatusFields on ApnsProviderStatus {
    configured
    teamId
    keyId
    topic
    keyFingerprint
    revision
    updatedAt
    lastErrorCode
    lastErrorAt
  }
`;

export const ApnsProviderStatusDocument = gql`
  query ApnsProviderStatus {
    apnsProviderStatus { ...ApnsProviderStatusFields }
  }
  ${ApnsProviderStatusFields}
`;

export const ConfigureApnsProviderDocument = gql`
  mutation ConfigureApnsProvider($input: ConfigureApnsProviderInput!) {
    configureApnsProvider(input: $input) { ...ApnsProviderStatusFields }
  }
  ${ApnsProviderStatusFields}
`;

export const RemoveApnsProviderDocument = gql`
  mutation RemoveApnsProvider($expectedRevision: Int!) {
    removeApnsProvider(expectedRevision: $expectedRevision) { ...ApnsProviderStatusFields }
  }
  ${ApnsProviderStatusFields}
`;
