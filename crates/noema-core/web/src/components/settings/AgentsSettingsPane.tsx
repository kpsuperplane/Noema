import { useMutation, useQuery } from "@apollo/client/react";
import {
  AgentsDocument,
  SaveAgentModelPreferenceDocument,
  SaveWebFetchSummarizerPreferenceDocument,
  WebFetchSettingsDocument,
  type AgentsQuery,
  type SaveAgentModelPreferenceMutation,
  type SaveAgentModelPreferenceMutationVariables,
  type SaveWebFetchSummarizerPreferenceMutation,
  type SaveWebFetchSummarizerPreferenceMutationVariables,
  type WebFetchSettingsQuery
} from "@/generated/graphql";
import { AgentsSettingsPaneContent } from "./AgentsSettingsPaneContent";

export { AgentsSettingsPaneContent } from "./AgentsSettingsPaneContent";

export function AgentsSettingsPane() {
  const agentsResult = useQuery<AgentsQuery>(AgentsDocument, { fetchPolicy: "cache-and-network" });
  const webFetchResult = useQuery<WebFetchSettingsQuery>(WebFetchSettingsDocument, {
    fetchPolicy: "cache-and-network"
  });
  const [savePreference, saveResult] = useMutation<
    SaveAgentModelPreferenceMutation,
    SaveAgentModelPreferenceMutationVariables
  >(SaveAgentModelPreferenceDocument, {
    refetchQueries: [{ query: AgentsDocument }],
    awaitRefetchQueries: true
  });
  const [saveWebFetchSummarizerPreference, saveWebFetchResult] = useMutation<
    SaveWebFetchSummarizerPreferenceMutation,
    SaveWebFetchSummarizerPreferenceMutationVariables
  >(SaveWebFetchSummarizerPreferenceDocument, {
    refetchQueries: [{ query: WebFetchSettingsDocument }],
    awaitRefetchQueries: true
  });

  return (
    <AgentsSettingsPaneContent
      agents={agentsResult.data?.agents ?? []}
      webFetchSummarizer={webFetchResult.data?.webFetchSettings.summarizer ?? null}
      loading={(agentsResult.loading && !agentsResult.data) || (webFetchResult.loading && !webFetchResult.data)}
      error={agentsResult.error?.message ?? null}
      webFetchError={webFetchResult.error?.message ?? null}
      saving={saveResult.loading || saveWebFetchResult.loading}
      saveError={saveResult.error?.message ?? saveWebFetchResult.error?.message ?? null}
      onSaveModelPreference={(input) => savePreference({ variables: { input } })}
      onSaveWebFetchSummarizerPreference={(input) =>
        saveWebFetchSummarizerPreference({ variables: { input } })
      }
    />
  );
}
