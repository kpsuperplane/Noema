import { ApolloLink, HttpLink } from "@apollo/client";
import { GraphQLWsLink } from "@apollo/client/link/subscriptions";
import { OperationTypeNode } from "graphql";
import { createClient } from "graphql-ws";

function graphqlWsUrl() {
  const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
  return `${protocol}//${window.location.host}/graphql/ws`;
}

export function createBrowserGraphqlLink() {
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

  return ApolloLink.split(
    ({ operationType }) => operationType === OperationTypeNode.SUBSCRIPTION,
    wsLink,
    httpLink
  );
}
