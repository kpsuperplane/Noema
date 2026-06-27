import { ApolloClient, ApolloLink, HttpLink, InMemoryCache } from "@apollo/client";
import { GraphQLWsLink } from "@apollo/client/link/subscriptions";
import { OperationTypeNode } from "graphql";
import { createClient } from "graphql-ws";

function graphqlWsUrl() {
  const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
  return `${protocol}//${window.location.host}/graphql/ws`;
}

const httpLink = new HttpLink({
  uri: "/graphql"
});

const wsLink = new GraphQLWsLink(
  createClient({
    url: graphqlWsUrl(),
    lazy: true,
    retryAttempts: 5
  })
);

const link = ApolloLink.split(
  ({ operationType }) => operationType === OperationTypeNode.SUBSCRIPTION,
  wsLink,
  httpLink
);

export const apolloClient = new ApolloClient({
  link,
  cache: new InMemoryCache({
    typePolicies: {
      GraphqlConversationItem: {
        keyFields: ["itemId"]
      },
      GraphqlConversationItemEvent: {
        keyFields: ["itemId"]
      },
      GraphqlAgentStatusEvent: {
        keyFields: false
      },
      GraphqlTurnCompletedEvent: {
        keyFields: false
      }
    }
  })
});
