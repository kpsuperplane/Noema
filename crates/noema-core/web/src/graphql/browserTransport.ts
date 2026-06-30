import { ApolloLink, HttpLink } from "@apollo/client";
import { GraphQLWsLink } from "@apollo/client/link/subscriptions";
import { OperationTypeNode } from "graphql";
import { createClient, type ClientOptions } from "graphql-ws";

const GRAPHQL_WS_INITIAL_RETRY_DELAY_MS = 500;
const GRAPHQL_WS_MAX_RETRY_DELAY_MS = 5_000;

type BrowserGraphqlWsLocation = Pick<Location, "protocol" | "host">;

export function browserGraphqlWsRetryDelayMs(retryAttempt: number) {
  return Math.min(
    GRAPHQL_WS_INITIAL_RETRY_DELAY_MS * 2 ** retryAttempt,
    GRAPHQL_WS_MAX_RETRY_DELAY_MS
  );
}

export function createBrowserGraphqlWsClientOptions(
  location: BrowserGraphqlWsLocation = window.location
): ClientOptions {
  const protocol = location.protocol === "https:" ? "wss:" : "ws:";
  return {
    url: `${protocol}//${location.host}/graphql/ws`,
    lazy: true,
    retryAttempts: Number.POSITIVE_INFINITY,
    retryWait: async (retryAttempt) => {
      await new Promise((resolve) => {
        window.setTimeout(resolve, browserGraphqlWsRetryDelayMs(retryAttempt));
      });
    }
  };
}

export function createBrowserGraphqlLink() {
  const httpLink = new HttpLink({
    uri: "/graphql"
  });

  const wsLink = new GraphQLWsLink(
    createClient(createBrowserGraphqlWsClientOptions())
  );

  return ApolloLink.split(
    ({ operationType }) => operationType === OperationTypeNode.SUBSCRIPTION,
    wsLink,
    httpLink
  );
}
