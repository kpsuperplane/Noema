import { ApolloLink, Observable, type FetchResult, type Operation } from "@apollo/client";
import { print } from "graphql";
import { invokeDesktop, listenDesktop } from "./desktopBridge";

type DesktopGraphqlResponse = FetchResult<Record<string, unknown>>;

type DesktopSubscriptionPayload = {
  subscriptionId: string;
  response: DesktopGraphqlResponse;
};

type DesktopTransportDependencies = {
  createSubscriptionId(): string;
  invokeDesktop<T>(command: string, args?: Record<string, unknown>): Promise<T>;
  listenDesktop<T>(eventName: string, handler: (payload: T) => void): Promise<() => void>;
};

const defaultDependencies: DesktopTransportDependencies = {
  createSubscriptionId: () => crypto.randomUUID(),
  invokeDesktop,
  listenDesktop
};

function requestJson(operation: Operation) {
  return {
    query: print(operation.query),
    variables: operation.variables,
    operationName: operation.operationName
  };
}

export function createDesktopGraphqlLink(dependencies = defaultDependencies) {
  return new ApolloLink((operation) => {
    if (operation.operationType === "subscription") {
      return new Observable<FetchResult>((observer) => {
        const subscriptionId = dependencies.createSubscriptionId();
        let disposed = false;
        let unlisten: (() => void) | null = null;
        let subscribeStarted = false;

        void dependencies
          .listenDesktop<DesktopSubscriptionPayload>("graphql_subscription_event", (payload) => {
            if (payload.subscriptionId !== subscriptionId) {
              return;
            }
            observer.next(payload.response);
          })
          .then((nextUnlisten) => {
            unlisten = nextUnlisten;
            if (disposed) {
              unlisten();
              return;
            }
            subscribeStarted = true;
            void dependencies
              .invokeDesktop("graphql_subscribe", {
                subscriptionId,
                requestJson: requestJson(operation)
              })
              .catch((error: unknown) => {
                if (!disposed) {
                  observer.error(error);
                }
              });
          })
          .catch((error: unknown) => {
            if (!disposed) {
              observer.error(error);
            }
          });

        return () => {
          disposed = true;
          unlisten?.();
          if (subscribeStarted) {
            void dependencies.invokeDesktop("graphql_unsubscribe", { subscriptionId });
          }
        };
      });
    }

    return new Observable<FetchResult>((observer) => {
      void dependencies
        .invokeDesktop<DesktopGraphqlResponse>("graphql_execute", {
        requestJson: requestJson(operation)
      })
        .then((response) => {
          observer.next(response);
          observer.complete();
        })
        .catch((error: unknown) => {
          observer.error(error);
        });
    });
  });
}
