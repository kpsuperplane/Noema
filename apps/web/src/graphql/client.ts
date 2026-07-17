import { ApolloClient, InMemoryCache } from "@apollo/client";
import { createBrowserGraphqlLink } from "./browserTransport";
import { createDesktopGraphqlLink } from "./desktopTransport";
import { isTauriRuntime } from "./transportMode";

const link = isTauriRuntime() ? createDesktopGraphqlLink() : createBrowserGraphqlLink();

export const apolloClient = new ApolloClient({
  link,
  cache: new InMemoryCache({
    typePolicies: {
      ConversationItem: {
        keyFields: ["itemId"]
      },
      ConversationItemEvent: {
        keyFields: ["itemId"]
      },
      AgentStatusEvent: {
        keyFields: false
      },
      TurnCompletedEvent: {
        keyFields: false
      }
    }
  })
});
