import { useMutation, useQuery } from "@apollo/client/react";
import {
  AgentsDocument,
  SaveAgentModelPreferenceDocument,
  type AgentsQuery,
  type SaveAgentModelPreferenceMutation,
  type SaveAgentModelPreferenceMutationVariables
} from "@/generated/graphql";
import { AgentsSettingsPaneContent } from "./AgentsSettingsPaneContent";

export { AgentsSettingsPaneContent } from "./AgentsSettingsPaneContent";

export function AgentsSettingsPane() {
  const agentsResult = useQuery<AgentsQuery>(AgentsDocument, { fetchPolicy: "cache-and-network" });
  const [savePreference, saveResult] = useMutation<
    SaveAgentModelPreferenceMutation,
    SaveAgentModelPreferenceMutationVariables
  >(SaveAgentModelPreferenceDocument, {
    refetchQueries: [{ query: AgentsDocument }],
    awaitRefetchQueries: true
  });

  return (
    <AgentsSettingsPaneContent
      agents={agentsResult.data?.agents ?? []}
      loading={agentsResult.loading && !agentsResult.data}
      error={agentsResult.error?.message ?? null}
      saving={saveResult.loading}
      saveError={saveResult.error?.message ?? null}
      onSaveModelPreference={(input) => savePreference({ variables: { input } })}
    />
  );
}
