import { useMutation, useQuery } from "@apollo/client/react";
import {
  MemorySettingsDocument,
  SaveMemoryModelPreferenceDocument,
  type MemorySettingsQuery,
  type SaveMemoryModelPreferenceMutation,
  type SaveMemoryModelPreferenceMutationVariables
} from "@/generated/graphql";
import { MemorySettingsPaneContent } from "./MemorySettingsPaneContent";

export { MemorySettingsPaneContent } from "./MemorySettingsPaneContent";

export function MemorySettingsPane() {
  const settingsResult = useQuery<MemorySettingsQuery>(MemorySettingsDocument, {
    fetchPolicy: "cache-and-network"
  });
  const [savePreference, saveResult] = useMutation<
    SaveMemoryModelPreferenceMutation,
    SaveMemoryModelPreferenceMutationVariables
  >(SaveMemoryModelPreferenceDocument, {
    refetchQueries: [{ query: MemorySettingsDocument }],
    awaitRefetchQueries: true
  });

  return (
    <MemorySettingsPaneContent
      settings={settingsResult.data?.memorySettings ?? null}
      loading={settingsResult.loading && !settingsResult.data}
      error={settingsResult.error?.message ?? null}
      saving={saveResult.loading}
      saveError={saveResult.error?.message ?? null}
      onSave={(input) =>
        savePreference({
          variables: {
            input: {
              providerAccountId: input.providerAccountId,
              modelProfile: input.modelProfile,
              reasoningEffort: input.reasoningEffort ?? null
            }
          }
        })
      }
    />
  );
}
