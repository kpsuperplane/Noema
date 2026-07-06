import { useMutation, useQuery } from "@apollo/client/react";
import {
  SaveWebFetchSummarizerPreferenceDocument,
  WebFetchSettingsDocument,
  type SaveWebFetchSummarizerPreferenceMutation,
  type SaveWebFetchSummarizerPreferenceMutationVariables,
  type WebFetchSettingsQuery
} from "@/generated/graphql";
import { WebSettingsPaneContent } from "./WebSettingsPaneContent";

export { WebSettingsPaneContent } from "./WebSettingsPaneContent";

export function WebSettingsPane() {
  const webFetchResult = useQuery<WebFetchSettingsQuery>(WebFetchSettingsDocument, {
    fetchPolicy: "cache-and-network"
  });
  const [saveWebFetchSummarizerPreference, saveWebFetchResult] = useMutation<
    SaveWebFetchSummarizerPreferenceMutation,
    SaveWebFetchSummarizerPreferenceMutationVariables
  >(SaveWebFetchSummarizerPreferenceDocument, {
    refetchQueries: [{ query: WebFetchSettingsDocument }],
    awaitRefetchQueries: true
  });

  return (
    <WebSettingsPaneContent
      webFetchSummarizer={webFetchResult.data?.webFetchSettings.summarizer ?? null}
      loading={webFetchResult.loading && !webFetchResult.data}
      error={webFetchResult.error?.message ?? null}
      saving={saveWebFetchResult.loading}
      saveError={saveWebFetchResult.error?.message ?? null}
      onSaveWebFetchSummarizerPreference={(input) =>
        saveWebFetchSummarizerPreference({ variables: { input } })
      }
    />
  );
}
