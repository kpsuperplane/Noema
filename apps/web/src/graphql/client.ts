import { ApolloClient, ApolloLink, InMemoryCache } from "@apollo/client";
import possibleTypes from "@/generated/possibleTypes.json";
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
    possibleTypes: possibleTypes.possibleTypes,
    typePolicies: {
      AcpAgent: { keyFields: ["agentId"] },
      Agent: { keyFields: ["agentId"] },
      CapabilityConnection: { keyFields: ["kind", "connectionId"] },
      CapabilityManagedTool: { keyFields: ["kind", "connectionId", "toolId"] },
      Client: { keyFields: ["clientId"] },
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
      },
      CurrentRunSummary: { keyFields: ["runId"] },
      LocalModelInstallation: { keyFields: ["installationId"] },
      MultipleChoiceOption: { keyFields: false },
      Project: { keyFields: ["projectId"] },
      ProviderAccount: { keyFields: ["providerAccountId"] },
      TaskCard: { keyFields: ["taskId"] },
      TaskDetail: { keyFields: ["taskId"] },
      TaskGate: { keyFields: ["gateId"] },
      TaskModelPoolEntry: { keyFields: ["poolEntryId"] },
      TaskRecurrence: { keyFields: ["recurrenceId"] },
      TaskRecurrenceSummary: { keyFields: ["recurrenceId"] },
      TaskRun: { keyFields: ["runId"] },
      TaskSummary: { keyFields: ["taskId"] },
      WebToolBindingSettings: { keyFields: ["toolName"] },
      WorkflowStage: { keyFields: ["stageId"] }
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
