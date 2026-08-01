import { ApolloLink, HttpLink } from "@apollo/client";
import { GraphQLWsLink } from "@apollo/client/link/subscriptions";
import { RetryLink } from "@apollo/client/link/retry";
import { OperationTypeNode } from "graphql";
import { createClient, type ClientOptions } from "graphql-ws";

const GRAPHQL_WS_INITIAL_RETRY_DELAY_MS = 500;
const GRAPHQL_WS_MAX_RETRY_DELAY_MS = 5_000;

type BrowserGraphqlWsLocation = Pick<Location, "protocol" | "host">;

export type BrowserGraphqlConnectionSnapshot = {
  state: "closed" | "connecting" | "ready";
  recoverySequence: number;
};

export class BrowserGraphqlConnectionMonitor {
  private snapshot: BrowserGraphqlConnectionSnapshot = {
    state: "closed",
    recoverySequence: 0
  };

  private readonly listeners = new Set<() => void>();

  private hasConnected = false;

  private recovering = false;

  getSnapshot = () => this.snapshot;

  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  connecting(wasRetry = false) {
    this.recovering ||= this.hasConnected || wasRetry;
    this.update({ ...this.snapshot, state: "connecting" });
  }

  connected(wasRetry: boolean) {
    const recovered = wasRetry || this.recovering;
    this.hasConnected = true;
    this.recovering = false;
    this.update({
      state: "ready",
      recoverySequence: this.snapshot.recoverySequence + (recovered ? 1 : 0)
    });
  }

  private update(next: BrowserGraphqlConnectionSnapshot) {
    if (
      next.state === this.snapshot.state &&
      next.recoverySequence === this.snapshot.recoverySequence
    ) {
      return;
    }
    this.snapshot = next;
    this.listeners.forEach((listener) => listener());
  }
}

export const browserGraphqlConnectionMonitor = new BrowserGraphqlConnectionMonitor();

export function waitForBrowserGraphqlReady(timeoutMs = 8_000) {
  if (browserGraphqlConnectionMonitor.getSnapshot().state === "ready") return Promise.resolve();
  return new Promise<void>((resolve, reject) => {
    const timeout = window.setTimeout(() => {
      unsubscribe();
      reject(new Error("Noema could not restore its live connection."));
    }, timeoutMs);
    const unsubscribe = browserGraphqlConnectionMonitor.subscribe(() => {
      if (browserGraphqlConnectionMonitor.getSnapshot().state !== "ready") return;
      window.clearTimeout(timeout);
      unsubscribe();
      resolve();
    });
  });
}

export function browserGraphqlWsRetryDelayMs(retryAttempt: number) {
  return Math.min(
    GRAPHQL_WS_INITIAL_RETRY_DELAY_MS * 2 ** retryAttempt,
    GRAPHQL_WS_MAX_RETRY_DELAY_MS
  );
}

export function createBrowserGraphqlWsClientOptions(
  location: BrowserGraphqlWsLocation = window.location,
  connectionMonitor = browserGraphqlConnectionMonitor
): ClientOptions {
  const protocol = location.protocol === "https:" ? "wss:" : "ws:";
  return {
    url: `${protocol}//${location.host}/graphql/ws`,
    lazy: true,
    retryAttempts: Number.POSITIVE_INFINITY,
    on: {
      connecting: (wasRetry) => connectionMonitor.connecting(wasRetry),
      connected: (_socket, _payload, wasRetry) => connectionMonitor.connected(wasRetry),
      closed: () => connectionMonitor.connecting(),
      error: () => connectionMonitor.connecting()
    },
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
  const retryingWsLink = new RetryLink({
    attempts: { max: Number.POSITIVE_INFINITY },
    delay: {
      initial: GRAPHQL_WS_INITIAL_RETRY_DELAY_MS,
      max: GRAPHQL_WS_MAX_RETRY_DELAY_MS,
      jitter: true
    }
  }).concat(wsLink);

  return ApolloLink.split(
    ({ operationType }) => operationType === OperationTypeNode.SUBSCRIPTION,
    retryingWsLink,
    httpLink
  );
}
