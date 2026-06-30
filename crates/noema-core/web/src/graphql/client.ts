import { ApolloClient, InMemoryCache } from "@apollo/client";
import { createBrowserGraphqlLink } from "./browserTransport";
import { createDesktopGraphqlLink } from "./desktopTransport";
import { isTauriRuntime } from "./transportMode";

const link = isTauriRuntime() ? createDesktopGraphqlLink() : createBrowserGraphqlLink();

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
