import { gql } from "@apollo/client";

export const CapabilityConnectionFields = gql`
  fragment CapabilityConnectionFields on CapabilityConnection {
    kind definitionId connectionId name connectionLabel sourceRevision connectionRevision
    credentialRevision grantRevision policyRevision
    status healthStatus authStatus dataSharingPolicy unsafeActionPolicy
    toolCount availableToolCount pendingToolCount defaultedToolCount disabledToolCount
    sourceDetails
  }
`;

export const CapabilityManagedToolFields = gql`
  fragment CapabilityManagedToolFields on CapabilityManagedTool {
    kind connectionId toolId name description enabled status policyRevision sourceRevision
    decisionPreview sourceDetails
    readOnly { value source }
    idempotent { value source }
    destructive { value source }
    openWorld { value source }
  }
`;

export const CapabilityIntegrationFields = gql`
  fragment CapabilityIntegrationFields on CapabilityIntegration {
    kind definitionId name sourceRevision reviewed sourceSummary
    connections { ...CapabilityConnectionFields }
  }
  ${CapabilityConnectionFields}
`;

export const CapabilityConnectionDocument = gql`
  query CapabilityConnection($ref: CapabilityConnectionRefInput!) {
    capabilityConnection(ref: $ref) { ...CapabilityConnectionFields }
    capabilityTools(ref: $ref) { ...CapabilityManagedToolFields }
  }
  ${CapabilityConnectionFields}
  ${CapabilityManagedToolFields}
`;

export const SaveCapabilityConnectionPolicyDocument = gql`
  mutation SaveCapabilityConnectionPolicy($input: SaveCapabilityConnectionPolicyInput!) {
    saveCapabilityConnectionPolicy(input: $input) { ...CapabilityConnectionFields }
  }
  ${CapabilityConnectionFields}
`;

export const SaveCapabilityConnectionLabelDocument = gql`
  mutation SaveCapabilityConnectionLabel($input: SaveCapabilityConnectionLabelInput!) {
    saveCapabilityConnectionLabel(input: $input) { ...CapabilityConnectionFields }
  }
  ${CapabilityConnectionFields}
`;

export const SaveCapabilityToolOverrideDocument = gql`
  mutation SaveCapabilityToolOverride($input: SaveCapabilityToolOverrideInput!) {
    saveCapabilityToolOverride(input: $input) { ...CapabilityManagedToolFields }
  }
  ${CapabilityManagedToolFields}
`;

export const ResetCapabilityToolPolicyDocument = gql`
  mutation ResetCapabilityToolPolicy($input: ResetCapabilityToolPolicyInput!) {
    resetCapabilityToolPolicy(input: $input) { ...CapabilityManagedToolFields }
  }
  ${CapabilityManagedToolFields}
`;

export const SetCapabilityToolEnabledDocument = gql`
  mutation SetCapabilityToolEnabled($input: SetCapabilityToolEnabledInput!) {
    setCapabilityToolEnabled(input: $input) { ...CapabilityManagedToolFields }
  }
  ${CapabilityManagedToolFields}
`;
