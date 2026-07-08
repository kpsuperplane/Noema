import { useMutation, useQuery } from "@apollo/client/react";
import {
  CheckMemoryServiceDocument,
  MemorySettingsDocument,
  SaveMemoryServiceSettingsDocument,
  type CheckMemoryServiceMutation,
  type MemorySettingsQuery,
  type SaveMemoryServiceSettingsMutation,
  type SaveMemoryServiceSettingsMutationVariables
} from "@/generated/graphql";
import { MemorySettingsPaneContent } from "./MemorySettingsPaneContent";

export { MemorySettingsPaneContent } from "./MemorySettingsPaneContent";

export function MemorySettingsPane() {
  const settingsResult = useQuery<MemorySettingsQuery>(MemorySettingsDocument, {
    fetchPolicy: "cache-and-network"
  });
  const [saveSettings, saveResult] = useMutation<
    SaveMemoryServiceSettingsMutation,
    SaveMemoryServiceSettingsMutationVariables
  >(SaveMemoryServiceSettingsDocument, {
    refetchQueries: [{ query: MemorySettingsDocument }],
    awaitRefetchQueries: true
  });
  const [checkService, checkResult] = useMutation<CheckMemoryServiceMutation>(
    CheckMemoryServiceDocument,
    {
      refetchQueries: [{ query: MemorySettingsDocument }],
      awaitRefetchQueries: true
    }
  );

  return (
    <MemorySettingsPaneContent
      settings={settingsResult.data?.memorySettings ?? null}
      loading={settingsResult.loading && !settingsResult.data}
      error={settingsResult.error?.message ?? null}
      saving={saveResult.loading}
      checking={checkResult.loading}
      saveError={saveResult.error?.message ?? null}
      onSave={(input) => saveSettings({ variables: { input } })}
      onCheck={() => checkService()}
    />
  );
}
