import { ApolloLink, Observable, type FetchResult, type Operation } from "@apollo/client";
import { print } from "graphql";
import { invokeDesktop, listenDesktop } from "./desktopBridge";

type DesktopGraphqlResponse = FetchResult<Record<string, unknown>>;

type DesktopSubscriptionPayload = {
  subscriptionId: string;
  response: DesktopGraphqlResponse;
};

function requestJson(operation: Operation) {
  return {
    query: print(operation.query),
    variables: operation.variables,
    operationName: operation.operationName
  };
}

export function createDesktopGraphqlLink() {
  return new ApolloLink((operation) => {
    if (operation.operationType === "subscription") {
      return new Observable<FetchResult>((observer) => {
        const subscriptionId = crypto.randomUUID();
        let disposed = false;
        let unlisten: (() => void) | null = null;

        void listenDesktop<DesktopSubscriptionPayload>("graphql_subscription_event", (payload) => {
          if (payload.subscriptionId !== subscriptionId) {
            return;
          }
          observer.next(payload.response);
        })
          .then((nextUnlisten) => {
            unlisten = nextUnlisten;
            if (disposed) {
              unlisten();
            }
          })
          .catch((error: unknown) => {
            observer.error(error);
          });

        void invokeDesktop("graphql_subscribe", {
          subscriptionId,
          requestJson: requestJson(operation)
        }).catch((error: unknown) => {
          observer.error(error);
        });

        return () => {
          disposed = true;
          unlisten?.();
          void invokeDesktop("graphql_unsubscribe", { subscriptionId });
        };
      });
    }

    return new Observable<FetchResult>((observer) => {
      void invokeDesktop<DesktopGraphqlResponse>("graphql_execute", {
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
