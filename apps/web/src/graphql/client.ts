import { ApolloClient, ApolloLink, InMemoryCache } from "@apollo/client";
import { createPwaApolloLink, pwaRuntime } from "@/pwa/runtime";
import { loadDurableSnapshot } from "@/pwa/storage";
import { createBrowserGraphqlLink } from "./browserTransport";
import { createDesktopGraphqlLink } from "./desktopTransport";
import { isTauriRuntime } from "./transportMode";

export async function createApolloClient() {
  const durableSnapshot = await loadDurableSnapshot();
  pwaRuntime.initialize(durableSnapshot);
  const transport = isTauriRuntime() ? createDesktopGraphqlLink() : createBrowserGraphqlLink();
  const cache = new InMemoryCache({
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
  });
  if (durableSnapshot) cache.restore(durableSnapshot.cache);

  const client = new ApolloClient({
    link: ApolloLink.from([createPwaApolloLink(), transport]),
    cache
  });
  pwaRuntime.attach(client);
  return client;
}
