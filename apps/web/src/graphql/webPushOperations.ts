import { gql } from "@apollo/client";

export const WebPushStatusDocument = gql`
  query WebPushStatus($endpoint: String) {
    webPushStatus(endpoint: $endpoint) {
      available
      blocker
      applicationServerKey
      subscriptionId
    }
  }
`;

export const RegisterWebPushSubscriptionDocument = gql`
  mutation RegisterWebPushSubscription($input: RegisterWebPushSubscriptionInput!) {
    registerWebPushSubscription(input: $input) {
      available
      blocker
      applicationServerKey
      subscriptionId
    }
  }
`;

export const RemoveWebPushSubscriptionDocument = gql`
  mutation RemoveWebPushSubscription($subscriptionId: String!) {
    removeWebPushSubscription(subscriptionId: $subscriptionId)
  }
`;

export const WebPushPresenceDocument = gql`
  subscription WebPushPresence($subscriptionId: String!) {
    webPushPresence(subscriptionId: $subscriptionId) {
      subscriptionId
      ready
    }
  }
`;
