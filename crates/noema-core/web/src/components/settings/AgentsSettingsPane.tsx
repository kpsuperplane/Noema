import { useQuery } from "@apollo/client/react";
import { AgentsDocument, type AgentsQuery } from "@/generated/graphql";
import { AgentsSettingsPaneContent } from "./AgentsSettingsPaneContent";

export { AgentsSettingsPaneContent } from "./AgentsSettingsPaneContent";

export function AgentsSettingsPane() {
  const result = useQuery<AgentsQuery>(AgentsDocument, { fetchPolicy: "cache-and-network" });

  return (
    <AgentsSettingsPaneContent
      agents={result.data?.agents ?? []}
      loading={result.loading && !result.data}
      error={result.error?.message ?? null}
    />
  );
}
