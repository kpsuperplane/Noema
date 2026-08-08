import { useMutation, useQuery } from "@apollo/client/react";
import {
  SaveWebToolProviderBindingDocument,
  SaveWebFetchSummarizerPreferenceDocument,
  WebToolSettingsDocument,
  WebFetchSettingsDocument,
  type SaveWebToolProviderBindingInput,
  type SaveWebToolProviderBindingMutation,
  type SaveWebToolProviderBindingMutationVariables,
  type SaveWebFetchSummarizerPreferenceMutation,
  type SaveWebFetchSummarizerPreferenceMutationVariables,
  type WebFetchSettingsQuery,
  type WebToolSettingsQuery
} from "@/generated/graphql";
import { WebSettingsPaneContent } from "./WebSettingsPaneContent";

export function WebSettingsPane() {
  const webFetchResult = useQuery<WebFetchSettingsQuery>(WebFetchSettingsDocument, {
    fetchPolicy: "cache-and-network"
  });
  const webToolResult = useQuery<WebToolSettingsQuery>(WebToolSettingsDocument, {
    fetchPolicy: "cache-and-network"
  });
  const [saveWebFetchSummarizerPreference, saveWebFetchResult] = useMutation<
    SaveWebFetchSummarizerPreferenceMutation,
    SaveWebFetchSummarizerPreferenceMutationVariables
  >(SaveWebFetchSummarizerPreferenceDocument, {
    refetchQueries: [{ query: WebFetchSettingsDocument }],
    awaitRefetchQueries: true
  });
  const [saveWebToolProviderBinding, saveWebToolResult] = useMutation<
    SaveWebToolProviderBindingMutation,
    SaveWebToolProviderBindingMutationVariables
  >(SaveWebToolProviderBindingDocument, {
    refetchQueries: [{ query: WebToolSettingsDocument }],
    awaitRefetchQueries: true
  });

  return (
    <WebSettingsPaneContent
      webFetchSummarizer={webFetchResult.data?.webFetchSettings.summarizer ?? null}
      webToolSettings={webToolResult.data?.webToolSettings ?? null}
      loading={webFetchResult.loading && !webFetchResult.data}
      error={webFetchResult.error?.message ?? null}
      saving={saveWebFetchResult.loading}
      saveError={saveWebFetchResult.error?.message ?? null}
      webToolLoading={webToolResult.loading && !webToolResult.data}
      webToolError={webToolResult.error?.message ?? null}
      webToolSaving={saveWebToolResult.loading}
      webToolSaveError={saveWebToolResult.error?.message ?? null}
      onSaveWebFetchSummarizerPreference={(input) =>
        saveWebFetchSummarizerPreference({ variables: { input } })
      }
      onSaveWebToolProviderBinding={(input: SaveWebToolProviderBindingInput) =>
        saveWebToolProviderBinding({ variables: { input } })
      }
    />
  );
}
