import { gql } from "@apollo/client";

export const PendingGovernedActionsDocument = gql`
  query PendingGovernedActions($conversationId: String, $taskId: String, $first: Int = 50) {
    pendingGovernedActions(conversationId: $conversationId, taskId: $taskId, first: $first) {
      actionId
      revision
      conversationId
      taskId
      runId
      capabilityName
      effect
      safeSummary
      arguments
      state
      failureCode
    }
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
      effect
      safeSummary
      arguments
      state
      output
      failureCode
    }
  }
`;
